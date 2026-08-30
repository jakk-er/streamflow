use crate::db;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use tauri::State;
use tokio::process::Command;

#[tauri::command]
pub async fn get_stream_proxy_port(state: State<'_, AppState>) -> CommandResult<u16> {
    let port = state
        .stream_proxy_port
        .lock()
        .map_err(|_| CommandError::Internal("stream proxy port lock poisoned".into()))?;
    Ok(port.unwrap_or(0))
}

/// Strips a matching pair of leading/trailing quotes off a user-typed path.
/// Windows Explorer's "Copy as path" wraps paths in double quotes; pasted
/// verbatim into a settings field and handed to `CreateProcess`, the quote
/// characters are illegal in a filename and the OS rejects the whole string
/// with `ERROR_INVALID_NAME` (a real launch failure this produced).
fn strip_wrapping_quotes(path: &str) -> &str {
    let trimmed = path.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') && bytes[bytes.len() - 1] == bytes[0] {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

/// Resolves the executable to launch: the user's configured path from
/// Settings if set, otherwise the bare command name, relying on the OS to
/// find it on `PATH`.
fn resolve_player_binary(player_type: &str, settings: &crate::types::AppSettings) -> CommandResult<String> {
    let configured = |raw: &Option<String>| {
        raw.as_deref().map(strip_wrapping_quotes).filter(|p| !p.is_empty()).map(str::to_string)
    };
    match player_type {
        "mpv" => Ok(configured(&settings.mpv_path).unwrap_or_else(|| "mpv".to_string())),
        "vlc" => Ok(configured(&settings.vlc_path).unwrap_or_else(|| "vlc".to_string())),
        other => Err(CommandError::Api(format!("Unsupported external player: {other}"))),
    }
}

/// Builds the argument list for launching `player_type` against `url`. Both
/// mpv and VLC accept a bare stream URL as a positional argument identically
/// to a local file path, so no special-casing is needed for live vs. VOD vs.
/// the local stream-proxy URL.
fn build_player_args(player_type: &str, url: &str, title: Option<&str>) -> Vec<String> {
    match player_type {
        "mpv" => {
            let mut args = vec![format!("--force-media-title={}", title.unwrap_or(url))];
            args.push(url.to_string());
            args
        }
        "vlc" => {
            let mut args = vec![format!("--meta-title={}", title.unwrap_or(url))];
            args.push(url.to_string());
            args
        }
        _ => vec![url.to_string()],
    }
}

#[tauri::command]
pub async fn spawn_external_player(
    state: State<'_, AppState>,
    player_type: String,
    url: String,
    title: Option<String>,
) -> CommandResult<String> {
    let settings = db::with_conn(&state.db, |conn| Ok(db::read_settings(conn))).await?;
    let binary = resolve_player_binary(&player_type, &settings)?;
    let args = build_player_args(&player_type, &url, title.as_deref());
    tracing::debug!("spawn_external_player: binary={binary:?} url={url:?} args={args:?}");

    let child = Command::new(&binary).args(&args).spawn().map_err(|e| {
        tracing::warn!("Failed to spawn {player_type} ({binary}): {e:?}");
        CommandError::Api(format!(
            "Couldn't start {player_type} ({binary}). Make sure it's installed and its path is set correctly in Settings if it's not on your system PATH."
        ))
    })?;

    let session_id = uuid::Uuid::new_v4().to_string();
    {
        let mut players = state.players.lock().map_err(|_| CommandError::Internal("players lock poisoned".into()))?;
        players.insert(session_id.clone(), child);
    }
    Ok(session_id)
}

#[tauri::command]
pub async fn kill_player(state: State<'_, AppState>, session_id: String) -> CommandResult<()> {
    let child = {
        let mut players = state.players.lock().map_err(|_| CommandError::Internal("players lock poisoned".into()))?;
        players.remove(&session_id)
    };
    if let Some(mut child) = child {
        let _ = child.kill().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_player_status(state: State<'_, AppState>, session_id: String) -> CommandResult<bool> {
    let mut players = state.players.lock().map_err(|_| CommandError::Internal("players lock poisoned".into()))?;
    let Some(child) = players.get_mut(&session_id) else {
        return Ok(false);
    };
    match child.try_wait() {
        // `Ok(None)`: still running.
        Ok(None) => Ok(true),
        // Exited (or errored checking) - drop the handle either way, there's
        // nothing further this session can report.
        Ok(Some(_)) | Err(_) => {
            players.remove(&session_id);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_matching_double_quotes_from_a_copy_as_path_style_value() {
        assert_eq!(strip_wrapping_quotes(r#""C:\Program Files\VideoLAN\VLC\vlc.exe""#), r"C:\Program Files\VideoLAN\VLC\vlc.exe");
    }

    #[test]
    fn strips_matching_single_quotes_too() {
        assert_eq!(strip_wrapping_quotes("'/usr/bin/vlc'"), "/usr/bin/vlc");
    }

    #[test]
    fn leaves_an_unquoted_path_untouched() {
        assert_eq!(strip_wrapping_quotes(r"C:\Program Files\VideoLAN\VLC\vlc.exe"), r"C:\Program Files\VideoLAN\VLC\vlc.exe");
    }

    #[test]
    fn leaves_a_mismatched_or_single_quote_char_untouched() {
        // A lone leading quote with no matching trailing one isn't a
        // wrapped path - stripping it would silently mangle a (admittedly
        // unlikely) filename that genuinely starts with a quote character.
        assert_eq!(strip_wrapping_quotes(r#""C:\odd"#), r#""C:\odd"#);
    }

    #[test]
    fn trims_surrounding_whitespace_along_with_quotes() {
        assert_eq!(strip_wrapping_quotes("  \"/usr/bin/vlc\"  "), "/usr/bin/vlc");
    }
}
