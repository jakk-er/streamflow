use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::net::downloader::{self, DownloadJob};
use crate::state::{AppState, DownloadHandle};
use crate::types::{DownloadMetadata, DownloadStatus};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::State;

/// Resolves the requested filename into a real path under the downloads
/// directory: strips path separators (defends against traversal via a
/// malicious playlist title), then de-dupes an existing file by appending
/// " (1)", " (2)", ... before the extension.
fn resolve_download_path(dir: &Path, requested_name: &str) -> PathBuf {
    let safe_name = requested_name.rsplit(['/', '\\']).next().unwrap_or(requested_name).trim();
    let safe_name = if safe_name.is_empty() { "download" } else { safe_name };

    let candidate = dir.join(safe_name);
    if !candidate.exists() && !part_exists(&candidate) {
        return candidate;
    }

    let path = Path::new(safe_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("download").to_string();
    let ext = path.extension().and_then(|s| s.to_str()).map(str::to_string);

    let mut n = 1;
    loop {
        let name = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = dir.join(name);
        if !candidate.exists() && !part_exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn part_exists(final_path: &Path) -> bool {
    let mut name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("download").to_string();
    name.push_str(".part");
    final_path.with_file_name(name).exists()
}

fn spawn_job(state: &AppState, job: DownloadJob) {
    let pause = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut downloads = state.downloads.lock().expect("downloads lock poisoned");
        downloads.insert(job.id.clone(), DownloadHandle { pause: pause.clone(), cancel: cancel.clone() });
    }
    let http = state.http.clone();
    let db = state.db.clone();
    tauri::async_runtime::spawn(downloader::run(http, db, job, pause, cancel));
}

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    url: String,
    file_path: String,
    headers: Option<Vec<(String, String)>>,
) -> CommandResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let resolved_path = resolve_download_path(&state.downloads_dir, &file_path);
    let headers = headers.unwrap_or_default();
    let headers_json = if headers.is_empty() { None } else { Some(serde_json::to_string(&headers)?) };

    let resolved_path_str = resolved_path.to_string_lossy().to_string();
    let insert_id = id.clone();
    let insert_url = url.clone();
    let insert_headers = headers_json.clone();
    db::with_conn(&state.db, move |conn| {
        Ok(db::downloads::insert(conn, &insert_id, &insert_url, &resolved_path_str, DownloadStatus::Downloading, insert_headers.as_deref())?)
    })
    .await?;

    spawn_job(
        &state,
        DownloadJob { id: id.clone(), url, final_path: resolved_path, headers, resume_from: 0, resume_validator: None },
    );

    Ok(id)
}

