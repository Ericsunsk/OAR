use oar_core::storage::postgres::operational_recovery_sql::{
    LIST_FAILED_AUDIT_OUTBOX_RECOVERY_ITEMS, LIST_PARKED_TOKEN_GRANT_RECOVERY_ITEMS,
};

use super::compact;

#[test]
fn operational_recovery_sql_is_readonly_tenant_scoped_and_avoids_secret_columns() {
    for sql in [
        LIST_FAILED_AUDIT_OUTBOX_RECOVERY_ITEMS,
        LIST_PARKED_TOKEN_GRANT_RECOVERY_ITEMS,
    ] {
        let normalized = compact(sql);
        assert!(
            normalized.starts_with("select "),
            "recovery report query must be readonly"
        );
        for forbidden_write in ["insert ", "update ", "delete ", " for update"] {
            assert!(
                !normalized.contains(forbidden_write),
                "recovery report query must not contain write/lock marker: {forbidden_write}"
            );
        }
        assert!(
            normalized.contains("where tenant_id = $1"),
            "recovery report query must be tenant scoped"
        );
        for forbidden in [
            "encrypted_oauth_grant",
            "oauth_grant_key_id",
            "oauth_grant_fingerprint",
            "authorization",
            "access_token",
            "refresh_token",
        ] {
            assert!(
                !normalized.contains(forbidden),
                "recovery report query selected forbidden marker: {forbidden}"
            );
        }
    }
}

#[test]
fn operational_recovery_sql_targets_only_terminal_or_parked_rows() {
    let outbox = compact(LIST_FAILED_AUDIT_OUTBOX_RECOVERY_ITEMS);
    assert!(outbox.contains("status = 'failed'"));
    assert!(outbox.contains("stream = 'audit-events'"));
    assert!(outbox.contains("sent_at is null"));

    let grants = compact(LIST_PARKED_TOKEN_GRANT_RECOVERY_ITEMS);
    assert!(grants.contains("state = 'reauth_required'"));
    assert!(grants.contains("state in ('valid', 'needs_refresh', 'expired')"));
    assert!(grants.contains("revoked_at is null"));
    assert!(grants.contains("reauth_required_at is null"));
    assert!(grants.contains("'refresh_config_required'"));
    assert!(grants.contains("'auth_refresh_parse_failed'"));
    assert!(grants.contains("'auth_refresh_oversized_response'"));
}
