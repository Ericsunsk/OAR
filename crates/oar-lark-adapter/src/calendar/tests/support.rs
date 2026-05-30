use crate::calendar::{CalendarFreeBusyBatchRequest, CalendarUserIdType};
use crate::redaction::SecretString;

pub(super) fn sample_request() -> CalendarFreeBusyBatchRequest {
    CalendarFreeBusyBatchRequest {
        user_access_token: SecretString::new("u-very-secret-calendar-token"),
        user_ids: vec!["ou_current_user".to_string()],
        time_min: "2026-05-29T00:00:00Z".to_string(),
        time_max: "2026-05-30T00:00:00Z".to_string(),
        include_external_calendar: false,
        only_busy: true,
        need_rsvp_status: false,
        user_id_type: CalendarUserIdType::OpenId,
    }
}
