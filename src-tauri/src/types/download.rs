use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
    Canceled,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadStatus::Pending => "pending",
            DownloadStatus::Downloading => "downloading",
            DownloadStatus::Paused => "paused",
            DownloadStatus::Completed => "completed",
            DownloadStatus::Failed => "failed",
            DownloadStatus::Canceled => "canceled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(DownloadStatus::Pending),
            "downloading" => Some(DownloadStatus::Downloading),
            "paused" => Some(DownloadStatus::Paused),
            "completed" => Some(DownloadStatus::Completed),
            "failed" => Some(DownloadStatus::Failed),
            "canceled" => Some(DownloadStatus::Canceled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadMetadata {
    pub id: String,
    pub url: String,
    pub file_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub status: DownloadStatus,
    pub created_at: String,
}

/// Full DB row, including columns `DownloadMetadata` never exposes to the
/// frontend (resolution plumbing, not UI state) - not `Serialize`, never
/// crosses IPC. `resume_download` uses this to rebuild a paused request.
#[derive(Debug, Clone)]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub file_path: String,
    pub total_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub status: DownloadStatus,
    pub resume_validator: Option<String>,
    pub request_headers: Option<String>,
    pub created_at: String,
}

impl DownloadRecord {
    pub fn to_metadata(&self) -> DownloadMetadata {
        DownloadMetadata {
            id: self.id.clone(),
            url: self.url.clone(),
            file_path: self.file_path.clone(),
            total_bytes: self.total_bytes,
            downloaded_bytes: self.downloaded_bytes,
            status: self.status,
            created_at: self.created_at.clone(),
        }
    }
}
