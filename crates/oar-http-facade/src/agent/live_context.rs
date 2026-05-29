use std::collections::BTreeSet;
use std::time::SystemTime;

use oar_core::storage::postgres::{EncryptedTokenGrantRecord, PostgresTokenGrantRepository};
use oar_lark_adapter::material::read_access_token_from_encrypted_grant;
use oar_lark_adapter::{
    AsyncFeishuOkrRead, AsyncFeishuTaskRead, FeishuOkrBatchGetRequest, FeishuOkrReadClient,
    FeishuTaskGetRequest, FeishuTaskReadClient, OkrReadSnapshot, OkrUserIdType,
    ReqwestAsyncHttpClient, TaskUserIdType,
};

use super::request::{AgentEvidenceRefDTO, AgentStreamRequest};
use crate::{AuthenticatedContext, OarHttpFacadeRuntime};

mod grant;
mod refs;
mod summary;

use grant::{
    grant_requires_refresh_before_read, live_read_grant_denial_reason,
    refresh_grant_before_live_read, resolve_grant_id_for_user, system_time_to_ms,
};
use refs::{
    gate_refs_by_scope, is_okr_source_type, is_task_source_type, parse_okr_evidence_ref,
    parse_task_evidence_ref, ParsedOkrEvidenceRef, ParsedTaskEvidenceRef,
};
use summary::{
    build_live_summary, build_task_live_summary, degraded_summary, task_read_error_reason,
};

const LIVE_EVIDENCE_REF_LIMIT: usize = 4;

pub(crate) async fn inject_live_feishu_context(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    request: &mut AgentStreamRequest,
) {
    let summaries =
        assemble_live_feishu_summaries(runtime, auth_context, &request.context.evidence_refs).await;
    request.context.live_feishu_read_summaries = summaries;
}

