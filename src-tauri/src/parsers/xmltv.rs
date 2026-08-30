use chrono::{DateTime, Duration, NaiveDate, Utc};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct ParsedEpgChannel {
    pub id: String,
    pub display_name: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedEpgProgram {
    pub channel_id: String,
    /// UTC RFC3339 strings, already timezone-normalized from the XMLTV
    /// `YYYYMMDDHHMMSS [+-HHMM]` format.
    pub start: String,
    pub stop: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Default)]
pub struct ParsedEpg {
    pub channels: Vec<ParsedEpgChannel>,
    pub programs: Vec<ParsedEpgProgram>,
}

#[derive(PartialEq)]
enum Context {
    None,
    Channel,
    Programme,
}

enum TextTarget {
    DisplayName,
    Title,
    Desc,
    Category,
}

/// Whole-document XMLTV parse (channels + programmes). Not a true streaming
/// SAX-to-DB pipeline like iptvnator's worker-thread parser - this already
/// runs off the async runtime via `spawn_blocking`, so one in-memory pass
/// keeps the UI responsive. Only the first `<display-name>`/`<title>`/
/// `<desc>`/`<category>`/`<icon>` per element is kept (XMLTV's convention of
/// listing localized duplicates).
pub fn parse(xml: &str) -> ParsedEpg {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut result = ParsedEpg::default();
    let mut context = Context::None;
    let mut text_target: Option<TextTarget> = None;

    let mut cur_channel_id: Option<String> = None;
    let mut cur_display_name: Option<String> = None;
    let mut cur_icon: Option<String> = None;

    let mut cur_prog_channel: Option<String> = None;
    let mut cur_prog_start: Option<String> = None;
    let mut cur_prog_stop: Option<String> = None;
    let mut cur_title: Option<String> = None;
    let mut cur_desc: Option<String> = None;
    let mut cur_category: Option<String> = None;
    let mut cur_prog_icon: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = local_name(&e);
                match local.as_str() {
                    "channel" => {
                        context = Context::Channel;
                        cur_channel_id = get_attr(&e, "id");
                        cur_display_name = None;
                        cur_icon = None;
                    }
                    "programme" => {
                        context = Context::Programme;
                        cur_prog_channel = get_attr(&e, "channel");
                        cur_prog_start = get_attr(&e, "start").and_then(|s| parse_xmltv_date(&s));
                        cur_prog_stop = get_attr(&e, "stop").and_then(|s| parse_xmltv_date(&s));
                        cur_title = None;
                        cur_desc = None;
                        cur_category = None;
                        cur_prog_icon = None;
                    }
                    "display-name" if context == Context::Channel && cur_display_name.is_none() => {
                        text_target = Some(TextTarget::DisplayName);
                    }
                    "title" if context == Context::Programme && cur_title.is_none() => {
                        text_target = Some(TextTarget::Title);
                    }
                    "desc" if context == Context::Programme && cur_desc.is_none() => {
                        text_target = Some(TextTarget::Desc);
                    }
                    "category" if context == Context::Programme && cur_category.is_none() => {
                        text_target = Some(TextTarget::Category);
                    }
                    "icon" => apply_icon(&e, &context, &mut cur_icon, &mut cur_prog_icon),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                if local_name(&e) == "icon" {
                    apply_icon(&e, &context, &mut cur_icon, &mut cur_prog_icon);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(target) = &text_target {
                    let text = e.unescape().unwrap_or_default().into_owned();
                    match target {
                        TextTarget::DisplayName => cur_display_name = Some(text),
                        TextTarget::Title => cur_title = Some(text),
                        TextTarget::Desc => cur_desc = Some(text),
                        TextTarget::Category => cur_category = Some(text),
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match local.as_str() {
                    "display-name" | "title" | "desc" | "category" => text_target = None,
                    "channel" => {
                        if let (Some(id), Some(display_name)) =
                            (cur_channel_id.take(), cur_display_name.take())
                        {
                            result.channels.push(ParsedEpgChannel {
                                id,
                                display_name,
                                icon_url: cur_icon.take(),
                            });
                        }
                        context = Context::None;
                    }
                    "programme" => {
                        if let (Some(channel_id), Some(start), Some(stop), Some(title)) = (
                            cur_prog_channel.take(),
                            cur_prog_start.take(),
                            cur_prog_stop.take(),
                            cur_title.take(),
                        ) {
                            result.programs.push(ParsedEpgProgram {
                                channel_id,
                                start,
                                stop,
                                title,
                                description: cur_desc.take(),
                                category: cur_category.take(),
                                icon_url: cur_prog_icon.take(),
                            });
                        }
                        context = Context::None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    result
}

fn local_name(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.name().as_ref()).into_owned()
}

fn apply_icon(
    e: &BytesStart,
    context: &Context,
    cur_icon: &mut Option<String>,
    cur_prog_icon: &mut Option<String>,
) {
    let src = get_attr(e, "src");
    match context {
        Context::Channel if cur_icon.is_none() => *cur_icon = src,
        Context::Programme if cur_prog_icon.is_none() => *cur_prog_icon = src,
        _ => {}
    }
}

fn get_attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

/// XMLTV datetimes look like `YYYYMMDDHHMMSS` optionally followed by a space
/// and a `+HHMM`/`-HHMM` offset (absent offset means already UTC). Returns a
/// normalized UTC RFC3339 string.
pub fn parse_xmltv_date(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.len() < 14 {
        return None;
    }
    let (datetime_part, tz_part) = s.split_at(14);

    let year: i32 = datetime_part[0..4].parse().ok()?;
    let month: u32 = datetime_part[4..6].parse().ok()?;
    let day: u32 = datetime_part[6..8].parse().ok()?;
    let hour: u32 = datetime_part[8..10].parse().ok()?;
    let minute: u32 = datetime_part[10..12].parse().ok()?;
    let second: u32 = datetime_part[12..14].parse().ok()?;

    let naive = NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, second)?;

    let tz_part = tz_part.trim();
    let offset_minutes: i64 = if tz_part.is_empty() {
        0
    } else {
        let sign: i64 = if tz_part.starts_with('-') { -1 } else { 1 };
        let digits = tz_part.trim_start_matches(['+', '-']);
        if digits.len() < 4 {
            0
        } else {
            let hh: i64 = digits[0..2].parse().unwrap_or(0);
            let mm: i64 = digits[2..4].parse().unwrap_or(0);
            sign * (hh * 60 + mm)
        }
    };

    let utc_naive = naive - Duration::minutes(offset_minutes);
    let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(utc_naive, Utc);
    Some(utc_dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_channels_and_programmes() {
        let xml = r#"<?xml version="1.0"?>
<tv>
  <channel id="bbc1.uk">
    <display-name>BBC One</display-name>
    <icon src="http://logo/bbc1.png"/>
  </channel>
  <programme start="20240101120000 +0000" stop="20240101130000 +0000" channel="bbc1.uk">
    <title>News at Noon</title>
    <desc>The latest headlines.</desc>
    <category>News</category>
  </programme>
</tv>"#;
        let parsed = parse(xml);
        assert_eq!(parsed.channels.len(), 1);
        assert_eq!(parsed.channels[0].id, "bbc1.uk");
        assert_eq!(parsed.channels[0].display_name, "BBC One");
        assert_eq!(parsed.channels[0].icon_url.as_deref(), Some("http://logo/bbc1.png"));

        assert_eq!(parsed.programs.len(), 1);
        let program = &parsed.programs[0];
        assert_eq!(program.channel_id, "bbc1.uk");
        assert_eq!(program.title, "News at Noon");
        assert_eq!(program.description.as_deref(), Some("The latest headlines."));
        assert_eq!(program.start, "2024-01-01T12:00:00+00:00");
        assert_eq!(program.stop, "2024-01-01T13:00:00+00:00");
    }

    #[test]
    fn applies_timezone_offset() {
        // 12:00 in +02:00 is 10:00 UTC.
        let parsed_date = parse_xmltv_date("20240101120000 +0200").unwrap();
        assert_eq!(parsed_date, "2024-01-01T10:00:00+00:00");
    }

    #[test]
    fn handles_missing_offset_as_utc() {
        let parsed_date = parse_xmltv_date("20240101120000").unwrap();
        assert_eq!(parsed_date, "2024-01-01T12:00:00+00:00");
    }
}
