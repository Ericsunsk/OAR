use oar_core::lark::fixtures::{
    AUTH_STATUS_JSON, CYCLE_DETAIL_WITH_STRINGIFIED_CONTENT, DRY_RUN_PROGRESS_UPDATE_JSON,
    PROGRESS_LIST_JSON,
};
use oar_core::lark::parser::{
    normalize_cycle_detail_content, parse_cli_json, progress_list_entries, strip_dry_run_prefix,
};

#[test]
fn strips_dry_run_prefix_before_json_parse() {
    let stripped = strip_dry_run_prefix(DRY_RUN_PROGRESS_UPDATE_JSON);
    assert!(stripped.starts_with('{'));

    let parsed = parse_cli_json(DRY_RUN_PROGRESS_UPDATE_JSON).expect("json should parse");
    assert_eq!(parsed["code"], 0);
    assert_eq!(parsed["data"]["status"], 1);
}

#[test]
fn parses_auth_status_json_without_format_wrapper() {
    let parsed = parse_cli_json(AUTH_STATUS_JSON).expect("auth status JSON should parse");
    assert_eq!(parsed["msg"], "ok");
    assert_eq!(parsed["data"]["user_id"], "ou_fake_user");
}

#[test]
fn normalizes_cycle_detail_content_from_json_string() {
    let mut parsed =
        parse_cli_json(CYCLE_DETAIL_WITH_STRINGIFIED_CONTENT).expect("cycle detail should parse");
    normalize_cycle_detail_content(&mut parsed);

    assert_eq!(parsed["data"]["objectives"][0]["content"]["text"], "Ship Phase 0.6");
    assert_eq!(
        parsed["data"]["objectives"][0]["key_results"][0]["content"]["text"],
        "Idempotent writes in ledger"
    );
}

#[test]
fn extracts_progress_list_entries_from_data_progress_list() {
    let parsed = parse_cli_json(PROGRESS_LIST_JSON).expect("progress list JSON should parse");
    let entries = progress_list_entries(&parsed);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["id"], "prg_fake_1");
}
