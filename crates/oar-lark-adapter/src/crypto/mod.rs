mod clock;
mod encryptor;
mod envelope;

pub use clock::{GrantTimeSource, SystemGrantClock};
pub use encryptor::{AesGcmGrantEncryptor, AesGcmGrantEncryptorError};
pub(crate) use envelope::decrypt_v1_envelope;
pub use envelope::AesGcmGrantDecryptError;
#[cfg(test)]
pub(crate) use envelope::{ENVELOPE_VERSION_V1, NONCE_LEN_V1};

#[cfg(test)]
mod tests;
