use std::fmt;

use crate::config::FeishuOpenApiConfig;
use crate::redaction::SecretString;
use crate::url_encoding::encode_query;

use super::error::{FeishuOAuthLoginConfigError, FeishuOAuthLoginConfigInvalidField};

const AUTHORIZE_PATH: &str = "/open-apis/authen/v1/authorize";

#[derive(Clone)]
pub struct FeishuOAuthLoginConfig {
    pub open_api: FeishuOpenApiConfig,
    pub authorize_base_url: String,
    pub client_id: String,
    pub client_secret: SecretString,
    pub redirect_uri: String,
    pub scope: Option<String>,
}

impl fmt::Debug for FeishuOAuthLoginConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOAuthLoginConfig")
            .field("open_api", &self.open_api)
            .field("authorize_base_url", &self.authorize_base_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("redirect_uri", &self.redirect_uri)
            .field("scope", &self.scope)
            .finish()
    }
}

impl FeishuOAuthLoginConfig {
    pub fn new(
        open_api: FeishuOpenApiConfig,
        authorize_base_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
        scope: Option<String>,
    ) -> Result<Self, FeishuOAuthLoginConfigError> {
        open_api.validate().map_err(|_| {
            FeishuOAuthLoginConfigError::InvalidOpenApi(FeishuOAuthLoginConfigInvalidField::OpenApi)
        })?;
        let authorize_base_url = required_string(
            authorize_base_url.into(),
            FeishuOAuthLoginConfigError::EmptyAuthorizeBaseUrl,
        )?;
        let client_id =
            required_string(client_id.into(), FeishuOAuthLoginConfigError::EmptyClientId)?;
        let client_secret = required_string(
            client_secret.into(),
            FeishuOAuthLoginConfigError::EmptyClientSecret,
        )?;
        let redirect_uri = required_string(
            redirect_uri.into(),
            FeishuOAuthLoginConfigError::EmptyRedirectUri,
        )?;
        Ok(Self {
            open_api,
            authorize_base_url,
            client_id,
            client_secret: SecretString::new(client_secret),
            redirect_uri,
            scope: scope.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            }),
        })
    }

    pub fn authorization_url(&self, state: &str) -> String {
        let mut pairs = vec![
            ("client_id", self.client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("state", state),
        ];
        if let Some(scope) = &self.scope {
            pairs.push(("scope", scope.as_str()));
        }
        let query = encode_query(pairs);
        format!(
            "{}{}?{}",
            self.authorize_base_url.trim_end_matches('/'),
            AUTHORIZE_PATH,
            query
        )
    }
}

fn required_string(
    value: String,
    error: FeishuOAuthLoginConfigError,
) -> Result<String, FeishuOAuthLoginConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(error);
    }
    Ok(trimmed.to_string())
}
