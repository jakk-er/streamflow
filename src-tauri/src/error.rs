use serde::Serialize;
use std::fmt;

/// Every Tauri command returns `Result<T, CommandError>`. The frontend
/// (`src/lib/utils/errors.ts`) strips known prefixes off the serialized
/// string, so `Serialize` must emit that prefixed string directly, not a
/// structured object.
#[derive(Debug)]
pub enum CommandError {
    Api(String),
    Auth(String),
    InvalidResponse(String),
    NotFound(String),
    Internal(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Api(msg) => write!(f, "ApiError: {msg}"),
            CommandError::Auth(msg) => write!(f, "Authentication failed: {msg}"),
            CommandError::InvalidResponse(msg) => write!(f, "Invalid response: {msg}"),
            CommandError::NotFound(msg) => write!(f, "CommandError: not found: {msg}"),
            CommandError::Internal(msg) => write!(f, "CommandError: {msg}"),
        }
    }
}

impl std::error::Error for CommandError {}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

impl From<rusqlite::Error> for CommandError {
    fn from(err: rusqlite::Error) -> Self {
        CommandError::Internal(err.to_string())
    }
}

impl From<r2d2::Error> for CommandError {
    fn from(err: r2d2::Error) -> Self {
        CommandError::Internal(format!("database pool error: {err}"))
    }
}

impl From<reqwest::Error> for CommandError {
    fn from(err: reqwest::Error) -> Self {
        CommandError::Api(err.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(err: serde_json::Error) -> Self {
        CommandError::Internal(format!("serialization error: {err}"))
    }
}
