use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

use oar_core::domain::identity::{
    ActorKind, LarkIdentityId, OAuthTokens, ScopeBoundary, SecretString, TenantId, TokenGrant,
    TokenGrantId, TokenGrantState,
};
use oar_core::domain::token_refresh::{
    decide_token_refresh, is_refreshable, plan_token_refresh_command,
    token_refresh_short_circuit_report, AuthRefreshAdapter, EncryptedGrantMaterial, RefreshOutcome,
    TokenRefreshApplyResult, TokenRefreshAttempt, TokenRefreshBridgeError, TokenRefreshCommandKind,
    TokenRefreshCommandSink, TokenRefreshDecision, TokenRefreshDecisionKind,
    TokenRefreshGrantSnapshot, TokenRefreshReportStatus, TokenRefreshRepositoryCommand,
    TokenRefreshService, TokenRefreshServiceError, TokenRefreshShortCircuitReason,
};

fn sample_grant(state: TokenGrantState, refresh_token: Option<&str>) -> TokenGrant {
    TokenGrant {
        id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        identity_id: LarkIdentityId("identity_01".to_string()),
        actor_kind: ActorKind::User,
        scope_boundary: ScopeBoundary::User,
        scopes: vec!["offline_access".to_string()],
        state,
        issued_at: SystemTime::UNIX_EPOCH,
        expires_at: Some(SystemTime::UNIX_EPOCH),
        refreshed_at: None,
        revoked_at: None,
        reauth_required_at: None,
        last_refresh_error: None,
        tokens: OAuthTokens {
            access_token: SecretString::new("access-old"),
            refresh_token: refresh_token.map(SecretString::new),
        },
        revocation_reason: None,
    }
}

fn sample_snapshot(grant: &TokenGrant) -> TokenRefreshGrantSnapshot {
    TokenRefreshGrantSnapshot::from_grant(grant, "fp_old")
}

#[derive(Clone)]
struct FakeAuthRefreshAdapter {
    state: Rc<RefCell<FakeAuthRefreshState>>,
}

#[derive(Clone)]
struct FakeAuthRefreshState {
    calls: usize,
    outcome: RefreshOutcome,
}

impl FakeAuthRefreshAdapter {
    fn new(outcome: RefreshOutcome) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeAuthRefreshState { calls: 0, outcome })),
        }
    }

    fn calls(&self) -> usize {
        self.state.borrow().calls
    }
}

impl AuthRefreshAdapter for FakeAuthRefreshAdapter {
    fn refresh(&mut self, _snapshot: &TokenRefreshGrantSnapshot) -> RefreshOutcome {
        let mut state = self.state.borrow_mut();
        state.calls += 1;
        state.outcome.clone()
    }
}

#[derive(Clone)]
struct FakeCommandSink {
    state: Rc<RefCell<FakeCommandSinkState>>,
}

#[derive(Clone)]
struct FakeCommandSinkState {
    calls: usize,
    last_command: Option<TokenRefreshRepositoryCommand>,
    result: Result<Option<TokenRefreshApplyResult>, ()>,
}

impl FakeCommandSink {
    fn new(result: Result<Option<TokenRefreshApplyResult>, ()>) -> Self {
        Self {
            state: Rc::new(RefCell::new(FakeCommandSinkState {
                calls: 0,
                last_command: None,
                result,
            })),
        }
    }

    fn calls(&self) -> usize {
        self.state.borrow().calls
    }

    fn last_command(&self) -> Option<TokenRefreshRepositoryCommand> {
        self.state.borrow().last_command.clone()
    }
}

impl TokenRefreshCommandSink for FakeCommandSink {
    type Error = ();

    fn apply_refresh_command(
        &mut self,
        command: TokenRefreshRepositoryCommand,
    ) -> Result<Option<TokenRefreshApplyResult>, Self::Error> {
        let mut state = self.state.borrow_mut();
        state.calls += 1;
        state.last_command = Some(command);
        state.result.clone()
    }
}

fn sample_apply_result(state: TokenGrantState, fingerprint: &str) -> TokenRefreshApplyResult {
    TokenRefreshApplyResult {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        state,
        fingerprint: fingerprint.to_string(),
    }
}

