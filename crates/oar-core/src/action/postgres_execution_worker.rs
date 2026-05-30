use super::confirmed_action::ActionStatus;
use super::executor::ActionAdapter;
use super::postgres_executor::PostgresActionExecutor;
use crate::storage::postgres::{PostgresOperationLedgerRepository, PostgresRepositoryError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresConfirmedActionDrainConfig {
    pub tenant_id: String,
    pub limit: u32,
}

impl PostgresConfirmedActionDrainConfig {
    pub fn new(tenant_id: impl Into<String>, limit: u32) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresConfirmedActionDrainReport {
    pub selected: usize,
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub duplicate: usize,
    pub execution_errors: usize,
}

pub struct PostgresConfirmedActionWorker<A, C = fn() -> u64>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    queue: PostgresOperationLedgerRepository,
    executor: PostgresActionExecutor<A, C>,
    config: PostgresConfirmedActionDrainConfig,
}

impl<A, C> PostgresConfirmedActionWorker<A, C>
where
    A: ActionAdapter,
    C: FnMut() -> u64,
{
    pub fn new(
        queue: PostgresOperationLedgerRepository,
        executor: PostgresActionExecutor<A, C>,
        config: PostgresConfirmedActionDrainConfig,
    ) -> Self {
        Self {
            queue,
            executor,
            config,
        }
    }

    pub async fn drain_once(
        &mut self,
    ) -> Result<PostgresConfirmedActionDrainReport, PostgresRepositoryError> {
        let pending = self
            .queue
            .list_confirmed_actions_ready_for_execution(&self.config.tenant_id, self.config.limit)
            .await?;

        let mut report = PostgresConfirmedActionDrainReport {
            selected: pending.len(),
            attempted: 0,
            succeeded: 0,
            failed: 0,
            duplicate: 0,
            execution_errors: 0,
        };

        for item in pending {
            match self.executor.execute_confirmed_request(&item.request).await {
                Ok(execution) => {
                    if execution.duplicate {
                        report.duplicate += 1;
                    } else {
                        report.attempted += 1;
                    }
                    match execution.operation.status {
                        ActionStatus::Succeeded => report.succeeded += 1,
                        ActionStatus::Failed | ActionStatus::Cancelled => report.failed += 1,
                        _ => {}
                    }
                }
                Err(_) => {
                    report.execution_errors += 1;
                }
            }
        }

        Ok(report)
    }

    pub fn executor(&self) -> &PostgresActionExecutor<A, C> {
        &self.executor
    }
}
