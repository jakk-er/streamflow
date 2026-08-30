use crate::db;
use crate::db::DbPool;
use crate::error::{CommandError, CommandResult};
use crate::parsers::xmltv;
use crate::state::AppState;
use crate::types::EpgProgram;
use flate2::read::GzDecoder;
use reqwest::Client;
use std::io::Read;
use tauri::State;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Core of `fetch_epg`, split out so playlist-declared EPG sources (M3U
/// `#EXTM3U` header `x-tvg-url`/`url-tvg`/`tvg-url`) can be auto-fetched
/// right after import/refresh without going through the Tauri command
/// boundary — see `commands::playlist::spawn_playlist_epg_fetch`.
pub(crate) async fn fetch_and_store_epg(http: &Client, db: &DbPool, epg_url: &str) -> CommandResult<()> {
    let trimmed_url = epg_url.trim().to_string();
    if trimmed_url.is_empty() {
        return Err(CommandError::Api("EPG URL is required".into()));
    }

    let response = http
        .get(&trimmed_url)
        .send()
        .await
        .map_err(|e| CommandError::Api(format!("Failed to fetch EPG: {e}")))?;
    if !response.status().is_success() {
        return Err(CommandError::Api(format!(
            "EPG server responded with status {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| CommandError::Api(format!("Failed to read EPG response: {e}")))?;

    // `reqwest`'s gzip feature only decompresses with a `Content-Encoding:
    // gzip` header; many EPG providers just serve a raw `.xml.gz` body with
    // no such header, so detect gzip via magic bytes instead.
    let xml = if bytes.len() >= 2 && bytes[0..2] == GZIP_MAGIC {
        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut decompressed = String::new();
        decoder
            .read_to_string(&mut decompressed)
            .map_err(|e| CommandError::InvalidResponse(format!("Failed to decompress gzipped EPG: {e}")))?;
        decompressed
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };

    let parsed = xmltv::parse(&xml);
    if parsed.channels.is_empty() && parsed.programs.is_empty() {
        return Err(CommandError::InvalidResponse("No EPG data found at that URL".into()));
    }

    db::with_conn(db, move |conn| Ok(db::epg::store(conn, &trimmed_url, &parsed)?)).await
}

#[tauri::command]
#[allow(unused_variables)]
pub async fn fetch_epg(state: State<'_, AppState>, playlist_id: String, epg_url: String) -> CommandResult<()> {
    fetch_and_store_epg(&state.http, &state.db, &epg_url).await
}

#[tauri::command]
pub async fn get_epg_for_channel(
    state: State<'_, AppState>,
    channel_id: String,
    start: String,
    end: String,
) -> CommandResult<Vec<EpgProgram>> {
    db::with_conn(&state.db, move |conn| {
        Ok(db::epg::programs_for_channel(conn, &channel_id, &start, &end)?)
    })
    .await
}

#[tauri::command]
pub async fn get_current_program(
    state: State<'_, AppState>,
    channel_id: String,
) -> CommandResult<Option<EpgProgram>> {
    db::with_conn(&state.db, move |conn| Ok(db::epg::current_program(conn, &channel_id)?)).await
}
