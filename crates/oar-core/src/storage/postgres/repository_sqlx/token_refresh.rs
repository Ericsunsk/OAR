use super::*;

mod command_executor;

use command_executor::validate_token_refresh_plan;
pub(super) use command_executor::{
    apply_refresh_command_in_tx, apply_refresh_command_with_executor,
};

impl PostgresTokenRefreshRecorder {
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
    ) -> PgRepositoryResult<PostgresTokenRefreshRecorderReport> {
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
    ) -> PgRepositoryResult<PostgresTokenRefreshRecorderReport> {
        let mut tx = self.pool.begin().await?;
        let apply_result = apply_refresh_command_in_tx(&mut tx, command).await?;
        let mut summary = summary;
        summary.status = if apply_result.is_some() {
            TokenRefreshReportStatus::Succeeded
        } else {
            TokenRefreshReportStatus::ConflictNoop
        };

        let event = token_refresh_audit_event(audit_context, &summary);
        super::audit::append_audit_event_in_tx(&mut tx, &event, None).await?;
        tx.commit().await?;

        Ok(PostgresTokenRefreshRecorderReport {
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
            recorder: PostgresTokenRefreshRecorder::new(pool.clone()),
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

        let recorder_report = self
            .recorder
            .apply_planned_command_with_audit(planned, audit_context)
            .await?;
        let service_report =
            report_template.into_service_report(recorder_report.apply_result.is_some());

        Ok(PostgresTokenRefreshOrchestratorReport {
            service_report,
            event: recorder_report.event,
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
