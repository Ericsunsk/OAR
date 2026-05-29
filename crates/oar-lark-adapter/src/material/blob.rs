use thiserror::Error;

use crate::crypto::decrypt_v1_envelope;
use crate::redaction::SecretString;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub(crate) enum BlobParseError {
    #[error("invalid encrypted grant blob")]
    Invalid,
}

pub(crate) fn parse_encrypted_grant_blob(blob: &[u8]) -> Result<(&[u8], &[u8]), BlobParseError> {
    let Some(primary_len_bytes) = blob.get(0..4) else {
        return Err(BlobParseError::Invalid);
    };
    let primary_len = u32::from_be_bytes(primary_len_bytes.try_into().expect("length checked"));
    let primary_len = primary_len as usize;

    let primary_start = 4usize;
    let primary_end = primary_start
        .checked_add(primary_len)
        .ok_or(BlobParseError::Invalid)?;
    let Some(primary) = blob.get(primary_start..primary_end) else {
        return Err(BlobParseError::Invalid);
    };

    let renewal_len_start = primary_end;
    let renewal_len_end = renewal_len_start
        .checked_add(4)
        .ok_or(BlobParseError::Invalid)?;
    let Some(renewal_len_bytes) = blob.get(renewal_len_start..renewal_len_end) else {
        return Err(BlobParseError::Invalid);
    };
    let renewal_len = u32::from_be_bytes(renewal_len_bytes.try_into().expect("length checked"));
    let renewal_len = renewal_len as usize;

    let renewal_start = renewal_len_end;
    let renewal_end = renewal_start
        .checked_add(renewal_len)
        .ok_or(BlobParseError::Invalid)?;
    let Some(renewal) = blob.get(renewal_start..renewal_end) else {
        return Err(BlobParseError::Invalid);
    };

    if renewal_end != blob.len() {
        return Err(BlobParseError::Invalid);
    }

    Ok((primary, renewal))
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum GrantAccessTokenReadError {
    #[error("invalid encrypted grant blob")]
    InvalidBlob,
    #[error("grant access token decrypt failed")]
    DecryptFailed,
    #[error("grant access token is not valid utf-8")]
    InvalidUtf8,
}

pub fn read_access_token_from_encrypted_grant(
    blob: &[u8],
    key_material: [u8; 32],
) -> Result<SecretString, GrantAccessTokenReadError> {
    let (primary, _) =
        parse_encrypted_grant_blob(blob).map_err(|_| GrantAccessTokenReadError::InvalidBlob)?;
    let token = decrypt_v1_envelope(&key_material, primary)
        .map_err(|_| GrantAccessTokenReadError::DecryptFailed)?;
    let token = String::from_utf8(token).map_err(|_| GrantAccessTokenReadError::InvalidUtf8)?;
    Ok(SecretString::new(token))
}

pub fn compose_encrypted_grant_blob(primary: Vec<u8>, renewal: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + primary.len() + renewal.len());
    out.extend_from_slice(&(primary.len() as u32).to_be_bytes());
    out.extend_from_slice(&primary);
    out.extend_from_slice(&(renewal.len() as u32).to_be_bytes());
    out.extend_from_slice(&renewal);
    out
}
