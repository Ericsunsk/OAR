use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use super::types::{
    valid_calendar_user_id, CalendarEventInstance, CalendarEventInstancePage,
    CalendarEventLocation, CalendarEventOrganizer, CalendarEventTimeInfo, CalendarFreeBusyItem,
    CalendarFreeBusyList, CalendarFreeBusyPage, CalendarPrimaryCalendar, CalendarPrimaryPage,
};

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
