use crate::url_encoding::percent_encode;

const MAX_COMPONENT_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OkrKrSourceRef {
    pub okr_id: String,
    pub objective_id: String,
    pub kr_id: String,
}

impl OkrKrSourceRef {
    pub fn new(
        okr_id: impl Into<String>,
        objective_id: impl Into<String>,
        kr_id: impl Into<String>,
    ) -> Option<Self> {
        let source_ref = Self {
            okr_id: okr_id.into(),
            objective_id: objective_id.into(),
            kr_id: kr_id.into(),
        };
        if source_ref.valid() {
            Some(source_ref)
        } else {
            None
        }
    }

    pub fn source_ref(&self) -> String {
        format!(
            "okr://{}/objectives/{}/krs/{}",
            percent_encode(&self.okr_id),
            percent_encode(&self.objective_id),
            percent_encode(&self.kr_id)
        )
    }

    pub fn source_id(&self) -> String {
        format!(
            "okr:{}:objective:{}:kr:{}",
            percent_encode(&self.okr_id),
            percent_encode(&self.objective_id),
            percent_encode(&self.kr_id)
        )
    }

    fn valid(&self) -> bool {
        valid_decoded_component(&self.okr_id)
            && valid_decoded_component(&self.objective_id)
            && valid_decoded_component(&self.kr_id)
    }
}

pub fn parse_okr_kr_source_ref(source_ref: &str) -> Option<OkrKrSourceRef> {
    let trimmed = source_ref.trim();
    if let Some(path_like) = trimmed.strip_prefix("okr://") {
        return parse_path_style_ref(path_like);
    }
    if let Some(value) = trimmed.strip_prefix("okr:") {
        return parse_colon_style_ref(value);
    }
    None
}

fn parse_path_style_ref(value: &str) -> Option<OkrKrSourceRef> {
    let segments = value.split('/').collect::<Vec<_>>();
    if segments.len() != 5 {
        return None;
    }
    if segments[1] != "objectives" || segments[3] != "krs" {
        return None;
    }
    decode_ref(segments[0], segments[2], segments[4])
}

fn parse_colon_style_ref(value: &str) -> Option<OkrKrSourceRef> {
    let segments = value.split(':').collect::<Vec<_>>();
    if segments.len() != 5 {
        return None;
    }
    if segments[1] != "objective" || segments[3] != "kr" {
        return None;
    }
    decode_ref(segments[0], segments[2], segments[4])
}

fn decode_ref(okr_id: &str, objective_id: &str, kr_id: &str) -> Option<OkrKrSourceRef> {
    OkrKrSourceRef::new(
        decode_component(okr_id)?,
        decode_component(objective_id)?,
        decode_component(kr_id)?,
    )
}

fn decode_component(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let high = *bytes.get(index + 1)?;
                let low = *bytes.get(index + 2)?;
                decoded.push((hex_value(high)? << 4) | hex_value(low)?);
                index += 3;
            }
            byte if is_unreserved(byte) => {
                decoded.push(byte);
                index += 1;
            }
            _ => return None,
        }
    }

    String::from_utf8(decoded)
        .ok()
        .filter(|decoded| valid_decoded_component(decoded))
}

fn valid_decoded_component(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_COMPONENT_CHARS
        && value.chars().all(|character| !character.is_control())
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || [b'-', b'_', b'.', b'~'].contains(&byte)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_safe_ids_exactly_as_legacy_refs() {
        let source_ref = OkrKrSourceRef::new("okr_1", "obj_1", "kr_1").expect("valid safe ref");

        assert_eq!(
            source_ref.source_ref(),
            "okr://okr_1/objectives/obj_1/krs/kr_1"
        );
        assert_eq!(source_ref.source_id(), "okr:okr_1:objective:obj_1:kr:kr_1");
    }

    #[test]
    fn parser_accepts_legacy_path_and_colon_styles() {
        let path =
            parse_okr_kr_source_ref("okr://okr_demo/objectives/obj_demo/krs/kr_demo").expect("okr");
        assert_eq!(
            path,
            OkrKrSourceRef {
                okr_id: "okr_demo".to_string(),
                objective_id: "obj_demo".to_string(),
                kr_id: "kr_demo".to_string(),
            }
        );

        let colon =
            parse_okr_kr_source_ref("okr:okr_demo:objective:obj_demo:kr:kr_demo").expect("okr");
        assert_eq!(colon, path);
    }

    #[test]
    fn formatter_percent_encodes_reserved_component_bytes() {
        let source_ref =
            OkrKrSourceRef::new("okr:1", "obj/1", "kr a%?#:").expect("encoded-reserved ids");

        assert_eq!(
            source_ref.source_ref(),
            "okr://okr%3A1/objectives/obj%2F1/krs/kr%20a%25%3F%23%3A"
        );
        assert_eq!(
            source_ref.source_id(),
            "okr:okr%3A1:objective:obj%2F1:kr:kr%20a%25%3F%23%3A"
        );
    }

    #[test]
    fn parser_percent_decodes_components() {
        let path =
            parse_okr_kr_source_ref("okr://okr%3A1/objectives/obj%2F1/krs/kr%20a%25%3F%23%3A")
                .expect("path ref");
        assert_eq!(path.okr_id, "okr:1");
        assert_eq!(path.objective_id, "obj/1");
        assert_eq!(path.kr_id, "kr a%?#:");

        let colon = parse_okr_kr_source_ref("okr:okr%3A1:objective:obj%2F1:kr:kr%20a%25%3F%23%3A")
            .expect("colon ref");
        assert_eq!(colon, path);
    }

    #[test]
    fn parser_rejects_malformed_or_ambiguous_components() {
        assert!(parse_okr_kr_source_ref("okr://okr_demo/objectives/obj_demo").is_none());
        assert!(parse_okr_kr_source_ref("okr:okr_demo:obj:obj_demo:kr:kr_demo").is_none());
        assert!(parse_okr_kr_source_ref("okr://okr%/objectives/obj_demo/krs/kr_demo").is_none());
        assert!(parse_okr_kr_source_ref("okr://okr%G0/objectives/obj_demo/krs/kr_demo").is_none());
        assert!(parse_okr_kr_source_ref("okr://okr_demo/objectives//krs/kr_demo").is_none());
        assert!(parse_okr_kr_source_ref("okr://okr_demo/objectives/obj_demo/krs/kr:a").is_none());
        assert!(parse_okr_kr_source_ref("okr://okr_demo/objectives/obj_demo/krs/kr?a").is_none());
        assert!(parse_okr_kr_source_ref("okr:okr_demo:objective:obj_demo:kr:kr/a").is_none());
        assert!(parse_okr_kr_source_ref("okr:okr_demo:objective:obj demo:kr:kr_a").is_none());
        assert!(parse_okr_kr_source_ref("okr:okr_demo:objective:obj_demo:kr:kr%0Aa").is_none());
        assert!(parse_okr_kr_source_ref(&format!(
            "okr://{}/objectives/obj_demo/krs/kr_demo",
            "x".repeat(MAX_COMPONENT_CHARS + 1)
        ))
        .is_none());
    }
}
