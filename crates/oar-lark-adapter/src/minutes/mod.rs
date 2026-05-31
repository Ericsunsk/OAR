mod client;
mod error;
mod feishu_types;
mod response_parser;
mod source_ref;
#[cfg(test)]
mod tests;
mod types;

pub use client::{
    build_get_minute_request, build_search_minutes_request, AsyncFeishuMinutesRead,
    FeishuMinutesReadClient,
};
pub use error::FeishuMinutesReadError;
pub use source_ref::{parse_minutes_source_ref, MinutesSourceRef};
pub use types::{
    FeishuMinuteReadRequest, FeishuMinuteSearchRequest, MinuteReadSummary, MinuteSearchPage,
};
