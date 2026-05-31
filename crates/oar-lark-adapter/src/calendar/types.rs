use std::fmt;

use serde::{Deserialize, Serialize};

use crate::redaction::SecretString;

#[derive(Clone, PartialEq, Eq)]
pub struct CalendarFreeBusyBatchRequest {
    pub user_access_token: SecretString,
    pub user_ids: Vec<String>,
    pub time_min: String,
    pub time_max: String,
    pub include_external_calendar: bool,
    pub only_busy: bool,
    pub need_rsvp_status: bool,
    pub user_id_type: CalendarUserIdType,
}

impl fmt::Debug for CalendarFreeBusyBatchRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalendarFreeBusyBatchRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("user_count", &self.user_ids.len())
            .field("time_min", &self.time_min)
            .field("time_max", &self.time_max)
            .field("include_external_calendar", &self.include_external_calendar)
            .field("only_busy", &self.only_busy)
            .field("need_rsvp_status", &self.need_rsvp_status)
            .field("user_id_type", &self.user_id_type)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CalendarPrimaryRequest {
    pub user_access_token: SecretString,
}

impl fmt::Debug for CalendarPrimaryRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalendarPrimaryRequest")
            .field("user_access_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CalendarEventInstanceViewRequest {
    pub user_access_token: SecretString,
    pub calendar_id: String,
    pub start_time: i64,
    pub end_time: i64,
}

impl fmt::Debug for CalendarEventInstanceViewRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalendarEventInstanceViewRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("calendar_id", &"[REDACTED]")
            .field("start_time", &self.start_time)
            .field("end_time", &self.end_time)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CalendarEventReadRequest {
    pub user_access_token: SecretString,
    pub source_ref: String,
}

impl fmt::Debug for CalendarEventReadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalendarEventReadRequest")
            .field("user_access_token", &"[REDACTED]")
            .field("source_ref", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarUserIdType {
    OpenId,
    UserId,
    UnionId,
}

impl CalendarUserIdType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenId => "open_id",
            Self::UserId => "user_id",
            Self::UnionId => "union_id",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarFreeBusyPage {
    pub lists: Vec<CalendarFreeBusyList>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarFreeBusyList {
    pub busy_items: Vec<CalendarFreeBusyItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarFreeBusyItem {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarPrimaryPage {
    pub calendar: CalendarPrimaryCalendar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarPrimaryCalendar {
    pub calendar_id: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventInstancePage {
    pub events: Vec<CalendarEventInstance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventInstance {
    pub summary: Option<String>,
    pub start_time_info: Option<CalendarEventTimeInfo>,
    pub end_time_info: Option<CalendarEventTimeInfo>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub free_busy_status: Option<String>,
    pub location: Option<CalendarEventLocation>,
    pub organizer: Option<CalendarEventOrganizer>,
    pub attendee_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventTimeInfo {
    pub timestamp: Option<String>,
    pub timezone: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventLocation {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventOrganizer {
    pub display_name: Option<String>,
}

pub(super) fn valid_calendar_user_id(user_id: &str) -> bool {
    !user_id.is_empty()
        && user_id.len() <= 100
        && !user_id.contains('/')
        && !user_id.contains('?')
        && !user_id.contains('#')
        && user_id
            .chars()
            .all(|character| !character.is_whitespace() && !character.is_control())
}

pub(super) fn valid_calendar_id(calendar_id: &str) -> bool {
    let trimmed = calendar_id.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 256
        && !trimmed.chars().any(|character| character.is_control())
}

pub(super) fn valid_rfc3339ish_time(value: &str) -> bool {
    let trimmed = value.trim();
    let Some(time_separator_index) = trimmed.find('T') else {
        return false;
    };
    let time_and_zone = &trimmed[time_separator_index + 1..];
    !trimmed.is_empty()
        && trimmed.len() <= 64
        && !trimmed.chars().any(|character| character.is_control())
        && (time_and_zone.ends_with('Z')
            || time_and_zone.contains('+')
            || time_and_zone.rfind('-').is_some())
}
