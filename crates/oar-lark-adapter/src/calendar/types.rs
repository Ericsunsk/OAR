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
