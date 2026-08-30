use crate::types::{Playlist, PlaylistType};
use rusqlite::{named_params, Connection, OptionalExtension, Row};

fn opt_json<T: serde::Serialize>(v: &Option<T>) -> Option<String> {
    v.as_ref().map(|x| serde_json::to_string(x).unwrap_or_default())
}

fn parse_opt_json<T: serde::de::DeserializeOwned>(s: Option<String>) -> Option<T> {
    s.and_then(|s| serde_json::from_str(&s).ok())
}

pub fn insert(conn: &Connection, p: &Playlist) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO playlists (
            id, title, filename, playlist_type, import_date, last_usage, count, url,
            user_agent, referrer, origin, file_path, epg_urls, detected_epg_urls,
            manual_epg_urls, disabled_epg_urls, auto_refresh, update_date, update_state,
            position, is_temporary, server_url, username, password, mac_address, portal_url,
            is_full_stalker_portal, server_timezone, stalker_token, stalker_session_identity,
            stalker_watchdog_timeout, stalker_timeslot, stalker_serial_number,
            stalker_device_id1, stalker_device_id2, stalker_signature1, stalker_signature2,
            stalker_account_info, hidden_group_titles, stalker_login_completed,
            stalker_not_valid, stalker_endpoint
        ) VALUES (
            :id, :title, :filename, :playlist_type, :import_date, :last_usage, :count, :url,
            :user_agent, :referrer, :origin, :file_path, :epg_urls, :detected_epg_urls,
            :manual_epg_urls, :disabled_epg_urls, :auto_refresh, :update_date, :update_state,
            :position, :is_temporary, :server_url, :username, :password, :mac_address, :portal_url,
            :is_full_stalker_portal, :server_timezone, :stalker_token, :stalker_session_identity,
            :stalker_watchdog_timeout, :stalker_timeslot, :stalker_serial_number,
            :stalker_device_id1, :stalker_device_id2, :stalker_signature1, :stalker_signature2,
            :stalker_account_info, :hidden_group_titles, :stalker_login_completed,
            :stalker_not_valid, :stalker_endpoint
        )",
        named_params! {
            ":id": p.id,
            ":title": p.title,
            ":filename": p.filename,
            ":playlist_type": p.playlist_type.as_str(),
            ":import_date": p.import_date,
            ":last_usage": p.last_usage,
            ":count": p.count,
            ":url": p.url,
            ":user_agent": p.user_agent,
            ":referrer": p.referrer,
            ":origin": p.origin,
            ":file_path": p.file_path,
            ":epg_urls": opt_json(&p.epg_urls),
            ":detected_epg_urls": opt_json(&p.detected_epg_urls),
            ":manual_epg_urls": opt_json(&p.manual_epg_urls),
            ":disabled_epg_urls": opt_json(&p.disabled_epg_urls),
            ":auto_refresh": p.auto_refresh as i64,
            ":update_date": p.update_date,
            ":update_state": p.update_state,
            ":position": p.position,
            ":is_temporary": p.is_temporary.map(|b| b as i64),
            ":server_url": p.server_url,
            ":username": p.username,
            ":password": p.password,
            ":mac_address": p.mac_address,
            ":portal_url": p.portal_url,
            ":is_full_stalker_portal": p.is_full_stalker_portal.map(|b| b as i64),
            ":server_timezone": p.server_timezone,
            ":stalker_token": p.stalker_token,
            ":stalker_session_identity": p.stalker_session_identity,
            ":stalker_watchdog_timeout": p.stalker_watchdog_timeout,
            ":stalker_timeslot": p.stalker_timeslot,
            ":stalker_serial_number": p.stalker_serial_number,
            ":stalker_device_id1": p.stalker_device_id1,
            ":stalker_device_id2": p.stalker_device_id2,
            ":stalker_signature1": p.stalker_signature1,
            ":stalker_signature2": p.stalker_signature2,
            ":stalker_account_info": opt_json(&p.stalker_account_info),
            ":hidden_group_titles": opt_json(&p.hidden_group_titles),
            ":stalker_login_completed": p.stalker_login_completed.map(|b| b as i64),
            ":stalker_not_valid": p.stalker_not_valid.map(|b| b as i64),
            ":stalker_endpoint": p.stalker_endpoint,
        },
    )?;
    Ok(())
}

/// A real `UPDATE`, not `INSERT OR REPLACE` — SQLite implements the latter
/// as delete-then-insert, which would `ON DELETE CASCADE`-wipe this row's
/// `channels`/`favorites`/`watch_history` on every edit even though the id
/// never changes.
pub fn update(conn: &Connection, p: &Playlist) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE playlists SET
            title = :title, filename = :filename, url = :url, user_agent = :user_agent,
            referrer = :referrer, origin = :origin, file_path = :file_path,
            server_url = :server_url, username = :username, password = :password,
            mac_address = :mac_address, portal_url = :portal_url,
            stalker_serial_number = :stalker_serial_number,
            stalker_device_id2 = :stalker_device_id2, stalker_signature1 = :stalker_signature1,
            stalker_signature2 = :stalker_signature2, update_date = :update_date,
            auto_refresh = :auto_refresh
        WHERE id = :id",
        named_params! {
            ":id": p.id,
            ":auto_refresh": p.auto_refresh as i64,
            ":title": p.title,
            ":filename": p.filename,
            ":url": p.url,
            ":user_agent": p.user_agent,
            ":referrer": p.referrer,
            ":origin": p.origin,
            ":file_path": p.file_path,
            ":server_url": p.server_url,
            ":username": p.username,
            ":password": p.password,
            ":mac_address": p.mac_address,
            ":portal_url": p.portal_url,
            ":stalker_serial_number": p.stalker_serial_number,
            ":stalker_device_id2": p.stalker_device_id2,
            ":stalker_signature1": p.stalker_signature1,
            ":stalker_signature2": p.stalker_signature2,
            ":update_date": p.update_date,
        },
    )?;
    Ok(())
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Playlist>> {
    let mut stmt = conn.prepare("SELECT * FROM playlists ORDER BY import_date DESC")?;
    let rows = stmt.query_map([], row_to_playlist)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: &str) -> rusqlite::Result<Option<Playlist>> {
    conn.query_row("SELECT * FROM playlists WHERE id = ?1", [id], row_to_playlist)
        .optional()
}

