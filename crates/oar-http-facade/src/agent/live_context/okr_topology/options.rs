const SUMMARY_PAGE_SIZE: u32 = 100;
const PROGRESS_PAGE_SIZE: u32 = 50;
const CYCLE_EXPAND_LIMIT: usize = 3;
const SUMMARY_OBJECTIVE_KEY_RESULT_LIMIT: usize = 8;
const PROGRESS_OBJECTIVE_KEY_RESULT_LIMIT: usize = 10;

#[derive(Clone, Copy)]
pub(in crate::agent::live_context) struct OkrTopologyReadOptions {
    pub(super) cycle_page_size: u32,
    pub(super) objective_page_size: u32,
    pub(super) key_result_page_size: u32,
    pub(super) cycle_expand_limit: usize,
    pub(super) objective_key_result_limit: usize,
}

impl OkrTopologyReadOptions {
    pub(in crate::agent::live_context) fn for_requested_tools(
        read_summary: bool,
        read_progress: bool,
    ) -> Self {
        match (read_summary, read_progress) {
            (true, true) => Self {
                cycle_page_size: SUMMARY_PAGE_SIZE,
                objective_page_size: SUMMARY_PAGE_SIZE,
                key_result_page_size: SUMMARY_PAGE_SIZE,
                cycle_expand_limit: CYCLE_EXPAND_LIMIT,
                objective_key_result_limit: PROGRESS_OBJECTIVE_KEY_RESULT_LIMIT,
            },
            (true, false) => Self {
                cycle_page_size: SUMMARY_PAGE_SIZE,
                objective_page_size: SUMMARY_PAGE_SIZE,
                key_result_page_size: SUMMARY_PAGE_SIZE,
                cycle_expand_limit: CYCLE_EXPAND_LIMIT,
                objective_key_result_limit: SUMMARY_OBJECTIVE_KEY_RESULT_LIMIT,
            },
            (false, true) => Self {
                cycle_page_size: PROGRESS_PAGE_SIZE,
                objective_page_size: PROGRESS_PAGE_SIZE,
                key_result_page_size: PROGRESS_PAGE_SIZE,
                cycle_expand_limit: CYCLE_EXPAND_LIMIT,
                objective_key_result_limit: PROGRESS_OBJECTIVE_KEY_RESULT_LIMIT,
            },
            (false, false) => Self {
                cycle_page_size: PROGRESS_PAGE_SIZE,
                objective_page_size: PROGRESS_PAGE_SIZE,
                key_result_page_size: PROGRESS_PAGE_SIZE,
                cycle_expand_limit: 0,
                objective_key_result_limit: 0,
            },
        }
    }
}
