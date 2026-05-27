use std::fmt;

use async_trait::async_trait;
use oar_core::lark::auth::client::{
    AsyncFeishuAuthRefreshTransport, FeishuAuthRefreshRawEnvelope, FeishuAuthRefreshTransport,
};
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::config::FeishuOpenApiConfig;
use crate::error::{classify_feishu_refresh_failure, safe_error_for_failure_class};
use crate::redaction::SecretString;

use super::envelope::{
    failure_envelope_value, failure_envelope_value_for_safe_error, raw_envelope,
    success_envelope_value,
};
use super::http::{AsyncHttpClient, HttpClient, HttpRequest, HttpResponse};
use super::types::{
    AsyncFeishuRefreshMaterialProvider, FeishuGrantEncryptionInput, FeishuGrantEncryptor,
    FeishuRefreshMaterialProvider,
};

const REFRESH_TOKEN_PATH: &str = "/open-apis/authen/v2/oauth/token";
const OAR_USER_AGENT: &str = concat!("oar-lark-adapter/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, PartialEq, Eq)]
pub enum FeishuOAuthTransportError {
    MaterialUnavailable,
    EncryptionFailed,
    EnvelopeSerializationFailed,
}

impl fmt::Debug for FeishuOAuthTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuOAuthTransportError::MaterialUnavailable => {
                write!(f, "FeishuOAuthTransportError(material_unavailable)")
            }
            FeishuOAuthTransportError::EncryptionFailed => {
                write!(f, "FeishuOAuthTransportError(encryption_failed)")
            }
            FeishuOAuthTransportError::EnvelopeSerializationFailed => {
                write!(
                    f,
                    "FeishuOAuthTransportError(envelope_serialization_failed)"
                )
            }
        }
    }
}

impl fmt::Display for FeishuOAuthTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuOAuthTransportError::MaterialUnavailable => {
                write!(f, "feishu refresh material unavailable")
            }
            FeishuOAuthTransportError::EncryptionFailed => {
                write!(f, "feishu grant encryption failed")
            }
            FeishuOAuthTransportError::EnvelopeSerializationFailed => {
                write!(f, "feishu refresh envelope serialization failed")
            }
        }
    }
}

impl std::error::Error for FeishuOAuthTransportError {}

pub struct FeishuOAuthTransport<P, E, H> {
    config: FeishuOpenApiConfig,
    material_provider: P,
    encryptor: E,
    http_client: H,
}

impl<P, E, H> FeishuOAuthTransport<P, E, H> {
    pub fn new(
        config: FeishuOpenApiConfig,
        material_provider: P,
        encryptor: E,
        http_client: H,
    ) -> Self {
        Self {
            config,
            material_provider,
            encryptor,
            http_client,
        }
    }

    pub fn config(&self) -> &FeishuOpenApiConfig {
        &self.config
    }

    pub fn http_client(&self) -> &H {
        &self.http_client
    }
}

impl<P, E, H> fmt::Debug for FeishuOAuthTransport<P, E, H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthTransport")
            .field("config", &self.config)
            .field("material_provider", &"[REDACTED]")
            .field("encryptor", &"[REDACTED]")
            .field("http_client", &"[REDACTED]")
            .finish()
    }
}

