use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedGrantMaterial {
    pub encrypted_primary: Vec<u8>,
    pub encrypted_renewal: Vec<u8>,
}

impl fmt::Debug for EncryptedGrantMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedGrantMaterial")
            .field("encrypted_primary", &"[REDACTED]")
            .field("encrypted_renewal", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedGrantBlob(pub Vec<u8>);

impl fmt::Debug for EncryptedGrantBlob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EncryptedGrantBlob")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl EncryptedGrantMaterial {
    pub fn into_blob(self) -> EncryptedGrantBlob {
        let primary_len = self.encrypted_primary.len() as u32;
        let renewal_len = self.encrypted_renewal.len() as u32;
        let mut out = Vec::with_capacity(8 + primary_len as usize + renewal_len as usize);
        out.extend_from_slice(&primary_len.to_be_bytes());
        out.extend_from_slice(&self.encrypted_primary);
        out.extend_from_slice(&renewal_len.to_be_bytes());
        out.extend_from_slice(&self.encrypted_renewal);
        EncryptedGrantBlob(out)
    }
}