#[test]
fn decide_success_requires_cas_rotation_payload() {
    let now = SystemTime::UNIX_EPOCH;
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::Success {
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![1, 2, 3],
                encrypted_renewal: vec![4, 5, 6],
            },
            key_id: "key_v2".to_string(),
            new_fingerprint: "fp_new".to_string(),
            refreshed_at: now,
            expires_at: None,
        },
    };

    let decision = decide_token_refresh(attempt);

    assert_eq!(
        decision,
        TokenRefreshDecision::RotateGrantCas {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            rotated_material: EncryptedGrantMaterial {
                encrypted_primary: vec![1, 2, 3],
                encrypted_renewal: vec![4, 5, 6],
            },
            key_id: "key_v2".to_string(),
            new_fingerprint: "fp_new".to_string(),
            refreshed_at: now,
            expires_at: None,
        }
    );
}

#[test]
fn decide_transient_failure_marks_needs_refresh() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        },
    };

    let decision = decide_token_refresh(attempt);
    assert_eq!(
        decision,
        TokenRefreshDecision::MarkNeedsRefresh {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            safe_error: "temporarily unavailable".to_string(),
        }
    );
}

#[test]
fn decide_failure_redacts_token_like_safe_error_before_report_or_persistence() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::TransientFailure {
            safe_error: "eyJhbGciOiJIUzI1NiJ9.fake.payload".to_string(),
        },
    };

    let decision = decide_token_refresh(attempt);

    assert_eq!(decision.safe_error(), Some("<redacted refresh error>"));
    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH)
        .expect("command should be safe to build");
    match command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. } => {
            assert_eq!(safe_error, "<redacted refresh error>");
        }
        other => panic!("expected MarkNeedsRefresh, got {other:?}"),
    }
}

#[test]
fn decide_failure_preserves_allowlisted_safe_error_code() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        },
    };

    let decision = decide_token_refresh(attempt);

    assert_eq!(decision.safe_error(), Some("temporarily unavailable"));
}

#[test]
fn decide_reauth_failure_marks_reauth_required() {
    let attempt = TokenRefreshAttempt {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        outcome: RefreshOutcome::ReauthFailure {
            safe_error: "invalid_grant".to_string(),
        },
    };

    let decision = decide_token_refresh(attempt);
    assert_eq!(
        decision,
        TokenRefreshDecision::MarkReauthRequired {
            grant_id: TokenGrantId("grant_01".to_string()),
            tenant_id: TenantId("tenant_01".to_string()),
            expected_fingerprint: "fp_old".to_string(),
            safe_error: "invalid_grant".to_string(),
        }
    );
}

#[test]
fn revoked_and_reauth_required_grants_are_not_refreshable() {
    let revoked = sample_grant(TokenGrantState::Revoked, Some("refresh-old"));
    let reauth_required = sample_grant(TokenGrantState::ReauthRequired, Some("refresh-old"));

    assert!(!is_refreshable(&revoked));
    assert!(!is_refreshable(&reauth_required));
}

#[test]
fn grant_without_refresh_material_is_not_refreshable() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, None);
    assert!(!is_refreshable(&grant));
}

#[test]
fn grant_with_revoked_or_reauth_timestamp_is_not_refreshable() {
    let mut revoked_ts = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    revoked_ts.revoked_at = Some(SystemTime::UNIX_EPOCH);
    assert!(!is_refreshable(&revoked_ts));

    let mut reauth_ts = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    reauth_ts.reauth_required_at = Some(SystemTime::UNIX_EPOCH);
    assert!(!is_refreshable(&reauth_ts));
}

#[test]
fn encrypted_material_debug_redacts_payload() {
    let material = EncryptedGrantMaterial {
        encrypted_primary: vec![9, 9, 9],
        encrypted_renewal: vec![8, 8, 8],
    };
    let debug_output = format!("{material:?}");
    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("9, 9, 9"));
    assert!(!debug_output.contains("8, 8, 8"));
}

