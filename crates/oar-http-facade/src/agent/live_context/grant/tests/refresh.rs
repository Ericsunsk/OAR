use super::*;

#[test]
fn expired_grant_triggers_refresh_path_before_live_read() {
    let now = UNIX_EPOCH + Duration::from_secs(10);
    let now_ms = system_time_to_ms(now);
    let grant = sample_token_grant_record(TokenGrantState::Valid, Some(now_ms - 1));
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![1, 2, 3],
            encrypted_renewal: vec![4, 5, 6],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: now,
        expires_at: Some(now + Duration::from_secs(3600)),
    });
    let sink = FakeCommandSink::new(Ok(Some(TokenRefreshApplyResult {
        grant_id: TokenGrantId(grant.id.clone()),
        tenant_id: TenantId(grant.tenant_id.clone()),
        state: TokenGrantState::Valid,
        fingerprint: "fp_new".to_string(),
    })));

    let report =
        refresh_if_stale_for_test(&grant, now, adapter.clone(), sink.clone()).expect("refresh");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::RotateGrantCas)
    );
    assert!(ensure_refresh_report_allows_read(&report).is_ok());
}

#[test]
fn refresh_failure_safely_degrades_live_read() {
    let now = UNIX_EPOCH + Duration::from_secs(10);
    let now_ms = system_time_to_ms(now);
    let grant = sample_token_grant_record(TokenGrantState::Expired, Some(now_ms - 1));
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
        safe_error: "raw-access-token-sensitive".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(TokenRefreshApplyResult {
        grant_id: TokenGrantId(grant.id.clone()),
        tenant_id: TenantId(grant.tenant_id.clone()),
        state: TokenGrantState::NeedsRefresh,
        fingerprint: "fp_old".to_string(),
    })));

    let report =
        refresh_if_stale_for_test(&grant, now, adapter.clone(), sink.clone()).expect("refresh");
    let error = ensure_refresh_report_allows_read(&report).expect_err("degrade");
    let debug = format!("{report:?} {error:?}");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkNeedsRefresh)
    );
    assert_eq!(error.safe_reason(), "授权令牌刷新失败");
    assert!(!debug.contains("raw-access-token-sensitive"));
}
