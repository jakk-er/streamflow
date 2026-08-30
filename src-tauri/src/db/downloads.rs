use crate::types::{DownloadRecord, DownloadStatus};
use chrono::Utc;
use rusqlite::{named_params, Connection, OptionalExtension, Row};

#[allow(clippy::too_many_arguments)]
pub fn insert(
    conn: &Connection,
    id: &str,
    url: &str,
    file_path: &str,
    status: DownloadStatus,
    request_headers: Option<&str>,
) -> rusqlite::Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO downloads (id, url, file_path, total_bytes, downloaded_bytes, status, resume_validator, request_headers, error_message, created_at, updated_at)
         VALUES (:id, :url, :file_path, NULL, 0, :status, NULL, :request_headers, NULL, :created_at, :updated_at)",
        named_params! {
            ":id": id,
            ":url": url,
            ":file_path": file_path,
            ":status": status.as_str(),
            ":request_headers": request_headers,
            ":created_at": now,
            ":updated_at": now,
        },
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> rusqlite::Result<Option<DownloadRecord>> {
    conn.query_row("SELECT * FROM downloads WHERE id = ?1", [id], row_to_record).optional()
}

/// Throttled by the caller (a few times a second, never per-chunk) — every
/// call is a real write, so per-chunk calls would make disk I/O the
/// bottleneck instead of the network.
pub fn update_progress(conn: &Connection, id: &str, downloaded_bytes: i64, total_bytes: Option<i64>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET downloaded_bytes = :downloaded_bytes, total_bytes = COALESCE(:total_bytes, total_bytes), updated_at = :updated_at WHERE id = :id",
        named_params! {
            ":id": id,
            ":downloaded_bytes": downloaded_bytes,
            ":total_bytes": total_bytes,
            ":updated_at": Utc::now().to_rfc3339(),
        },
    )?;
    Ok(())
}

pub fn update_status(conn: &Connection, id: &str, status: DownloadStatus, error_message: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET status = :status, error_message = :error_message, updated_at = :updated_at WHERE id = :id",
        named_params! {
            ":id": id,
            ":status": status.as_str(),
            ":error_message": error_message,
            ":updated_at": Utc::now().to_rfc3339(),
        },
    )?;
    Ok(())
}

pub fn set_resume_validator(conn: &Connection, id: &str, validator: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE downloads SET resume_validator = :validator, updated_at = :updated_at WHERE id = :id",
        named_params! {
            ":id": id,
            ":validator": validator,
            ":updated_at": Utc::now().to_rfc3339(),
        },
    )?;
    Ok(())
}

fn row_to_record(row: &Row) -> rusqlite::Result<DownloadRecord> {
    let status_str: String = row.get("status")?;
    Ok(DownloadRecord {
        id: row.get("id")?,
        url: row.get("url")?,
        file_path: row.get("file_path")?,
        total_bytes: row.get("total_bytes")?,
        downloaded_bytes: row.get("downloaded_bytes")?,
        status: DownloadStatus::from_str(&status_str).unwrap_or(DownloadStatus::Failed),
        resume_validator: row.get("resume_validator")?,
        request_headers: row.get("request_headers")?,
        created_at: row.get("created_at")?,
    })
}