#[test]
fn decision_bridge_maps_rotate_command_and_preserves_cas_fields() {
    let refreshed_at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(10);
    let expires_at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(20);
    let decision = TokenRefreshDecision::RotateGrantCas {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![1, 2, 3],
            encrypted_renewal: vec![4, 5],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at,
        expires_at: Some(expires_at),
    };

    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH)
        .expect("bridge command");

    match command {
        TokenRefreshRepositoryCommand::RotateGrantCas {
            grant_id,
            tenant_id,
            expected_fingerprint,
            expires_at_ms,
            refreshed_at_ms,
            encrypted_grant_blob,
            grant_key_id,
            new_fingerprint,
        } => {
            assert_eq!(grant_id, TokenGrantId("grant_01".to_string()));
            assert_eq!(tenant_id, TenantId("tenant_01".to_string()));
            assert_eq!(expected_fingerprint, "fp_old");
            assert_eq!(expires_at_ms, Some(20_000));
            assert_eq!(refreshed_at_ms, 10_000);
            assert_eq!(grant_key_id, "key_v2");
            assert_eq!(new_fingerprint, "fp_new");
            assert_eq!(
                encrypted_grant_blob.0,
                vec![0, 0, 0, 3, 1, 2, 3, 0, 0, 0, 2, 4, 5]
            );
        }
        _ => panic!("expected RotateGrantCas command"),
    }
}

#[test]
fn decision_bridge_failure_commands_use_now_and_keep_expected_fingerprint() {
    let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(33);
    let needs_refresh = TokenRefreshDecision::MarkNeedsRefresh {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        safe_error: "temporarily unavailable".to_string(),
    };
    let reauth = TokenRefreshDecision::MarkReauthRequired {
        grant_id: TokenGrantId("grant_02".to_string()),
        tenant_id: TenantId("tenant_02".to_string()),
        expected_fingerprint: "fp_current".to_string(),
        safe_error: "invalid_grant".to_string(),
    };

    let needs_refresh_command = needs_refresh
        .into_repository_command_at(now)
        .expect("needs refresh command");
    let reauth_command = reauth
        .into_repository_command_at(now)
        .expect("reauth command");

    match needs_refresh_command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            expected_fingerprint,
            refreshed_at_ms,
            ..
        } => {
            assert_eq!(expected_fingerprint, "fp_old");
            assert_eq!(refreshed_at_ms, 33_000);
        }
        _ => panic!("expected MarkNeedsRefresh command"),
    }

    match reauth_command {
        TokenRefreshRepositoryCommand::MarkReauthRequired {
            expected_fingerprint,
            reauth_required_at_ms,
            ..
        } => {
            assert_eq!(expected_fingerprint, "fp_current");
            assert_eq!(reauth_required_at_ms, 33_000);
        }
        _ => panic!("expected MarkReauthRequired command"),
    }
}

#[test]
fn decision_bridge_and_blob_debug_redact_bytes() {
    let decision = TokenRefreshDecision::RotateGrantCas {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![9, 9, 9],
            encrypted_renewal: vec![8, 8, 8],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH,
        expires_at: None,
    };

    let command = decision
        .into_repository_command_at(SystemTime::UNIX_EPOCH)
        .expect("bridge command");
    let debug_output = format!("{command:?}");

    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("9, 9, 9"));
    assert!(!debug_output.contains("8, 8, 8"));
}

#[test]
fn plan_token_refresh_command_builds_command_and_report_from_one_decision() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let planned = plan_token_refresh_command(
        &snapshot,
        RefreshOutcome::TransientFailure {
            safe_error: "temporarily unavailable".to_string(),
        },
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(21),
    )
    .expect("planned command should be built");

    assert_eq!(
        planned.report.decision_kind,
        TokenRefreshDecisionKind::MarkNeedsRefresh
    );
    assert_eq!(
        planned.report.command_kind,
        TokenRefreshCommandKind::MarkNeedsRefresh
    );
    assert_eq!(
        planned.report.safe_error.as_deref(),
        Some("temporarily unavailable")
    );

    match planned.command {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            assert_eq!(grant_id, TokenGrantId("grant_01".to_string()));
            assert_eq!(tenant_id, TenantId("tenant_01".to_string()));
            assert_eq!(expected_fingerprint, "fp_old");
            assert_eq!(refreshed_at_ms, 21_000);
            assert_eq!(safe_error, "temporarily unavailable");
        }
        other => panic!("expected MarkNeedsRefresh command, got {other:?}"),
    }
}