async fn assemble_live_feishu_summaries(
    runtime: &OarHttpFacadeRuntime,
    auth_context: &AuthenticatedContext,
    evidence_refs: &[AgentEvidenceRefDTO],
) -> Vec<String> {
    if evidence_refs.is_empty() {
        return vec![];
    }

    let mut degraded = Vec::new();
    let mut okr_refs = Vec::new();
    let mut task_refs = Vec::new();
    for evidence_ref in evidence_refs.iter().take(LIVE_EVIDENCE_REF_LIMIT) {
        if is_okr_source_type(&evidence_ref.source_type) {
            match parse_okr_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => okr_refs.push((evidence_ref, parsed)),
                None => degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的 OKR 引用",
                )),
            }
            continue;
        }

        if is_task_source_type(&evidence_ref.source_type) {
            match parse_task_evidence_ref(&evidence_ref.source_ref) {
                Some(parsed) => task_refs.push((evidence_ref, parsed)),
                None => degraded.push(degraded_summary(
                    evidence_ref,
                    "source_ref 不是可识别的任务引用",
                )),
            }
            continue;
        }

        degraded.push(degraded_summary(
            evidence_ref,
            "source_type 暂不支持实时读取",
        ));
    }

    if evidence_refs.len() > LIVE_EVIDENCE_REF_LIMIT {
        degraded.push(format!(
            "仅实时读取前 {} 条 evidence refs。",
            LIVE_EVIDENCE_REF_LIMIT
        ));
    }

    if okr_refs.is_empty() && task_refs.is_empty() {
        return degraded;
    }

    let Some(persistence) = runtime
        .feishu_login
        .as_ref()
        .and_then(|login| login.grant_persistence())
    else {
        degraded.push("未读取到实时 Feishu 证据：后端未配置 Feishu 授权存储。".to_string());
        return degraded;
    };

    let pool = persistence.pool();
    let grant_id = match resolve_grant_id_for_user(&pool, auth_context).await {
        Ok(grant_id) => grant_id,
        Err(reason) => {
            degraded.push(format!("未读取到实时 Feishu 证据：{}。", reason));
            return degraded;
        }
    };

    let token_grant = match PostgresTokenGrantRepository::new(pool.clone())
        .get_by_id(&auth_context.tenant_id, &grant_id)
        .await
    {
        Ok(Some(grant)) => grant,
        Ok(None) => {
            degraded.push("未读取到实时 Feishu 证据：未找到用户授权 grant。".to_string());
            return degraded;
        }
        Err(_) => {
            degraded.push("未读取到实时 Feishu 证据：读取授权 grant 失败。".to_string());
            return degraded;
        }
    };

    if !gate_grant_and_refs_for_live_read(
        &token_grant,
        persistence.grant_key_id(),
        &mut okr_refs,
        &mut task_refs,
        &mut degraded,
    ) {
        return degraded;
    }

    let mut token_grant = token_grant;
    let now = SystemTime::now();
    let now_ms = system_time_to_ms(now);
    if grant_requires_refresh_before_read(&token_grant, now_ms) {
        let Some(login) = runtime.feishu_login.as_ref() else {
            degraded.push("未读取到实时 Feishu 证据：后端未配置 Feishu 授权刷新。".to_string());
            return degraded;
        };
        token_grant = match refresh_grant_before_live_read(
            pool.clone(),
            login,
            persistence,
            auth_context,
            &token_grant,
            now,
            now_ms,
        )
        .await
        {
            Ok(grant) => grant,
            Err(error) => {
                degraded.push(format!(
                    "未读取到实时 Feishu 证据：{}。",
                    error.safe_reason()
                ));
                return degraded;
            }
        };
    }

    if !gate_grant_and_refs_for_live_read(
        &token_grant,
        persistence.grant_key_id(),
        &mut okr_refs,
        &mut task_refs,
        &mut degraded,
    ) {
        return degraded;
    }

    let access_token = match read_access_token_from_encrypted_grant(
        &token_grant.encrypted_oauth_grant,
        persistence.grant_key_material(),
    ) {
        Ok(token) => token,
        Err(_) => {
            degraded.push("未读取到实时 Feishu 证据：授权令牌解密失败。".to_string());
            return degraded;
        }
    };

    let open_api_config = runtime
        .feishu_login
        .as_ref()
        .map(|login| login.open_api_config())
        .unwrap_or_default();
    let http_client = match ReqwestAsyncHttpClient::with_config(&open_api_config) {
        Ok(client) => client,
        Err(_) => {
            degraded.push("未读取到实时 Feishu 证据：Feishu HTTP 客户端初始化失败。".to_string());
            return degraded;
        }
    };
    let mut live_summaries = Vec::new();
    if !okr_refs.is_empty() {
        let mut okr_client = FeishuOkrReadClient::new(open_api_config.clone(), http_client.clone());
        let okr_ids = okr_refs
            .iter()
            .map(|(_, parsed)| parsed.okr_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        match okr_client
            .batch_get_okrs(FeishuOkrBatchGetRequest {
                user_access_token: access_token.clone(),
                user_id_type: OkrUserIdType::OpenId,
                okr_ids,
                lang: None,
            })
            .await
        {
            Ok(response) => {
                if let Some(data) = response.data {
                    let snapshot = OkrReadSnapshot::from_batch_get_data(&data);
                    live_summaries.extend(okr_refs.into_iter().map(|(evidence_ref, parsed)| {
                        build_live_summary(evidence_ref, &parsed, &snapshot)
                    }));
                } else {
                    live_summaries
                        .push("未读取到实时 Feishu 证据：Feishu 返回空数据。".to_string());
                }
            }
            Err(_) => {
                live_summaries.push("未读取到实时 Feishu 证据：实时读取失败。".to_string());
            }
        }
    }

    if !task_refs.is_empty() {
        let mut task_client = FeishuTaskReadClient::new(open_api_config, http_client);
        for (evidence_ref, parsed) in task_refs {
            match task_client
                .get_task_summary(FeishuTaskGetRequest {
                    user_access_token: access_token.clone(),
                    source_ref: parsed.source_ref,
                    user_id_type: TaskUserIdType::OpenId,
                })
                .await
            {
                Ok(summary) => {
                    live_summaries.push(build_task_live_summary(evidence_ref, &summary));
                }
                Err(error) => {
                    live_summaries.push(degraded_summary(
                        evidence_ref,
                        task_read_error_reason(error),
                    ));
                }
            }
        }
    }

    live_summaries.extend(degraded);
    live_summaries
}

