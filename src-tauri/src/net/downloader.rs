use crate::db::{self, DbPool};
use crate::types::DownloadStatus;
use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::{ETAG, IF_RANGE, LAST_MODIFIED, RANGE};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};

/// Everything one download run needs, gathered up front so `run` itself only
/// deals with the transfer loop.
pub struct DownloadJob {
    pub id: String,
    pub url: String,
    /// The FINAL path a completed download is renamed to. The working file
    /// during transfer is this path plus `.part`, so a crash/force-quit never
    /// leaves something that looks like a completed file.
    pub final_path: PathBuf,
    pub headers: Vec<(String, String)>,
    /// `0` for a fresh download. Non-zero means this is a `resume_download`
    /// call continuing a previously paused/failed transfer.
    pub resume_from: i64,
    pub resume_validator: Option<String>,
}

/// Runs one download to completion, pause, cancel, or failure, persisting
/// status/progress throughout. Always removes its own `state.downloads`
/// entry before returning - that entry's presence is what "currently
/// running" means elsewhere (`pause_download`/`cancel_download` no-op if absent).
pub async fn run(http: Client, db: DbPool, job: DownloadJob, pause: Arc<AtomicBool>, cancel: Arc<AtomicBool>) {
    let result = run_inner(&http, &db, &job, &pause, &cancel).await;

    let final_status = match &result {
        Ok(Outcome::Completed) => DownloadStatus::Completed,
        Ok(Outcome::Paused) => DownloadStatus::Paused,
        Ok(Outcome::Canceled) => DownloadStatus::Canceled,
        Err(_) => DownloadStatus::Failed,
    };
    let error_message = result.as_ref().err().map(|e| e.clone());

    let id = job.id.clone();
    let _ = db::with_conn(&db, move |conn| {
        Ok(db::downloads::update_status(conn, &id, final_status, error_message.as_deref())?)
    })
    .await;

    if matches!(result, Ok(Outcome::Canceled)) {
        let part_path = part_path(&job.final_path);
        let _ = tokio::fs::remove_file(&part_path).await;
    }
}

enum Outcome {
    Completed,
    Paused,
    Canceled,
}

fn part_path(final_path: &std::path::Path) -> PathBuf {
    let mut name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("download").to_string();
    name.push_str(".part");
    final_path.with_file_name(name)
}

async fn run_inner(
    http: &Client,
    db: &DbPool,
    job: &DownloadJob,
    pause: &Arc<AtomicBool>,
    cancel: &Arc<AtomicBool>,
) -> Result<Outcome, String> {
    let part_path = part_path(&job.final_path);
    let resuming = job.resume_from > 0;

    let mut request = http.get(&job.url);
    for (name, value) in &job.headers {
        request = request.header(name.as_str(), value.as_str());
    }
    if resuming {
        request = request.header(RANGE, format!("bytes={}-", job.resume_from));
        if let Some(validator) = &job.resume_validator {
            request = request.header(IF_RANGE, validator.as_str());
        }
    }

    let response = request.send().await.map_err(|e| {
        tracing::warn!("Download {} failed to connect: {e:?}", job.id);
        "Couldn't reach the download source. Check your internet connection.".to_string()
    })?;

    if !response.status().is_success() && response.status().as_u16() != 206 {
        return Err(format!("The server rejected the download request (status {}).", response.status().as_u16()));
    }

    // A server ignoring `Range` answers 200 (not 206) and resends from byte
    // 0 - appending that to existing bytes would corrupt the file, so treat
    // it as a fresh download instead of trusting `resume_from`.
    let actually_resuming = resuming && response.status().as_u16() == 206;

    let validator = response
        .headers()
        .get(ETAG)
        .or_else(|| response.headers().get(LAST_MODIFIED))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if !actually_resuming {
        let id = job.id.clone();
        let validator_clone = validator.clone();
        let _ = db::with_conn(db, move |conn| {
            Ok(db::downloads::set_resume_validator(conn, &id, validator_clone.as_deref())?)
        })
        .await;
    }

    let content_length = response.content_length().map(|n| n as i64);
    let total_bytes = if actually_resuming {
        content_length.map(|n| n + job.resume_from)
    } else {
        content_length
    };

    if let Some(parent) = part_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| format!("Couldn't create the downloads folder: {e}"))?;
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(actually_resuming)
        .truncate(!actually_resuming)
        .open(&part_path)
        .await
        .map_err(|e| format!("Couldn't open the destination file: {e}"))?;
    let mut writer = BufWriter::new(file);

    let mut downloaded: i64 = if actually_resuming { job.resume_from } else { 0 };
    let mut stream = response.bytes_stream();
    let mut last_db_write = Instant::now();

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            let _ = writer.flush().await;
            return Ok(Outcome::Canceled);
        }
        if pause.load(Ordering::Relaxed) {
            let _ = writer.flush().await;
            persist_progress(db, &job.id, downloaded, total_bytes).await;
            return Ok(Outcome::Paused);
        }

        let chunk = chunk.map_err(|e| {
            tracing::warn!("Download {} interrupted mid-transfer: {e:?}", job.id);
            "The connection was interrupted while downloading.".to_string()
        })?;
        writer.write_all(&chunk).await.map_err(|e| format!("Couldn't write to disk: {e}"))?;
        downloaded += chunk.len() as i64;

        if last_db_write.elapsed() >= Duration::from_millis(500) {
            persist_progress(db, &job.id, downloaded, total_bytes).await;
            last_db_write = Instant::now();
        }
    }

    writer.flush().await.map_err(|e| format!("Couldn't write to disk: {e}"))?;
    drop(writer);
    persist_progress(db, &job.id, downloaded, total_bytes).await;

    tokio::fs::rename(&part_path, &job.final_path)
        .await
        .map_err(|e| format!("Download finished but couldn't be saved to its final location: {e}"))?;

    Ok(Outcome::Completed)
}

async fn persist_progress(db: &DbPool, id: &str, downloaded: i64, total: Option<i64>) {
    let id = id.to_string();
    let _ = db::with_conn(db, move |conn| Ok(db::downloads::update_progress(conn, &id, downloaded, total)?)).await;
}
