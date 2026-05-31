use std::fmt;
use std::time::Duration;

use tokio::task::{JoinError, JoinHandle};
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::warn;

const DAEMON_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct TenantMaintenanceDaemonHandle {
    pub(super) cancellation: CancellationToken,
    pub(super) task: Option<JoinHandle<()>>,
}

impl fmt::Debug for TenantMaintenanceDaemonHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantMaintenanceDaemonHandle")
            .field("cancellation", &"[REDACTED]")
            .field("task", &"[REDACTED]")
            .finish()
    }
}

impl TenantMaintenanceDaemonHandle {
    pub(crate) async fn shutdown(mut self) {
        self.cancellation.cancel();
        let Some(mut task) = self.task.take() else {
            return;
        };
        tokio::select! {
            result = &mut task => {
                if let Err(error) = result {
                    warn!(
                        panic = error.is_panic(),
                        cancelled = error.is_cancelled(),
                        "tenant maintenance daemon task finished with join error"
                    );
                }
            }
            _ = time::sleep(DAEMON_SHUTDOWN_TIMEOUT) => {
                warn!("tenant maintenance daemon shutdown timed out; aborting task");
                task.abort();
                let _ = task.await;
            }
        }
    }

    pub(crate) async fn wait_finished(&mut self) -> Result<(), JoinError> {
        match self.task.as_mut() {
            Some(task) => task.await,
            None => Ok(()),
        }
    }
}

impl Drop for TenantMaintenanceDaemonHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.as_ref() {
            if !task.is_finished() {
                task.abort();
            }
        }
    }
}