#[tauri::command]
pub async fn get_download_progress(state: State<'_, AppState>, id: String) -> CommandResult<DownloadMetadata> {
    let lookup_id = id.clone();
    let record = db::with_conn(&state.db, move |conn| Ok(db::downloads::get(conn, &lookup_id)?)).await?;
    record.map(|r| r.to_metadata()).ok_or_else(|| CommandError::NotFound(format!("download {id} not found")))
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let downloads = state.downloads.lock().map_err(|_| CommandError::Internal("downloads lock poisoned".into()))?;
    if let Some(handle) = downloads.get(&id) {
        handle.pause.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let already_running = {
        let downloads = state.downloads.lock().map_err(|_| CommandError::Internal("downloads lock poisoned".into()))?;
        if let Some(handle) = downloads.get(&id) {
            handle.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    };

    if already_running {
        // The running task itself deletes the `.part` file and writes the
        // final `canceled` status once it notices the flag - nothing more to
        // do here.
        return Ok(());
    }

    // Not running (already paused, or never resumed after a restart): no
    // task will observe the flag, so this command is the one responsible for
    // cleanup.
    let lookup_id = id.clone();
    let record = db::with_conn(&state.db, move |conn| Ok(db::downloads::get(conn, &lookup_id)?)).await?;
    if let Some(record) = record {
        let part = {
            let path = Path::new(&record.file_path);
            let mut name = path.file_name().and_then(|n| n.to_str()).unwrap_or("download").to_string();
            name.push_str(".part");
            path.with_file_name(name)
        };
        let _ = tokio::fs::remove_file(&part).await;
    }
    let update_id = id.clone();
    db::with_conn(&state.db, move |conn| Ok(db::downloads::update_status(conn, &update_id, DownloadStatus::Canceled, None)?)).await?;
    Ok(())
}

#[tauri::command]
pub async fn resume_download(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    {
        let downloads = state.downloads.lock().map_err(|_| CommandError::Internal("downloads lock poisoned".into()))?;
        if downloads.contains_key(&id) {
            return Err(CommandError::Api("This download is already in progress.".into()));
        }
    }

    let lookup_id = id.clone();
    let record = db::with_conn(&state.db, move |conn| Ok(db::downloads::get(conn, &lookup_id)?)).await?;
    let record = record.ok_or_else(|| CommandError::NotFound(format!("download {id} not found")))?;

    if !matches!(record.status, DownloadStatus::Paused | DownloadStatus::Failed) {
        return Err(CommandError::Api("Only a paused or failed download can be resumed.".into()));
    }

    let final_path = PathBuf::from(&record.file_path);
    let part = {
        let mut name = final_path.file_name().and_then(|n| n.to_str()).unwrap_or("download").to_string();
        name.push_str(".part");
        final_path.with_file_name(name)
    };
    // Trust the file on disk over `downloaded_bytes` (they can disagree if
    // the app was killed before the last throttled progress update landed).
    // Resuming past what's really on disk would leave a gap, so restart from
    // 0 whenever the sizes disagree at all.
    let on_disk_len = tokio::fs::metadata(&part).await.map(|m| m.len() as i64).unwrap_or(0);
    let resume_from = if on_disk_len == record.downloaded_bytes { on_disk_len } else { 0 };

    let headers: Vec<(String, String)> =
        record.request_headers.as_deref().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();

    let update_id = id.clone();
    db::with_conn(&state.db, move |conn| Ok(db::downloads::update_status(conn, &update_id, DownloadStatus::Downloading, None)?)).await?;

    spawn_job(
        &state,
        DownloadJob {
            id: id.clone(),
            url: record.url,
            final_path,
            headers,
            resume_from,
            resume_validator: record.resume_validator,
        },
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_to_a_plain_path_when_nothing_collides() {
        let dir = std::env::temp_dir().join(format!("streamflow-dl-test-{}", uuid::Uuid::new_v4()));
        let path = resolve_download_path(&dir, "Movie Title.mp4");
        assert_eq!(path, dir.join("Movie Title.mp4"));
    }

    #[test]
    fn strips_path_separators_from_a_malicious_title() {
        let dir = std::env::temp_dir().join(format!("streamflow-dl-test-{}", uuid::Uuid::new_v4()));
        let path = resolve_download_path(&dir, "../../etc/evil.mp4");
        assert_eq!(path, dir.join("evil.mp4"));
        assert!(path.starts_with(&dir));
    }

    #[test]
    fn dedupes_an_existing_file_by_appending_a_counter_before_the_extension() {
        let dir = std::env::temp_dir().join(format!("streamflow-dl-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Movie.mp4"), b"x").unwrap();
        std::fs::write(dir.join("Movie (1).mp4"), b"x").unwrap();

        let path = resolve_download_path(&dir, "Movie.mp4");
        assert_eq!(path, dir.join("Movie (2).mp4"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dedupes_against_an_in_progress_part_file_too() {
        let dir = std::env::temp_dir().join(format!("streamflow-dl-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Movie.mp4.part"), b"x").unwrap();

        let path = resolve_download_path(&dir, "Movie.mp4");
        assert_eq!(path, dir.join("Movie (1).mp4"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
