use std::fmt;

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};

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
            .field("calendar_id", &self.calendar_id)
            .field("start_time", &self.start_time)
            .field("end_time", &self.end_time)
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

#[derive(Debug, Deserialize)]
pub(super) struct FeishuFreeBusyBatchResponse {
    pub code: i64,
    pub data: Option<FeishuFreeBusyBatchData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuFreeBusyBatchData {
    #[serde(default)]
    pub freebusy_lists: Vec<FeishuFreeBusyList>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuFreeBusyList {
    pub user_id: Option<String>,
    #[serde(default)]
    pub freebusy_items: Vec<FeishuFreeBusyItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuFreeBusyItem {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuPrimaryCalendarResponse {
    pub code: i64,
    pub data: Option<FeishuPrimaryCalendarData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuPrimaryCalendarData {
    #[serde(default)]
    pub calendars: Vec<FeishuPrimaryCalendarItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuPrimaryCalendarItem {
    pub calendar: Option<FeishuPrimaryCalendar>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuPrimaryCalendar {
    pub calendar_id: Option<String>,
    #[serde(alias = "name")]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuEventInstanceViewResponse {
    pub code: i64,
    pub data: Option<FeishuEventInstanceViewData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuEventInstanceViewData {
    #[serde(default, alias = "items", alias = "event_instances")]
    pub instances: Vec<FeishuEventInstance>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuEventInstance {
    #[serde(alias = "id", alias = "uid")]
    pub event_id: Option<String>,
    #[serde(alias = "title")]
    pub summary: Option<String>,
    #[serde(alias = "start_time", alias = "start")]
    pub start_time_info: Option<FeishuEventTimeInfo>,
    #[serde(alias = "end_time", alias = "end")]
    pub end_time_info: Option<FeishuEventTimeInfo>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub free_busy_status: Option<String>,
    pub location: Option<FeishuEventLocation>,
    #[serde(alias = "event_organizer")]
    pub organizer: Option<FeishuEventOrganizer>,
    #[serde(
        default,
        rename = "attendees",
        deserialize_with = "deserialize_attendee_count"
    )]
    pub attendee_count: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuEventTimeInfo {
    pub timestamp: Option<String>,
    pub timezone: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuEventLocation {
    #[serde(alias = "display_name")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FeishuEventOrganizer {
    #[serde(alias = "name")]
    pub display_name: Option<String>,
}

impl CalendarFreeBusyPage {
    pub(super) fn from_feishu_data(data: FeishuFreeBusyBatchData) -> Self {
        let lists = data
            .freebusy_lists
            .into_iter()
            .filter_map(|list| {
                let user_id = non_empty(list.user_id)?;
                if !valid_calendar_user_id(&user_id) {
                    return None;
                }
                let busy_items = list
                    .freebusy_items
                    .into_iter()
                    .map(|item| CalendarFreeBusyItem {
                        start_time: non_empty(item.start_time),
                        end_time: non_empty(item.end_time),
                    })
                    .collect::<Vec<_>>();
                Some(CalendarFreeBusyList { busy_items })
            })
            .collect::<Vec<_>>();
        Self { lists }
    }
}

impl CalendarPrimaryPage {
    pub(super) fn from_feishu_data(data: FeishuPrimaryCalendarData) -> Option<Self> {
        let FeishuPrimaryCalendarData { calendars } = data;
        let (calendar_id, summary) = calendars
            .into_iter()
            .filter_map(|item| item.calendar)
            .find_map(primary_calendar_parts)?;
        Some(Self {
            calendar: CalendarPrimaryCalendar {
                calendar_id,
                summary: non_empty(summary),
            },
        })
    }
}

fn primary_calendar_parts(calendar: FeishuPrimaryCalendar) -> Option<(String, Option<String>)> {
    Some((
        non_empty(calendar.calendar_id)?,
        non_empty(calendar.summary),
    ))
}

impl CalendarEventInstancePage {
    pub(super) fn from_feishu_data(data: FeishuEventInstanceViewData) -> Self {
        let events = data
            .instances
            .into_iter()
            .filter_map(CalendarEventInstance::from_feishu_instance)
            .collect::<Vec<_>>();
        Self { events }
    }
}

impl CalendarEventInstance {
    fn from_feishu_instance(instance: FeishuEventInstance) -> Option<Self> {
        non_empty(instance.event_id)?;
        Some(Self {
            summary: non_empty(instance.summary),
            start_time_info: instance
                .start_time_info
                .map(CalendarEventTimeInfo::from_feishu),
            end_time_info: instance
                .end_time_info
                .map(CalendarEventTimeInfo::from_feishu),
            status: non_empty(instance.status),
            visibility: non_empty(instance.visibility),
            free_busy_status: non_empty(instance.free_busy_status),
            location: instance.location.map(CalendarEventLocation::from_feishu),
            organizer: instance.organizer.map(CalendarEventOrganizer::from_feishu),
            attendee_count: instance.attendee_count,
        })
    }
}

impl CalendarEventTimeInfo {
    fn from_feishu(time_info: FeishuEventTimeInfo) -> Self {
        Self {
            timestamp: non_empty(time_info.timestamp),
            timezone: non_empty(time_info.timezone),
            date: non_empty(time_info.date),
        }
    }
}

impl CalendarEventLocation {
    fn from_feishu(location: FeishuEventLocation) -> Self {
        Self {
            name: non_empty(location.name),
        }
    }
}

impl CalendarEventOrganizer {
    fn from_feishu(organizer: FeishuEventOrganizer) -> Self {
        Self {
            display_name: non_empty(organizer.display_name),
        }
    }
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

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn deserialize_attendee_count<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let attendees = Option::<Vec<IgnoredAny>>::deserialize(deserializer)?;
    Ok(attendees.map_or(0, |attendees| attendees.len()))
}