fn gate_grant_and_refs_for_live_read<'a>(
    token_grant: &EncryptedTokenGrantRecord,
    expected_grant_key_id: &str,
    okr_refs: &mut Vec<(&'a AgentEvidenceRefDTO, ParsedOkrEvidenceRef)>,
    task_refs: &mut Vec<(&'a AgentEvidenceRefDTO, ParsedTaskEvidenceRef)>,
    degraded: &mut Vec<String>,
) -> bool {
    if let Some(reason) = live_read_grant_denial_reason(token_grant) {
        degraded.push(format!("未读取到实时 Feishu 证据：{}。", reason));
        return false;
    }

    if token_grant.oauth_grant_key_id != expected_grant_key_id {
        degraded.push("未读取到实时 Feishu 证据：授权密钥版本不匹配。".to_string());
        return false;
    }

    gate_refs_by_scope(&token_grant.scopes, okr_refs, task_refs, degraded);
    !(okr_refs.is_empty() && task_refs.is_empty())
}

#[cfg(test)]
mod tests {
    use super::grant::{
        ensure_refresh_report_allows_read, token_refresh_snapshot_for_live_read,
        TOKEN_REFRESH_SKEW_MS,
    };
    use super::refs::{has_okr_progress_read_scope, has_task_read_scope};
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};
    use oar_core::action::capability::FeishuScope;
    use oar_core::domain::identity::{
        ActorKind, ScopeBoundary, TenantId, TokenGrantId, TokenGrantState,
    };
    use oar_core::domain::token_refresh::service::{
        AuthRefreshAdapter, TokenRefreshCommandSink, TokenRefreshService,
    };
    use oar_core::domain::token_refresh::types::{
        EncryptedGrantMaterial, RefreshOutcome, TokenRefreshApplyResult, TokenRefreshCommandKind,
        TokenRefreshGrantSnapshot, TokenRefreshRepositoryCommand, TokenRefreshServiceReport,
    };
    use oar_core::storage::postgres::EncryptedTokenGrantRecord;
    use oar_lark_adapter::TaskReadSummary;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_okr_ref_supports_path_style() {
        let parsed =
            parse_okr_evidence_ref("okr://okr_demo/objectives/obj_demo/krs/kr_demo").expect("okr");
        assert_eq!(parsed.okr_id, "okr_demo");
        assert_eq!(parsed.objective_id, "obj_demo");
        assert_eq!(parsed.kr_id, "kr_demo");
    }

    #[test]
    fn parse_okr_ref_supports_colon_style() {
        let parsed =
            parse_okr_evidence_ref("okr:okr_demo:objective:obj_demo:kr:kr_demo").expect("okr");
        assert_eq!(parsed.okr_id, "okr_demo");
        assert_eq!(parsed.objective_id, "obj_demo");
        assert_eq!(parsed.kr_id, "kr_demo");
    }

    #[test]
    fn parse_okr_ref_rejects_invalid_format() {
        assert!(parse_okr_evidence_ref("okr://okr_demo/objectives/obj_demo").is_none());
        assert!(parse_okr_evidence_ref("okr:okr_demo:obj:obj_demo:kr:kr_demo").is_none());
    }

    #[test]
    fn parse_okr_ref_rejects_unsafe_segments() {
        assert!(parse_okr_evidence_ref(&format!(
            "okr://{}/objectives/obj_demo/krs/kr_demo",
            "x".repeat(101)
        ))
        .is_none());
        assert!(parse_okr_evidence_ref("okr:okr?demo:objective:obj_demo:kr:kr_demo").is_none());
        assert!(parse_okr_evidence_ref("okr:okr_demo:objective:obj#demo:kr:kr_demo").is_none());
    }

    #[test]
    fn parse_task_ref_supports_task_and_feishu_task_schemes() {
        let task = parse_task_evidence_ref(" task://task_123 ").expect("task ref");
        assert_eq!(task.source_ref, "task://task_123");
        assert_eq!(task.task_id, "task_123");

        let feishu_task = parse_task_evidence_ref("feishu://task/task_456").expect("feishu task");
        assert_eq!(feishu_task.source_ref, "task://task_456");
        assert_eq!(feishu_task.task_id, "task_456");
    }

    #[test]
    fn parse_task_ref_rejects_unsafe_shapes() {
        assert!(parse_task_evidence_ref("task://").is_none());
        assert!(parse_task_evidence_ref("task://task_123/subtask").is_none());
        assert!(parse_task_evidence_ref("feishu://task/task_123?debug=true").is_none());
        assert!(
            parse_task_evidence_ref("okr://okr_demo/objectives/obj_demo/krs/kr_demo").is_none()
        );
    }

    #[test]
    fn okr_progress_read_scope_accepts_only_feishu_scope_name() {
        assert!(has_okr_progress_read_scope(&[FeishuScope::OkrProgressRead
            .as_str()
            .to_string()]));
        assert!(!has_okr_progress_read_scope(&[
            "okr.progress.read".to_string()
        ]));
        assert!(!has_okr_progress_read_scope(
            &["task:task:read".to_string()]
        ));
    }

    #[test]
    fn task_read_scope_accepts_only_feishu_scope_name() {
        assert!(has_task_read_scope(&[FeishuScope::TaskRead
            .as_str()
            .to_string()]));
        assert!(!has_task_read_scope(&["task.read".to_string()]));
        assert!(!has_task_read_scope(&[FeishuScope::OkrProgressRead
            .as_str()
            .to_string()]));
    }

    #[tokio::test]
    async fn live_context_degrades_when_feishu_persistence_is_unavailable() {
        let mut request = AgentStreamRequest {
            messages: vec![AgentMessageDTO {
                role: "user".to_string(),
                text: "请读取实时进展".to_string(),
            }],
            context: AgentConversationContextDTO {
                title: "KR 风险".to_string(),
                risk_reason: "延期".to_string(),
                action_summary: "更新进度".to_string(),
                evidence_summaries: vec!["历史摘要".to_string()],
                evidence_refs: vec![AgentEvidenceRefDTO {
                    source_type: "okr".to_string(),
                    source_ref: "okr://okr_demo/objectives/obj_demo/krs/kr_demo".to_string(),
                    summary: "KR 实时读取".to_string(),
                }],
                workspace_summary: "摘要".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
                live_feishu_read_summaries: vec![],
            },
        };
        let runtime = OarHttpFacadeRuntime::disabled();
        let auth_context = AuthenticatedContext {
            session_id: "oar_session_test".to_string(),
            tenant_id: "tenant_x".to_string(),
            user_id: "feishu_user_tenant_ou_demo".to_string(),
        };

        inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

        assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
        assert!(
            request.context.live_feishu_read_summaries[0].contains("后端未配置 Feishu 授权存储")
        );
    }

    #[tokio::test]
    async fn task_live_context_uses_task_refs_before_safe_degrade() {
        let mut request = AgentStreamRequest {
            messages: vec![AgentMessageDTO {
                role: "user".to_string(),
                text: "请读取实时任务".to_string(),
            }],
            context: AgentConversationContextDTO {
                title: "任务风险".to_string(),
                risk_reason: "待办未闭环".to_string(),
                action_summary: "更新任务".to_string(),
                evidence_summaries: vec!["历史摘要".to_string()],
                evidence_refs: vec![AgentEvidenceRefDTO {
                    source_type: "task".to_string(),
                    source_ref: "feishu://task/task_123".to_string(),
                    summary: "任务实时读取".to_string(),
                }],
                workspace_summary: "摘要".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
                live_feishu_read_summaries: vec![],
            },
        };
        let runtime = OarHttpFacadeRuntime::disabled();
        let auth_context = AuthenticatedContext {
            session_id: "oar_session_test".to_string(),
            tenant_id: "tenant_x".to_string(),
            user_id: "feishu_user_tenant_ou_demo".to_string(),
        };

        inject_live_feishu_context(&runtime, &auth_context, &mut request).await;

        assert_eq!(request.context.live_feishu_read_summaries.len(), 1);
        assert!(
            request.context.live_feishu_read_summaries[0].contains("后端未配置 Feishu 授权存储")
        );
        assert!(!request.context.live_feishu_read_summaries[0].contains("暂不支持实时读取"));
    }

    #[tokio::test]
    async fn live_context_requires_source_type_to_match_task_ref() {
        let refs = vec![AgentEvidenceRefDTO {
            source_type: "doc".to_string(),
            source_ref: "task://task_123".to_string(),
            summary: "任务证据".to_string(),
        }];
        let runtime = OarHttpFacadeRuntime::disabled();
        let auth_context = AuthenticatedContext {
            session_id: "oar_session_test".to_string(),
            tenant_id: "tenant_x".to_string(),
            user_id: "feishu_user_tenant_ou_demo".to_string(),
        };

        let summaries = assemble_live_feishu_summaries(&runtime, &auth_context, &refs).await;

        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].contains("source_type 暂不支持实时读取"));
        assert!(!summaries[0].contains("授权存储"));
    }

    #[test]
    fn task_live_summary_is_sanitized_and_compact() {
        let evidence_ref = AgentEvidenceRefDTO {
            source_type: "task".to_string(),
            source_ref: "task://task_123".to_string(),
            summary: "任务证据".to_string(),
        };
        let summary = build_task_live_summary(
            &evidence_ref,
            &TaskReadSummary {
                source_ref: "task://task_123".to_string(),
                task_id: "task_123".to_string(),
                title: Some(" Ship task read integration ".to_string()),
                status: Some("open".to_string()),
                due: Some(oar_lark_adapter::TaskReadDue {
                    timestamp: Some("2026-05-29".to_string()),
                    is_all_day: Some(true),
                }),
                owners: vec![oar_lark_adapter::TaskReadOwner {
                    owner_id: Some("ou_sensitive".to_string()),
                    owner_type: Some("open_id".to_string()),
                }],
                update_time: Some("1717000000".to_string()),
            },
        );

        assert!(summary.contains("任务证据｜实时：任务「Ship task read integration」状态 open"));
        assert!(summary.contains("截止 2026-05-29（全天）"));
        assert!(summary.contains("负责人 1 人"));
        assert!(!summary.contains("ou_sensitive"));
    }

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
    fn grant_refresh_predicate_uses_expiry_skew() {
        let now = UNIX_EPOCH + Duration::from_secs(10);
        let now_ms = system_time_to_ms(now);

        let inside_skew =
            sample_token_grant_record(TokenGrantState::Valid, Some(now_ms + TOKEN_REFRESH_SKEW_MS));
        assert!(grant_requires_refresh_before_read(&inside_skew, now_ms));

        let outside_skew = sample_token_grant_record(
            TokenGrantState::Valid,
            Some(now_ms + TOKEN_REFRESH_SKEW_MS + 1),
        );
        assert!(!grant_requires_refresh_before_read(&outside_skew, now_ms));
    }

    #[test]
    fn unusable_grant_states_deny_live_read_even_without_timestamps() {
        let revoked = sample_token_grant_record(TokenGrantState::Revoked, None);
        assert_eq!(
            live_read_grant_denial_reason(&revoked),
            Some("授权已失效，需要重新登录")
        );

        let reauth = sample_token_grant_record(TokenGrantState::ReauthRequired, None);
        assert_eq!(
            live_read_grant_denial_reason(&reauth),
            Some("授权已失效，需要重新登录")
        );

        let mut bot = sample_token_grant_record(TokenGrantState::Valid, None);
        bot.actor_kind = ActorKind::Bot;
        assert_eq!(
            live_read_grant_denial_reason(&bot),
            Some("授权主体不是当前用户")
        );
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

    #[test]
    fn grant_debug_redacts_token_material() {
        let mut grant = sample_token_grant_record(TokenGrantState::Valid, None);
        grant.encrypted_oauth_grant =
            b"access-token-sensitive refresh-token-sensitive raw-response".to_vec();

        let debug = format!("{grant:?}");

        assert!(!debug.contains("access-token-sensitive"));
        assert!(!debug.contains("refresh-token-sensitive"));
        assert!(!debug.contains("raw-response"));
    }

    fn refresh_if_stale_for_test<A, S>(
        grant: &EncryptedTokenGrantRecord,
        now: SystemTime,
        adapter: A,
        sink: S,
    ) -> Option<TokenRefreshServiceReport>
    where
        A: AuthRefreshAdapter,
        S: TokenRefreshCommandSink,
        S::Error: std::fmt::Debug,
    {
        if !grant_requires_refresh_before_read(grant, system_time_to_ms(now)) {
            return None;
        }
        let snapshot = token_refresh_snapshot_for_live_read(grant);
        let mut service = TokenRefreshService::new(adapter, sink);
        Some(
            service
                .refresh_grant_at(snapshot, now)
                .expect("test token refresh service"),
        )
    }

    fn sample_token_grant_record(
        state: TokenGrantState,
        expires_at_ms: Option<u64>,
    ) -> EncryptedTokenGrantRecord {
        EncryptedTokenGrantRecord {
            id: "grant_01".to_string(),
            tenant_id: "tenant_01".to_string(),
            identity_id: "identity_01".to_string(),
            actor_kind: ActorKind::User,
            scope_boundary: ScopeBoundary::User,
            scopes: vec![FeishuScope::OkrProgressRead.as_str().to_string()],
            state,
            issued_at_ms: 1,
            expires_at_ms,
            refreshed_at_ms: None,
            revoked_at_ms: None,
            reauth_required_at_ms: None,
            last_refresh_error: None,
            encrypted_oauth_grant: vec![1, 2, 3],
            oauth_grant_key_id: "grant_key_v1".to_string(),
            oauth_grant_fingerprint: "fp_old".to_string(),
            revocation_reason: None,
        }
    }

    #[derive(Clone)]
    struct FakeAuthRefreshAdapter {
        state: Rc<RefCell<FakeAuthRefreshState>>,
    }

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

    struct FakeCommandSinkState {
        calls: usize,
        result: Result<Option<TokenRefreshApplyResult>, ()>,
        last_command: Option<TokenRefreshRepositoryCommand>,
    }

    impl FakeCommandSink {
        fn new(result: Result<Option<TokenRefreshApplyResult>, ()>) -> Self {
            Self {
                state: Rc::new(RefCell::new(FakeCommandSinkState {
                    calls: 0,
                    result,
                    last_command: None,
                })),
            }
        }

        fn calls(&self) -> usize {
            self.state.borrow().calls
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
}
