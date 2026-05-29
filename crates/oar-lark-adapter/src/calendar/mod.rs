mod client;
mod error;
#[cfg(test)]
mod tests;
mod types;

pub use client::{
    build_free_busy_batch_request, AsyncFeishuCalendarRead, FeishuCalendarReadClient,
};
pub use error::FeishuCalendarReadError;
pub use types::{
    CalendarFreeBusyBatchRequest, CalendarFreeBusyItem, CalendarFreeBusyList, CalendarFreeBusyPage,
    CalendarUserIdType,
};
