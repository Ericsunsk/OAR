mod client;
mod error;
mod feishu_types;
mod response_parser;
mod source_ref;
#[cfg(test)]
mod tests;
mod types;

pub use client::{
    build_event_instance_view_request, build_free_busy_batch_request, build_get_event_request,
    build_primary_calendar_request, AsyncFeishuCalendarRead, FeishuCalendarReadClient,
};
pub use error::FeishuCalendarReadError;
pub use source_ref::{parse_calendar_event_source_ref, CalendarEventSourceRef};
pub use types::{
    CalendarEventInstance, CalendarEventInstancePage, CalendarEventInstanceViewRequest,
    CalendarEventLocation, CalendarEventOrganizer, CalendarEventReadRequest, CalendarEventTimeInfo,
    CalendarFreeBusyBatchRequest, CalendarFreeBusyItem, CalendarFreeBusyList, CalendarFreeBusyPage,
    CalendarPrimaryCalendar, CalendarPrimaryPage, CalendarPrimaryRequest, CalendarUserIdType,
};
