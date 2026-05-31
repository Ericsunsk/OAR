#[derive(Default)]
pub(super) struct SseFrameParser {
    buffer: Vec<u8>,
}

impl SseFrameParser {
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Vec<SseFrameParseResult> {
        self.buffer.extend_from_slice(bytes);
        self.drain_complete_frames()
    }

    pub(super) fn finish(&mut self) -> Vec<SseFrameParseResult> {
        if self.buffer.iter().all(u8::is_ascii_whitespace) {
            self.buffer.clear();
            return vec![];
        }
        let remaining = std::mem::take(&mut self.buffer);
        vec![decode_sse_frame(remaining)]
    }

    fn drain_complete_frames(&mut self) -> Vec<SseFrameParseResult> {
        let mut frames = Vec::new();
        while let Some((index, separator_len)) = next_sse_boundary(&self.buffer) {
            let frame = self.buffer[..index].to_vec();
            self.buffer.drain(..index + separator_len);
            frames.push(decode_sse_frame(frame));
        }
        frames
    }
}

pub(super) type SseFrameParseResult = Result<String, SseFrameParseError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SseFrameParseError {
    InvalidUtf8,
}

fn decode_sse_frame(frame: Vec<u8>) -> SseFrameParseResult {
    String::from_utf8(frame).map_err(|_| SseFrameParseError::InvalidUtf8)
}

fn next_sse_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    match (
        find_byte_sequence(buffer, b"\r\n\r\n"),
        find_byte_sequence(buffer, b"\n\n"),
    ) {
        (Some(crlf), Some(lf)) if crlf < lf => Some((crlf, 4)),
        (Some(_), Some(lf)) => Some((lf, 2)),
        (Some(crlf), None) => Some((crlf, 4)),
        (None, Some(lf)) => Some((lf, 2)),
        (None, None) => None,
    }
}

fn find_byte_sequence(buffer: &[u8], needle: &[u8]) -> Option<usize> {
    buffer
        .windows(needle.len())
        .position(|window| window == needle)
}

pub(in crate::agent) fn sse_data_payload(frame: &str) -> Option<String> {
    let data_lines = frame
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            let value = line.strip_prefix("data:")?;
            Some(value.strip_prefix(' ').unwrap_or(value).to_string())
        })
        .collect::<Vec<_>>();
    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_extracts_data_payload_frames() {
        let mut parser = SseFrameParser::default();
        let frames = parser
            .feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n");

        assert_eq!(frames.len(), 2);
        let frames = frames
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("utf8 frames");
        assert_eq!(
            sse_data_payload(&frames[0]).expect("payload"),
            "{\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}"
        );
        assert_eq!(sse_data_payload(&frames[1]).expect("done"), "[DONE]");
    }

    #[test]
    fn sse_data_payload_joins_multiple_data_lines() {
        assert_eq!(
            sse_data_payload("event: message\ndata: first\r\ndata: second\nid: 1"),
            Some("first\nsecond".to_string())
        );
    }

    #[test]
    fn sse_parser_preserves_utf8_split_across_chunks() {
        let mut parser = SseFrameParser::default();
        let frame = "data: {\"choices\":[{\"delta\":{\"content\":\"你好🙂\"}}]}\n\n";
        let split = frame.find('🙂').expect("emoji byte index") + 1;
        let bytes = frame.as_bytes();

        assert!(std::str::from_utf8(&bytes[..split]).is_err());
        assert!(parser.feed(&bytes[..split]).is_empty());

        let frames = parser.feed(&bytes[split..]);

        assert_eq!(frames.len(), 1);
        let parsed = frames
            .into_iter()
            .next()
            .expect("frame")
            .expect("utf8 frame");
        assert_eq!(
            sse_data_payload(&parsed).expect("payload"),
            "{\"choices\":[{\"delta\":{\"content\":\"你好🙂\"}}]}"
        );
    }

    #[test]
    fn sse_parser_keeps_crlf_frame_boundaries() {
        let mut parser = SseFrameParser::default();
        let frames = parser.feed(b"data: one\r\n\r\ndata: two\n\n");
        let frames = frames
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("utf8 frames");

        assert_eq!(frames, vec!["data: one", "data: two"]);
    }

    #[test]
    fn sse_parser_keeps_unterminated_frame_on_finish() {
        let mut parser = SseFrameParser::default();

        assert!(parser.feed(b"data: partial").is_empty());
        assert_eq!(parser.finish(), vec![Ok("data: partial".to_string())]);
    }

    #[test]
    fn sse_parser_drops_whitespace_remainder_on_finish() {
        let mut parser = SseFrameParser::default();

        assert!(parser.feed(b" \r\n\t").is_empty());
        assert!(parser.finish().is_empty());
    }

    #[test]
    fn sse_parser_reports_invalid_utf8_after_complete_frame() {
        let mut parser = SseFrameParser::default();

        assert_eq!(
            parser.feed(b"data: \xFF\n\n"),
            vec![Err(SseFrameParseError::InvalidUtf8)]
        );
    }
}