pub fn delete(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

pub fn update_stalker_session(conn: &Connection, id: &str, session: &crate::types::StalkerSessionInfo) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE playlists SET
            stalker_token = :token,
            stalker_endpoint = :endpoint,
            is_full_stalker_portal = :full_portal,
            stalker_watchdog_timeout = :watchdog_timeout,
            stalker_timeslot = :timeslot,
            stalker_not_valid = :not_valid,
            stalker_login_completed = :login_completed,
            stalker_session_identity = :session_identity
         WHERE id = :id",
        named_params! {
            ":token": session.token,
            ":endpoint": session.endpoint,
            ":full_portal": session.full_portal as i64,
            ":watchdog_timeout": session.watchdog_timeout,
            ":timeslot": session.timeslot,
            ":not_valid": session.not_valid as i64,
            ":login_completed": session.login_completed as i64,
            ":session_identity": session.session_fingerprint,
            ":id": id,
        },
    )?;
    Ok(())
}

pub fn update_stalker_credentials(conn: &Connection, id: &str, username: &str, password: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE playlists SET username = ?1, password = ?2 WHERE id = ?3",
        rusqlite::params![username, password, id],
    )?;
    Ok(())
}

pub fn update_count(conn: &Connection, id: &str, count: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE playlists SET count = ?1 WHERE id = ?2",
        rusqlite::params![count, id],
    )?;
    Ok(())
}

fn row_to_playlist(row: &Row) -> rusqlite::Result<Playlist> {
    let playlist_type_str: String = row.get("playlist_type")?;
    let playlist_type = PlaylistType::from_str(&playlist_type_str).unwrap_or(PlaylistType::M3u);

    Ok(Playlist {
        id: row.get("id")?,
        title: row.get("title")?,
        filename: row.get("filename")?,
        playlist_type,
        import_date: row.get("import_date")?,
        last_usage: row.get("last_usage")?,
        count: row.get("count")?,
        url: row.get("url")?,
        user_agent: row.get("user_agent")?,
        referrer: row.get("referrer")?,
        origin: row.get("origin")?,
        file_path: row.get("file_path")?,
        epg_urls: parse_opt_json(row.get::<_, Option<String>>("epg_urls")?),
        detected_epg_urls: parse_opt_json(row.get::<_, Option<String>>("detected_epg_urls")?),
        manual_epg_urls: parse_opt_json(row.get::<_, Option<String>>("manual_epg_urls")?),
        disabled_epg_urls: parse_opt_json(row.get::<_, Option<String>>("disabled_epg_urls")?),
        auto_refresh: row.get::<_, i64>("auto_refresh")? != 0,
        update_date: row.get("update_date")?,
        update_state: row.get("update_state")?,
        position: row.get("position")?,
        is_temporary: row.get::<_, Option<i64>>("is_temporary")?.map(|v| v != 0),
        server_url: row.get("server_url")?,
        username: row.get("username")?,
        password: row.get("password")?,
        mac_address: row.get("mac_address")?,
        portal_url: row.get("portal_url")?,
        is_full_stalker_portal: row
            .get::<_, Option<i64>>("is_full_stalker_portal")?
            .map(|v| v != 0),
        server_timezone: row.get("server_timezone")?,
        stalker_token: row.get("stalker_token")?,
        stalker_session_identity: row.get("stalker_session_identity")?,
        stalker_watchdog_timeout: row.get("stalker_watchdog_timeout")?,
        stalker_timeslot: row.get("stalker_timeslot")?,
        stalker_serial_number: row.get("stalker_serial_number")?,
        stalker_device_id1: row.get("stalker_device_id1")?,
        stalker_device_id2: row.get("stalker_device_id2")?,
        stalker_signature1: row.get("stalker_signature1")?,
        stalker_signature2: row.get("stalker_signature2")?,
        stalker_account_info: parse_opt_json(row.get::<_, Option<String>>("stalker_account_info")?),
        hidden_group_titles: parse_opt_json(row.get::<_, Option<String>>("hidden_group_titles")?),
        stalker_login_completed: row
            .get::<_, Option<i64>>("stalker_login_completed")?
            .map(|v| v != 0),
        stalker_not_valid: row.get::<_, Option<i64>>("stalker_not_valid")?.map(|v| v != 0),
        stalker_endpoint: row.get("stalker_endpoint")?,
    })
}
