use std::fmt;

use secrecy::ExposeSecret;

#[derive(Clone)]
pub struct SecretString(secrecy::SecretString);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(secrecy::SecretString::new(value.into().into_boxed_str()))
    }

    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}
