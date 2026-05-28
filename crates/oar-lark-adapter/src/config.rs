use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct FeishuOpenApiConfig {
    pub base_url: String,
    pub max_response_bytes: usize,
    pub request_timeout_ms: u64,
    pub connect_timeout_ms: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FeishuOpenApiConfigError {
    EmptyBaseUrl,
    InvalidMaxResponseBytes,
    InvalidRequestTimeoutMs,
    InvalidConnectTimeoutMs,
}

impl fmt::Debug for FeishuOpenApiConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuOpenApiConfigError::EmptyBaseUrl => {
                write!(f, "FeishuOpenApiConfigError(empty_base_url)")
            }
            FeishuOpenApiConfigError::InvalidMaxResponseBytes => {
                write!(f, "FeishuOpenApiConfigError(invalid_max_response_bytes)")
            }
            FeishuOpenApiConfigError::InvalidRequestTimeoutMs => {
                write!(f, "FeishuOpenApiConfigError(invalid_request_timeout_ms)")
            }
            FeishuOpenApiConfigError::InvalidConnectTimeoutMs => {
                write!(f, "FeishuOpenApiConfigError(invalid_connect_timeout_ms)")
            }
        }
    }
}

impl fmt::Display for FeishuOpenApiConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeishuOpenApiConfigError::EmptyBaseUrl => write!(f, "feishu base url is required"),
            FeishuOpenApiConfigError::InvalidMaxResponseBytes => {
                write!(f, "feishu max response bytes must be greater than zero")
            }
            FeishuOpenApiConfigError::InvalidRequestTimeoutMs => {
                write!(f, "feishu request timeout must be greater than zero")
            }
            FeishuOpenApiConfigError::InvalidConnectTimeoutMs => {
                write!(f, "feishu connect timeout must be greater than zero")
            }
        }
    }
}

impl std::error::Error for FeishuOpenApiConfigError {}

impl Default for FeishuOpenApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://open.feishu.cn".to_string(),
            max_response_bytes: 64 * 1024,
            request_timeout_ms: 30_000,
            connect_timeout_ms: 5_000,
        }
    }
}

impl fmt::Debug for FeishuOpenApiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeishuOpenApiConfig")
            .field("base_url", &self.base_url)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("request_timeout_ms", &self.request_timeout_ms)
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .finish()
    }
}

impl FeishuOpenApiConfig {
    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, FeishuOpenApiConfigError> {
        let mut config = Self::default();

        if let Some(base_url) = env("OAR_FEISHU_BASE_URL") {
            config.base_url = base_url;
        }
        if let Some(max_response_bytes) = env("OAR_FEISHU_MAX_RESPONSE_BYTES") {
            config.max_response_bytes = parse_positive_usize(
                &max_response_bytes,
                FeishuOpenApiConfigError::InvalidMaxResponseBytes,
            )?;
        }
        if let Some(request_timeout_ms) = env("OAR_FEISHU_REQUEST_TIMEOUT_MS") {
            config.request_timeout_ms = parse_positive_u64(
                &request_timeout_ms,
                FeishuOpenApiConfigError::InvalidRequestTimeoutMs,
            )?;
        }
        if let Some(connect_timeout_ms) = env("OAR_FEISHU_CONNECT_TIMEOUT_MS") {
            config.connect_timeout_ms = parse_positive_u64(
                &connect_timeout_ms,
                FeishuOpenApiConfigError::InvalidConnectTimeoutMs,
            )?;
        }

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), FeishuOpenApiConfigError> {
        if self.base_url.trim().is_empty() {
            return Err(FeishuOpenApiConfigError::EmptyBaseUrl);
        }
        if self.max_response_bytes == 0 {
            return Err(FeishuOpenApiConfigError::InvalidMaxResponseBytes);
        }
        if self.request_timeout_ms == 0 {
            return Err(FeishuOpenApiConfigError::InvalidRequestTimeoutMs);
        }
        if self.connect_timeout_ms == 0 {
            return Err(FeishuOpenApiConfigError::InvalidConnectTimeoutMs);
        }
        Ok(())
    }
}

fn parse_positive_usize(
    value: &str,
    error: FeishuOpenApiConfigError,
) -> Result<usize, FeishuOpenApiConfigError> {
    let parsed = value.parse::<usize>().map_err(|_| error)?;
    if parsed == 0 {
        return Err(error);
    }
    Ok(parsed)
}

fn parse_positive_u64(
    value: &str,
    error: FeishuOpenApiConfigError,
) -> Result<u64, FeishuOpenApiConfigError> {
    let parsed = value.parse::<u64>().map_err(|_| error)?;
    if parsed == 0 {
        return Err(error);
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_map_uses_defaults_when_env_is_absent() {
        let config = FeishuOpenApiConfig::from_env_map(&|_| None).expect("default env config");
        assert_eq!(config, FeishuOpenApiConfig::default());
    }

    #[test]
    fn from_env_map_applies_overrides() {
        let config = FeishuOpenApiConfig::from_env_map(&|key| match key {
            "OAR_FEISHU_BASE_URL" => Some("https://open.feishu.cn".to_string()),
            "OAR_FEISHU_MAX_RESPONSE_BYTES" => Some("2048".to_string()),
            "OAR_FEISHU_REQUEST_TIMEOUT_MS" => Some("45000".to_string()),
            "OAR_FEISHU_CONNECT_TIMEOUT_MS" => Some("2500".to_string()),
            _ => None,
        })
        .expect("env overrides should parse");

        assert_eq!(
            config,
            FeishuOpenApiConfig {
                base_url: "https://open.feishu.cn".to_string(),
                max_response_bytes: 2048,
                request_timeout_ms: 45_000,
                connect_timeout_ms: 2_500,
            }
        );
    }

    #[test]
    fn from_env_map_rejects_invalid_or_zero_numeric_values_without_echoing_raw_value() {
        let invalid_number = FeishuOpenApiConfig::from_env_map(&|key| match key {
            "OAR_FEISHU_MAX_RESPONSE_BYTES" => Some("bad-number-secret".to_string()),
            _ => None,
        })
        .expect_err("invalid max bytes should fail");
        assert_eq!(
            invalid_number,
            FeishuOpenApiConfigError::InvalidMaxResponseBytes
        );
        let rendered = invalid_number.to_string();
        assert!(!rendered.contains("bad-number-secret"));

        let zero_timeout = FeishuOpenApiConfig::from_env_map(&|key| match key {
            "OAR_FEISHU_REQUEST_TIMEOUT_MS" => Some("0".to_string()),
            _ => None,
        })
        .expect_err("zero request timeout should fail");
        assert_eq!(
            zero_timeout,
            FeishuOpenApiConfigError::InvalidRequestTimeoutMs
        );
    }
}
