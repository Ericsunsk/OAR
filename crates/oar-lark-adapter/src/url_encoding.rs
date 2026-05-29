pub(crate) fn encode_query<I, K, V>(parts: I) -> String
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    // Keep ordered pairs so repeated Feishu query keys such as okr_ids are preserved.
    parts
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                percent_encode(key.as_ref()),
                percent_encode(value.as_ref())
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

pub(crate) fn percent_encode(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        if byte.is_ascii_alphanumeric() || [b'-', b'_', b'.', b'~'].contains(byte) {
            output.push(char::from(*byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_preserves_unreserved_and_encodes_utf8() {
        assert_eq!(percent_encode("abc-_.~XYZ09"), "abc-_.~XYZ09");
        assert_eq!(percent_encode("a b/飞"), "a%20b%2F%E9%A3%9E");
    }

    #[test]
    fn encode_query_preserves_order_and_repeated_keys() {
        let query = encode_query([
            ("okr_ids", "first"),
            ("okr_ids", "second value"),
            ("lang", "zh_cn"),
        ]);
        assert_eq!(query, "okr_ids=first&okr_ids=second%20value&lang=zh_cn");
    }
}
