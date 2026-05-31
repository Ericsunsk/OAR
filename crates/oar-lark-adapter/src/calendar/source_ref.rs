use std::fmt;

use crate::url_encoding::percent_encode;

const MAX_SOURCE_REF_COMPONENT_CHARS: usize = 256;

#[derive(Clone, PartialEq, Eq)]
pub struct CalendarEventSourceRef {
    pub calendar_id: String,
    pub event_id: String,
}

impl CalendarEventSourceRef {
    pub fn new(calendar_id: impl Into<String>, event_id: impl Into<String>) -> Option<Self> {
        let source_ref = Self {
            calendar_id: calendar_id.into(),
            event_id: event_id.into(),
        };
        if source_ref.valid() {
            Some(source_ref)
        } else {
            None
        }
    }

    pub fn source_ref(&self) -> String {
        format!(
            "calendar://{}/events/{}",
            percent_encode(&self.calendar_id),
            percent_encode(&self.event_id)
        )
    }

    fn valid(&self) -> bool {
        valid_decoded_component(&self.calendar_id) && valid_decoded_component(&self.event_id)
    }
}

impl fmt::Debug for CalendarEventSourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalendarEventSourceRef")
            .field("calendar_id", &"[REDACTED]")
            .field("event_id", &"[REDACTED]")
            .finish()
    }
}

pub fn parse_calendar_event_source_ref(source_ref: &str) -> Option<CalendarEventSourceRef> {
    let trimmed = source_ref.trim();
    if let Some(path_like) = trimmed.strip_prefix("calendar://") {
        return parse_path_style_ref(path_like);
    }
    if let Some(path_like) = trimmed.strip_prefix("feishu://calendar/") {
        return parse_path_style_ref(path_like);
    }
    None
}

fn parse_path_style_ref(value: &str) -> Option<CalendarEventSourceRef> {
    let segments = value.split('/').collect::<Vec<_>>();
    if segments.len() != 3 || segments[1] != "events" {
        return None;
    }
    CalendarEventSourceRef::new(
        decode_component(segments[0])?,
        decode_component(segments[2])?,
    )
}

fn decode_component(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let high = *bytes.get(index + 1)?;
                let low = *bytes.get(index + 2)?;
                decoded.push((hex_value(high)? << 4) | hex_value(low)?);
                index += 3;
            }
            byte if is_unreserved(byte) => {
                decoded.push(byte);
                index += 1;
            }
            _ => return None,
        }
    }

    String::from_utf8(decoded)
        .ok()
        .filter(|decoded| valid_decoded_component(decoded))
}

fn valid_decoded_component(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_SOURCE_REF_COMPONENT_CHARS
        && value
            .chars()
            .all(|character| !character.is_control() && !character.is_whitespace())
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || [b'-', b'_', b'.', b'~'].contains(&byte)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_accepts_calendar_and_feishu_calendar_event_refs() {
        let parsed = parse_calendar_event_source_ref(" calendar://cal_1/events/evt_1 ")
            .expect("calendar source ref");
        assert_eq!(parsed.calendar_id, "cal_1");
        assert_eq!(parsed.event_id, "evt_1");
        assert_eq!(parsed.source_ref(), "calendar://cal_1/events/evt_1");

        let feishu = parse_calendar_event_source_ref("feishu://calendar/cal_2/events/evt_2")
            .expect("feishu calendar source ref");
        assert_eq!(feishu.calendar_id, "cal_2");
        assert_eq!(feishu.event_id, "evt_2");
        assert_eq!(feishu.source_ref(), "calendar://cal_2/events/evt_2");
    }

    #[test]
    fn parser_percent_decodes_components_and_canonicalizes_refs() {
        let parsed = parse_calendar_event_source_ref("calendar://cal%3A1/events/evt%2F1%25x")
            .expect("encoded source ref");

        assert_eq!(parsed.calendar_id, "cal:1");
        assert_eq!(parsed.event_id, "evt/1%x");
        assert_eq!(parsed.source_ref(), "calendar://cal%3A1/events/evt%2F1%25x");
    }

    #[test]
    fn parser_rejects_malformed_or_unsafe_refs() {
        assert!(parse_calendar_event_source_ref("task://task_1").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/event/evt_1").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/evt/1").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/evt?1").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/evt%").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/evt%G0").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/evt%0A1").is_none());
        assert!(parse_calendar_event_source_ref("calendar://cal_1/events/evt%201").is_none());
        assert!(parse_calendar_event_source_ref(&format!(
            "calendar://{}/events/evt_1",
            "x".repeat(MAX_SOURCE_REF_COMPONENT_CHARS + 1)
        ))
        .is_none());
    }

    #[test]
    fn source_ref_debug_redacts_raw_ids() {
        let parsed =
            parse_calendar_event_source_ref("calendar://cal_secret/events/evt_secret").unwrap();

        let debug = format!("{parsed:?}");

        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("cal_secret"));
        assert!(!debug.contains("evt_secret"));
    }
}
