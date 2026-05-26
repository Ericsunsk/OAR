pub const AUTH_STATUS_JSON: &str = r#"{
  "code": 0,
  "msg": "ok",
  "data": {
    "user_id": "ou_fake_user",
    "tenant_key": "tk_fake_tenant",
    "scopes": ["offline_access", "auth:user.id:read"]
  }
}"#;

pub const DRY_RUN_PROGRESS_UPDATE_JSON: &str = r#"=== Dry Run ===
{
  "code": 0,
  "msg": "ok",
  "data": {
    "record_id": "prg_fake_record_id",
    "status": 1
  }
}"#;

pub const CYCLE_DETAIL_WITH_STRINGIFIED_CONTENT: &str = r#"{
  "code": 0,
  "msg": "ok",
  "data": {
    "objectives": [
      {
        "id": "obj_fake_1",
        "content": "{\"text\":\"Ship Phase 0.6\"}",
        "key_results": [
          {
            "id": "kr_fake_1",
            "content": "{\"text\":\"Idempotent writes in ledger\"}"
          }
        ]
      }
    ]
  }
}"#;

pub const PROGRESS_LIST_JSON: &str = r#"{
  "code": 0,
  "msg": "ok",
  "data": {
    "progress_list": [
      { "id": "prg_fake_1", "status": "normal" },
      { "id": "prg_fake_2", "status": "normal" }
    ]
  }
}"#;

pub const AUTH_REFRESH_ROTATED_ENCRYPTED_JSON: &str = r#"{
  "outcome": "success",
  "encrypted_primary": [1, 2, 3, 4, 5],
  "encrypted_renewal": [6, 7, 8, 9, 10],
  "key_id": "kms-key-2026-05",
  "new_fingerprint": "fp_rotated_v2",
  "refreshed_at_ms": 1779465600000,
  "expires_at_ms": 1779472800000
}"#;

pub const AUTH_REFRESH_REAUTH_REQUIRED_JSON: &str = r#"{
  "outcome": "reauth_required",
  "safe_error": "invalid_grant"
}"#;

pub const AUTH_REFRESH_TRANSIENT_FAILURE_JSON: &str = r#"{
  "outcome": "transient_failure",
  "safe_error": "temporarily unavailable"
}"#;

pub const AUTH_REFRESH_PLAINTEXT_TOKEN_LEAK_JSON: &str = r#"{
  "code": 0,
  "msg": "ok",
  "data": {
    "access_token": "tok_access_live_should_never_parse",
    "refresh_token": "tok_refresh_live_should_never_parse",
    "token_type": "Bearer",
    "note": "refresh_token=tok_refresh_live_should_never_parse"
  }
}"#;