impl<P, E, H> FeishuAuthRefreshTransport for FeishuOAuthTransport<P, E, H>
where
    P: FeishuRefreshMaterialProvider,
    E: FeishuGrantEncryptor,
    H: HttpClient,
{
    type Error = FeishuOAuthTransportError;

    fn execute(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshRawEnvelope, Self::Error> {
        let material = match self.material_provider.refresh_material(request) {
            Ok(material) => material,
            Err(_) => return raw_envelope(failure_envelope_value("transient_failure")),
        };
        let http_request = build_refresh_request(&self.config, &material);
        let response = match self.http_client.post_json(http_request) {
            Ok(response) => response,
            Err(_) => return raw_envelope(failure_envelope_value("transient_failure")),
        };

        refresh_response_to_envelope(request, response, &mut self.encryptor)
    }
}

#[async_trait]
impl<P, E, H> AsyncFeishuAuthRefreshTransport for FeishuOAuthTransport<P, E, H>
where
    P: AsyncFeishuRefreshMaterialProvider + Send,
    E: FeishuGrantEncryptor + Send,
    H: AsyncHttpClient + Send,
{
    type Error = FeishuOAuthTransportError;

    async fn execute(
        &mut self,
        request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuAuthRefreshRawEnvelope, Self::Error> {
        let material = match self.material_provider.refresh_material(request).await {
            Ok(material) => material,
            Err(_) => return raw_envelope(failure_envelope_value("transient_failure")),
        };
        let http_request = build_refresh_request(&self.config, &material);
        let response = match self.http_client.post_json(http_request).await {
            Ok(response) => response,
            Err(_) => return raw_envelope(failure_envelope_value("transient_failure")),
        };

        refresh_response_to_envelope(request, response, &mut self.encryptor)
    }
}

fn refresh_response_to_envelope<E>(
    request: &FeishuAuthRefreshRequest,
    response: HttpResponse,
    encryptor: &mut E,
) -> Result<FeishuAuthRefreshRawEnvelope, FeishuOAuthTransportError>
where
    E: FeishuGrantEncryptor,
{
    if response.status >= 500 {
        return raw_envelope(failure_envelope_value("transient_failure"));
    }

    let parsed: FeishuTokenResponse = match serde_json::from_str(&response.body) {
        Ok(parsed) => parsed,
        Err(_) => return raw_envelope(failure_envelope_value("transient_failure")),
    };

    match parsed.code {
        Some(0) => {
            let Some(access_token) = parsed.access_token else {
                return raw_envelope(failure_envelope_value("transient_failure"));
            };
            let Some(refresh_token) = parsed.refresh_token else {
                return raw_envelope(failure_envelope_value("transient_failure"));
            };
            let Some(expires_in_seconds) = parsed.expires_in else {
                return raw_envelope(failure_envelope_value("transient_failure"));
            };
            let encrypted = encryptor
                .encrypt(FeishuGrantEncryptionInput {
                    grant_id: request.grant_id.clone(),
                    tenant_id: request.tenant_id.clone(),
                    expected_fingerprint: request.expected_fingerprint.clone(),
                    access_token: SecretString::new(access_token),
                    refresh_token: SecretString::new(refresh_token),
                    expires_in_seconds,
                    refresh_token_expires_in_seconds: parsed.refresh_token_expires_in,
                    token_type: parsed.token_type,
                    scope: parsed.scope,
                })
                .map_err(|_| FeishuOAuthTransportError::EncryptionFailed)?;
            raw_envelope(success_envelope_value(encrypted))
        }
        Some(code) => {
            let class = classify_feishu_refresh_failure(code, response.status);
            raw_envelope(failure_envelope_value_for_safe_error(
                match class {
                    crate::error::FeishuRefreshFailureClass::ReauthRequired => "reauth_required",
                    crate::error::FeishuRefreshFailureClass::ConfigRequired => "config_required",
                    crate::error::FeishuRefreshFailureClass::Transient => "transient_failure",
                },
                safe_error_for_failure_class(class),
            ))
        }
        None => raw_envelope(failure_envelope_value("transient_failure")),
    }
}

#[derive(Deserialize)]
struct FeishuTokenResponse {
    code: Option<i64>,
    access_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<u64>,
    token_type: Option<String>,
    scope: Option<String>,
}

pub(super) fn build_refresh_request(
    config: &FeishuOpenApiConfig,
    material: &super::types::FeishuRefreshMaterial,
) -> HttpRequest {
    let mut body = Map::new();
    body.insert("grant_type".to_string(), json!("refresh_token"));
    body.insert("client_id".to_string(), json!(material.client_id));
    body.insert(
        "client_secret".to_string(),
        json!(material.client_secret.expose_secret()),
    );
    body.insert(
        "refresh_token".to_string(),
        json!(material.refresh_token.expose_secret()),
    );
    if let Some(scope) = &material.scope {
        body.insert("scope".to_string(), json!(scope));
    }

    HttpRequest {
        method: "POST".to_string(),
        url: format!(
            "{}{}",
            config.base_url.trim_end_matches('/'),
            REFRESH_TOKEN_PATH
        ),
        headers: vec![
            (
                "Content-Type".to_string(),
                "application/json; charset=utf-8".to_string(),
            ),
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), OAR_USER_AGENT.to_string()),
        ],
        body: Value::Object(body),
        max_response_bytes: config.max_response_bytes,
    }
}
