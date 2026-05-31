mod events;
mod free_busy;

pub(super) use events::read_my_calendar_events_summary;
pub(super) use free_busy::read_my_calendar_free_busy_summary;

const CALENDAR_LOOKAHEAD_DAYS: u64 = 7;

fn lookahead_window_text() -> String {
    format!("未来 {CALENDAR_LOOKAHEAD_DAYS} 天")
}
