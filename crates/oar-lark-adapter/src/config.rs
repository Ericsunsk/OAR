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
