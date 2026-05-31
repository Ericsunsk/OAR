use std::fmt;

use reqwest::Url;

use super::TenantMaintenanceSettingsError;
use crate::util::non_empty_env;

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum TenantMaintenanceAuditOutboxSinkSettings {
    Webhook { endpoint: String },
}

impl fmt::Debug for TenantMaintenanceAuditOutboxSinkSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Webhook { .. } => f
                .debug_struct("Webhook")
                .field("endpoint", &"[REDACTED]")
                .finish(),
        }
    }
}

pub(super) fn tenant_maintenance_audit_outbox_sink_from_env(
    env: &impl Fn(&str) -> Option<String>,
) -> Result<TenantMaintenanceAuditOutboxSinkSettings, TenantMaintenanceSettingsError> {
    let Some(kind) = non_empty_env(env, TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK_ENV) else {
        return Err(TenantMaintenanceSettingsError::RequiresAuditOutboxSink);
    };
    match kind.as_str() {
        "webhook" => {
            let endpoint = non_empty_env(env, TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL_ENV)
                .ok_or(TenantMaintenanceSettingsError::RequiresAuditOutboxSink)?;
            validate_webhook_endpoint(&endpoint)?;
            Ok(TenantMaintenanceAuditOutboxSinkSettings::Webhook { endpoint })
        }
        "noop" | "local-noop" => Err(TenantMaintenanceSettingsError::InvalidConfig),
        _ => Err(TenantMaintenanceSettingsError::InvalidConfig),
    }
}

const TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK_ENV: &str = "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK";
const TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL_ENV: &str =
    "OAR_TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL";

fn validate_webhook_endpoint(value: &str) -> Result<(), TenantMaintenanceSettingsError> {
    let endpoint = Url::parse(value).map_err(|_| TenantMaintenanceSettingsError::InvalidConfig)?;
    if endpoint.scheme() == "https" && endpoint.host().is_some() {
        Ok(())
    } else {
        Err(TenantMaintenanceSettingsError::InvalidConfig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_sink_debug_redacts_endpoint() {
        let sink = tenant_maintenance_audit_outbox_sink_from_env(&audit_env(
            Some("webhook"),
            Some("https://audit.example.test/webhook?token=webhook-secret"),
        ))
        .expect("webhook sink");

        assert!(!format!("{sink:?}").contains("webhook-secret"));
    }

    #[test]
    fn audit_sink_rejects_non_production_sinks_and_unsafe_urls_without_echoing_values() {
        for sink in ["noop", "local-noop", "unknown"] {
            let error = tenant_maintenance_audit_outbox_sink_from_env(&audit_env(Some(sink), None))
                .expect_err("sink kind should be rejected");

            assert_eq!(error, TenantMaintenanceSettingsError::InvalidConfig);
            assert!(!format!("{error:?}").contains(sink));
        }

        let missing_url =
            tenant_maintenance_audit_outbox_sink_from_env(&audit_env(Some("webhook"), None))
                .expect_err("missing webhook URL should be rejected");
        assert_eq!(
            missing_url,
            TenantMaintenanceSettingsError::RequiresAuditOutboxSink
        );

        for endpoint in [
            "http://audit.example.test/webhook?token=webhook-secret",
            "not a url with webhook-secret",
        ] {
            let error = tenant_maintenance_audit_outbox_sink_from_env(&audit_env(
                Some("webhook"),
                Some(endpoint),
            ))
            .expect_err("webhook URL should be rejected");

            assert_eq!(error, TenantMaintenanceSettingsError::InvalidConfig);
            assert!(!format!("{error:?}").contains("webhook-secret"));
        }
    }

    fn audit_env<'a>(
        sink: Option<&'a str>,
        webhook_url: Option<&'a str>,
    ) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| match key {
            TENANT_MAINTENANCE_AUDIT_OUTBOX_SINK_ENV => sink.map(str::to_string),
            TENANT_MAINTENANCE_AUDIT_OUTBOX_WEBHOOK_URL_ENV => webhook_url.map(str::to_string),
            _ => None,
        }
    }
}
