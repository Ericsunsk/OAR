use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

pub(super) fn stable_prefixed_id(prefix: &str, parts: &[&str]) -> String {
    const MAX_FRAGMENT_CHARS: usize = 96;

    let fragment = parts
        .iter()
        .map(|part| sanitize_id_fragment(part))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let digest = stable_sha256_hex(parts);
    if fragment.is_empty() {
        return format!("{prefix}_{}", &digest[..16]);
    }
    if fragment.chars().count() <= MAX_FRAGMENT_CHARS {
        return format!("{prefix}_{fragment}");
    }

    let shortened = fragment
        .chars()
        .take(MAX_FRAGMENT_CHARS)
        .collect::<String>();
    format!("{prefix}_{shortened}_{}", &digest[..16])
}

fn sanitize_id_fragment(value: &str) -> String {
    let trimmed = value.trim().trim_matches('_');
    let mut out = String::with_capacity(trimmed.len());
    let mut previous_was_separator = false;
    for character in trimmed.chars() {
        let next = if character.is_ascii_alphanumeric() {
            previous_was_separator = false;
            Some(character)
        } else if character == '-' || character == '_' {
            if previous_was_separator {
                None
            } else {
                previous_was_separator = true;
                Some('_')
            }
        } else if previous_was_separator {
            None
        } else {
            previous_was_separator = true;
            Some('_')
        };
        if let Some(next) = next {
            out.push(next);
        }
    }
    out.trim_matches('_').to_string()
}

pub(super) fn stable_sha256_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub(super) fn system_time_to_ms_lossy(time: SystemTime) -> u64 {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis();
    millis.min(u128::from(u64::MAX)) as u64
}

pub(super) fn secure_random_hex(bytes_len: usize) -> std::io::Result<String> {
    let mut bytes = vec![0_u8; bytes_len];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

pub(super) fn sanitize_session_suffix(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    if sanitized.is_empty() {
        "fallback".to_string()
    } else {
        sanitized
    }
}

pub(super) fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let raw = value.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        match raw[index] {
            b'+' => {
                bytes.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < raw.len() => {
                let hex = std::str::from_utf8(&raw[index + 1..index + 3]).ok()?;
                let decoded = u8::from_str_radix(hex, 16).ok()?;
                bytes.push(decoded);
                index += 3;
            }
            b'%' => return None,
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(bytes).ok()
}

pub(crate) fn iso8601_utc(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}
