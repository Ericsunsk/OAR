use std::sync::Once;

static INIT_TRACING: Once = Once::new();

pub fn init_structured_logging(service_name: &'static str) {
    INIT_TRACING.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with_target(true)
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .try_init();

        tracing::info!(service = service_name, "structured logging initialized");
    });
}
