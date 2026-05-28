use std::error::Error;
use std::fmt;
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OarHttpFacadeConfig {
    pub bind_addr: SocketAddr,
}

impl Default for OarHttpFacadeConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarHttpFacadeConfigError {
    InvalidBindAddr,
}

impl fmt::Display for OarHttpFacadeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBindAddr => {
                write!(f, "oar_http_facade_config_invalid: invalid_bind_addr")
            }
        }
    }
}

impl Error for OarHttpFacadeConfigError {}

impl OarHttpFacadeConfig {
    pub fn from_env_map(
        env: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, OarHttpFacadeConfigError> {
        let Some(raw_bind_addr) = env("OAR_HTTP_BIND_ADDR") else {
            return Ok(Self::default());
        };
        let bind_addr = raw_bind_addr
            .parse::<SocketAddr>()
            .map_err(|_| OarHttpFacadeConfigError::InvalidBindAddr)?;
        Ok(Self { bind_addr })
    }
}
