use thiserror::Error;

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

pub fn compose_encrypted_grant_blob(primary: Vec<u8>, renewal: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + primary.len() + renewal.len());
    out.extend_from_slice(&(primary.len() as u32).to_be_bytes());
    out.extend_from_slice(&primary);
    out.extend_from_slice(&(renewal.len() as u32).to_be_bytes());
    out.extend_from_slice(&renewal);
    out
}
