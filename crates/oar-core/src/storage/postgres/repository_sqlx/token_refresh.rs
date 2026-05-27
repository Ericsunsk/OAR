use super::*;

impl PostgresTokenRefreshUnitOfWork {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn apply_planned_command_with_audit(
        &self,
        planned: TokenRefreshPlannedCommand,
        audit_context: TokenRefreshAuditContext,
    ) -> PgRepositoryResult<PostgresTokenRefreshUnitOfWorkReport> {
        validate_token_refresh_plan(&planned)?;
        let summary = planned
            .report
            .audit_summary(TokenRefreshReportStatus::ConflictNoop);
        self.apply_command_with_summary(planned.command, summary, audit_context)
            .await
    }

    async fn apply_command_with_summary(
        &self,
        command: TokenRefreshRepositoryCommand,
        summary: TokenRefreshAuditSummary,
        audit_context: TokenRefreshAuditContext,
    ) -> PgRepositoryResult<PostgresTokenRefreshUnitOfWorkReport> {
        let mut tx = self.pool.begin().await?;
        let apply_result =
            super::token_refresh::apply_refresh_command_in_tx(&mut tx, command).await?;
        let mut summary = summary;
        summary.status = if apply_result.is_some() {
            TokenRefreshReportStatus::Succeeded
        } else {
            TokenRefreshReportStatus::ConflictNoop
        };

        let event = token_refresh_audit_event(audit_context, &summary);
        super::audit::append_audit_event_in_tx(&mut tx, &event, None).await?;
        tx.commit().await?;

        Ok(PostgresTokenRefreshUnitOfWorkReport {
            apply_result,
            event,
        })
    }
}

impl<A> PostgresTokenRefreshOrchestrator<A>
where
    A: AsyncAuthRefreshAdapter,
{
    pub fn new(pool: PgPool, adapter: A) -> Self {
        Self {
            adapter,
            uow: PostgresTokenRefreshUnitOfWork::new(pool.clone()),
            audit: PostgresAuditEventRepository::new(pool),
        }
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub async fn refresh_grant_with_audit(
        &mut self,
        snapshot: TokenRefreshGrantSnapshot,
        now: SystemTime,
        audit_context: TokenRefreshAuditContext,
    ) -> PgRepositoryResult<PostgresTokenRefreshOrchestratorReport> {
        if let Some(service_report) = token_refresh_short_circuit_report(&snapshot) {
            let event = token_refresh_audit_event(audit_context, &service_report.audit_summary());
            self.audit.append(&event, None).await?;
            return Ok(PostgresTokenRefreshOrchestratorReport {
                service_report,
                event,
            });
        }

        let outcome = self.adapter.refresh(&snapshot).await;
        let planned = plan_token_refresh_command(&snapshot, outcome, now)
            .map_err(PostgresRepositoryError::TokenRefreshDecisionBridge)?;
        let report_template = planned.report.clone();

        let uow_report = self
            .uow
            .apply_planned_command_with_audit(planned, audit_context)
            .await?;
        let service_report = report_template.into_service_report(uow_report.apply_result.is_some());

        Ok(PostgresTokenRefreshOrchestratorReport {
            service_report,
            event: uow_report.event,
        })
    }
}

impl<A> PostgresTokenRefreshSweep<A>
where
    A: AsyncAuthRefreshAdapter,
{
    pub fn new(pool: PgPool, adapter: A) -> Self {
        Self {
            candidates: PostgresTokenGrantRepository::new(pool.clone()),
            orchestrator: PostgresTokenRefreshOrchestrator::new(pool, adapter),
        }
    }

    pub fn adapter(&self) -> &A {
        self.orchestrator.adapter()
    }

    pub async fn run_once_for_tenant(
        &mut self,
        request: PostgresTokenRefreshSweepRequest,
    ) -> PgRepositoryResult<PostgresTokenRefreshSweepReport> {
        if request.limit == 0 {
            return Ok(PostgresTokenRefreshSweepReport {
                candidate_count: 0,
                attempted_count: 0,
                has_more: false,
                reports: Vec::new(),
            });
        }

        let query_limit = request.limit.saturating_add(1);
        let mut candidates = self
            .candidates
            .list_refresh_candidate_snapshots(&request.tenant_id, request.due_before, query_limit)
            .await?;
        let has_more = candidates.len() > request.limit as usize;
        if has_more {
            candidates.truncate(request.limit as usize);
        }
        let candidate_count = candidates.len();
        let mut reports = Vec::with_capacity(candidate_count);

        for (index, snapshot) in candidates.into_iter().enumerate() {
            let audit_context = TokenRefreshAuditContext {
                trace_id: request.audit_trace_id.clone(),
                sequence: request.audit_sequence_start + index as u64,
                occurred_at_ms: request.occurred_at_ms,
                actor: request.actor.clone(),
                workspace_id: request.workspace_id.clone(),
            };

            let report = self
                .orchestrator
                .refresh_grant_with_audit(snapshot, request.now, audit_context)
                .await?;
            reports.push(report);
        }

        Ok(PostgresTokenRefreshSweepReport {
            candidate_count,
            attempted_count: reports.len(),
            has_more,
            reports,
        })
    }
}

fn validate_token_refresh_plan(planned: &TokenRefreshPlannedCommand) -> PgRepositoryResult<()> {
    let expected_command_kind = planned.command.kind();
    if planned.report.command_kind != expected_command_kind {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "command_kind",
            expected: format!("{expected_command_kind:?}"),
            actual: format!("{:?}", planned.report.command_kind),
        });
    }

    if planned.report.tenant_id != *planned.tenant_id() {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "tenant_id",
            expected: planned.tenant_id().0.clone(),
            actual: planned.report.tenant_id.0.clone(),
        });
    }

    if planned.report.grant_id != *planned.grant_id() {
        return Err(PostgresRepositoryError::TokenRefreshPlanMismatch {
            field: "grant_id",
            expected: planned.grant_id().0.clone(),
            actual: planned.report.grant_id.0.clone(),
        });
    }

    Ok(())
}

