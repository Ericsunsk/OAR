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