#[test]
fn short_circuit_report_is_the_standard_non_adapter_report() {
    let grant = sample_grant(TokenGrantState::ReauthRequired, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);

    let report = token_refresh_short_circuit_report(&snapshot).expect("should short-circuit");

    assert_eq!(
        report.status,
        TokenRefreshReportStatus::ShortCircuited(TokenRefreshShortCircuitReason::ReauthRequired)
    );
    assert!(!report.adapter_called);
    assert!(!report.sink_called);
    assert_eq!(report.decision, None);
    assert_eq!(report.command, None);
}

#[test]
fn decision_bridge_rejects_pre_epoch_timestamps() {
    let decision = TokenRefreshDecision::MarkNeedsRefresh {
        grant_id: TokenGrantId("grant_01".to_string()),
        tenant_id: TenantId("tenant_01".to_string()),
        expected_fingerprint: "fp_old".to_string(),
        safe_error: "temporarily unavailable".to_string(),
    };

    let before_epoch = SystemTime::UNIX_EPOCH - std::time::Duration::from_secs(1);
    let err = decision
        .into_repository_command_at(before_epoch)
        .expect_err("pre-epoch should fail");
    assert_eq!(err, TokenRefreshBridgeError::TimestampBeforeUnixEpoch);
}

#[test]
fn service_success_path_calls_adapter_and_sink_once_and_reports_success() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![1, 2, 3],
            encrypted_renewal: vec![4, 5, 6],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
        expires_at: None,
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::Valid,
        "fp_new",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(
            snapshot,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3),
        )
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::RotateGrantCas)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::RotateGrantCas)
    );
    assert_eq!(report.safe_error, None);

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::RotateGrantCas { .. } => {}
        other => panic!("expected RotateGrantCas, got {other:?}"),
    }
}

#[test]
fn service_transient_failure_marks_needs_refresh() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
        safe_error: "temporarily unavailable".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::NeedsRefresh,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(
            snapshot,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(8),
        )
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::MarkNeedsRefresh)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkNeedsRefresh)
    );
    assert_eq!(
        report.safe_error.as_deref(),
        Some("temporarily unavailable")
    );

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. } => {
            assert_eq!(safe_error, "temporarily unavailable");
        }
        other => panic!("expected MarkNeedsRefresh, got {other:?}"),
    }
}

#[test]
fn service_reauth_failure_marks_reauth_required() {
    let grant = sample_grant(TokenGrantState::Valid, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::ReauthFailure {
        safe_error: "invalid_grant".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::ReauthRequired,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(
            snapshot,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(13),
        )
        .expect("service refresh should succeed");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::Succeeded);
    assert_eq!(
        report.decision,
        Some(TokenRefreshDecisionKind::MarkReauthRequired)
    );
    assert_eq!(
        report.command,
        Some(TokenRefreshCommandKind::MarkReauthRequired)
    );
    assert_eq!(report.safe_error.as_deref(), Some("invalid_grant"));

    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkReauthRequired { safe_error, .. } => {
            assert_eq!(safe_error, "invalid_grant");
        }
        other => panic!("expected MarkReauthRequired, got {other:?}"),
    }
}

#[test]
fn service_short_circuits_revoked_reauth_and_missing_refresh_material() {
    let cases = [
        (
            sample_grant(TokenGrantState::Revoked, Some("refresh-old")),
            TokenRefreshShortCircuitReason::Revoked,
        ),
        (
            sample_grant(TokenGrantState::ReauthRequired, Some("refresh-old")),
            TokenRefreshShortCircuitReason::ReauthRequired,
        ),
        (
            sample_grant(TokenGrantState::NeedsRefresh, None),
            TokenRefreshShortCircuitReason::MissingRefreshMaterial,
        ),
    ];

    for (grant, expected_reason) in cases {
        let snapshot = sample_snapshot(&grant);
        let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
            safe_error: "not-used".to_string(),
        });
        let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
            TokenGrantState::NeedsRefresh,
            "fp_old",
        ))));
        let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

        let report = service
            .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH)
            .expect("short-circuit should not fail");
        assert_eq!(
            report.status,
            TokenRefreshReportStatus::ShortCircuited(expected_reason)
        );
        assert_eq!(report.decision, None);
        assert_eq!(report.command, None);
        assert_eq!(adapter.calls(), 0);
        assert_eq!(sink.calls(), 0);
    }
}