fn token_refresh_apply_result_from_record(
    record: EncryptedTokenGrantRecord,
) -> TokenRefreshApplyResult {
    TokenRefreshApplyResult {
        grant_id: crate::domain::identity::TokenGrantId(record.id),
        tenant_id: crate::domain::identity::TenantId(record.tenant_id),
        state: record.state,
        fingerprint: record.oauth_grant_fingerprint,
    }
}

pub(super) async fn apply_refresh_command_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    command: TokenRefreshRepositoryCommand,
) -> PgRepositoryResult<Option<TokenRefreshApplyResult>> {
    let row = match command {
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
            sqlx::query(ROTATE_TOKEN_GRANT)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(option_u64_to_i64(expires_at_ms))
                .bind(refreshed_at_ms as i64)
                .bind(&encrypted_grant_blob.0)
                .bind(&grant_key_id)
                .bind(&new_fingerprint)
                .fetch_optional(&mut **tx)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkNeedsRefresh {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(refreshed_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(&mut **tx)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkReauthRequired {
            grant_id,
            tenant_id,
            expected_fingerprint,
            reauth_required_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REAUTH_REQUIRED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(reauth_required_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(&mut **tx)
                .await?
        }
        TokenRefreshRepositoryCommand::MarkConfigRequired {
            grant_id,
            tenant_id,
            expected_fingerprint,
            refreshed_at_ms,
            safe_error,
        } => {
            let safe_error = sanitize_refresh_error_for_storage(&safe_error);
            sqlx::query(MARK_TOKEN_GRANT_REFRESH_FAILED)
                .bind(&tenant_id.0)
                .bind(&grant_id.0)
                .bind(&expected_fingerprint)
                .bind(refreshed_at_ms as i64)
                .bind(&safe_error)
                .fetch_optional(&mut **tx)
                .await?
        }
    };

    row.as_ref()
        .map(encrypted_token_grant_from_row)
        .transpose()
        .map(|value| value.map(token_refresh_apply_result_from_record))
}
