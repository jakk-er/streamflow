use crate::parsers::xmltv::ParsedEpg;
use crate::types::EpgProgram;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Row};

/// Replaces all channels/programs previously stored for `source_url` — a
/// full per-source replace rather than an incremental merge, since it's all
/// one in-memory parse-then-store pass anyway.
pub fn store(conn: &mut Connection, source_url: &str, parsed: &ParsedEpg) -> rusqlite::Result<()> {
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    {
        tx.execute("DELETE FROM epg_programs WHERE source_url = ?1", [source_url])?;
        tx.execute("DELETE FROM epg_channels WHERE source_url = ?1", [source_url])?;

        let mut chan_stmt = tx.prepare(
            "INSERT INTO epg_channels (id, display_name, icon_url, source_url, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                icon_url = excluded.icon_url,
                source_url = excluded.source_url,
                updated_at = excluded.updated_at",
        )?;
        for ch in &parsed.channels {
            chan_stmt.execute(rusqlite::params![ch.id, ch.display_name, ch.icon_url, source_url, now])?;
        }

        // OR IGNORE: real XMLTV feeds sometimes have literal duplicate
        // programme entries; skip them rather than aborting the whole import.
        let mut prog_stmt = tx.prepare(
            "INSERT OR IGNORE INTO epg_programs
                (id, channel_id, start, stop, title, description, category, icon_url, source_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for p in &parsed.programs {
            let id = uuid::Uuid::new_v4().to_string();
            prog_stmt.execute(rusqlite::params![
                id,
                p.channel_id,
                p.start,
                p.stop,
                p.title,
                p.description,
                p.category,
                p.icon_url,
                source_url
            ])?;
        }
    }
    tx.commit()
}

/// Resolves `channels.id` to whatever key `epg_programs` is keyed by: the
/// channel's `tvg_id` if EPG data exists under it, else a case-insensitive
/// `epg_channels.display_name` match (XMLTV ids and M3U `tvg-id`s often
/// don't align across providers).
fn resolve_epg_key(conn: &Connection, channel_id: &str) -> rusqlite::Result<Option<String>> {
    let row: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT tvg_id, name FROM channels WHERE id = ?1",
            [channel_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((tvg_id, name)) = row else {
        return Ok(None);
    };

    if let Some(tvg_id) = tvg_id.filter(|s| !s.trim().is_empty()) {
        let exists: i64 = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM epg_programs WHERE channel_id = ?1)",
            [&tvg_id],
            |row| row.get(0),
        )?;
        if exists != 0 {
            return Ok(Some(tvg_id));
        }
    }

    conn.query_row(
        "SELECT id FROM epg_channels WHERE display_name = ?1 COLLATE NOCASE LIMIT 1",
        [&name],
        |row| row.get(0),
    )
    .optional()
}

pub fn programs_for_channel(
    conn: &Connection,
    channel_id: &str,
    start: &str,
    end: &str,
) -> rusqlite::Result<Vec<EpgProgram>> {
    let Some(epg_key) = resolve_epg_key(conn, channel_id)? else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT * FROM epg_programs
         WHERE channel_id = ?1 AND datetime(start) < datetime(?3) AND datetime(stop) > datetime(?2)
         ORDER BY start ASC LIMIT 500",
    )?;
    let rows = stmt.query_map(rusqlite::params![epg_key, start, end], row_to_program)?;
    rows.collect()
}

pub fn current_program(conn: &Connection, channel_id: &str) -> rusqlite::Result<Option<EpgProgram>> {
    let Some(epg_key) = resolve_epg_key(conn, channel_id)? else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT * FROM epg_programs
         WHERE channel_id = ?1 AND datetime(start) <= datetime('now') AND datetime(stop) > datetime('now')
         ORDER BY start DESC LIMIT 1",
        [epg_key],
        row_to_program,
    )
    .optional()
}

fn row_to_program(row: &Row) -> rusqlite::Result<EpgProgram> {
    Ok(EpgProgram {
        id: row.get("id")?,
        channel_id: row.get("channel_id")?,
        start: row.get("start")?,
        stop: row.get("stop")?,
        title: row.get("title")?,
        description: row.get("description")?,
        category: row.get("category")?,
        icon: row.get("icon_url")?,
    })
}