#[test]
fn service_reports_conflict_noop_when_sink_returns_none() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![7, 7, 7],
            encrypted_renewal: vec![8, 8, 8],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
        expires_at: None,
    });
    let sink = FakeCommandSink::new(Ok(None));
    let mut service = TokenRefreshService::new(adapter.clone(), sink.clone());

    let report = service
        .refresh_grant_at(
            snapshot,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(5),
        )
        .expect("service refresh should not fail");

    assert_eq!(adapter.calls(), 1);
    assert_eq!(sink.calls(), 1);
    assert_eq!(report.status, TokenRefreshReportStatus::ConflictNoop);
}

#[test]
fn service_report_and_audit_summary_do_not_leak_tokens_or_encrypted_bytes() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::Success {
        rotated_material: EncryptedGrantMaterial {
            encrypted_primary: vec![9, 9, 9],
            encrypted_renewal: vec![8, 8, 8],
        },
        key_id: "key_v2".to_string(),
        new_fingerprint: "fp_new".to_string(),
        refreshed_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
        expires_at: None,
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::Valid,
        "fp_new",
    ))));
    let mut service = TokenRefreshService::new(adapter, sink);

    let report = service
        .refresh_grant_at(
            snapshot,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
        )
        .expect("service refresh should succeed");
    let audit = report.audit_summary();
    let report_debug = format!("{report:?}");
    let audit_debug = format!("{audit:?}");

    assert!(!report_debug.contains("access-old"));
    assert!(!report_debug.contains("refresh-old"));
    assert!(!report_debug.contains("9, 9, 9"));
    assert!(!report_debug.contains("8, 8, 8"));
    assert!(!audit_debug.contains("access-old"));
    assert!(!audit_debug.contains("refresh-old"));
    assert!(!audit_debug.contains("9, 9, 9"));
    assert!(!audit_debug.contains("8, 8, 8"));
}

#[test]
fn service_report_redacts_token_like_adapter_errors() {
    let grant = sample_grant(TokenGrantState::NeedsRefresh, Some("refresh-old"));
    let snapshot = sample_snapshot(&grant);
    let adapter = FakeAuthRefreshAdapter::new(RefreshOutcome::TransientFailure {
        safe_error: "opaque-token-fragment-without-keyword".to_string(),
    });
    let sink = FakeCommandSink::new(Ok(Some(sample_apply_result(
        TokenGrantState::NeedsRefresh,
        "fp_old",
    ))));
    let mut service = TokenRefreshService::new(adapter, sink.clone());

    let report = service
        .refresh_grant_at(snapshot, SystemTime::UNIX_EPOCH)
        .expect("service refresh should succeed");
    let audit = report.audit_summary();

    assert_eq!(
        report.safe_error.as_deref(),
        Some("<redacted refresh error>")
    );
    assert_eq!(
        audit.safe_error.as_deref(),
        Some("<redacted refresh error>")
    );
    match sink.last_command().expect("expected command") {
        TokenRefreshRepositoryCommand::MarkNeedsRefresh { safe_error, .. } => {
            assert_eq!(safe_error, "<redacted refresh error>");
        }
        other => panic!("expected MarkNeedsRefresh, got {other:?}"),
    }
    assert!(!format!("{report:?}").contains("opaque-token-fragment"));
    assert!(!format!("{audit:?}").contains("opaque-token-fragment"));
}

#[test]
fn service_error_redacts_command_sink_error_outputs() {
    let error: TokenRefreshServiceError<String> =
        TokenRefreshServiceError::CommandSink("refresh-token-secret".to_string());

    assert_eq!(error.to_string(), "token refresh command sink failed");
    assert!(format!("{error:?}").contains("[REDACTED]"));
    assert!(!format!("{error:?}").contains("refresh-token-secret"));
    assert!(std::error::Error::source(&error).is_none());
}
