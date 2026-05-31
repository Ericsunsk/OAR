use std::convert::Infallible;

use bytes::Bytes;
use hyper::body::Frame;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

pub(super) const AGENT_STREAM_CHANNEL_SIZE: usize = 16;
pub(crate) type AgentFrameStream = ReceiverStream<Result<Frame<Bytes>, Infallible>>;
pub(super) type AgentFrameSender = mpsc::Sender<Result<Frame<Bytes>, Infallible>>;
pub(super) type AgentFrameSendError = mpsc::error::SendError<Result<Frame<Bytes>, Infallible>>;

pub(super) async fn send_agent_error(
    sender: &AgentFrameSender,
    code: &'static str,
) -> Result<(), AgentFrameSendError> {
    send_agent_frame(
        sender,
        json!({
            "event": "error",
            "error": code
        }),
    )
    .await
}

pub(super) async fn send_agent_frame(
    sender: &AgentFrameSender,
    payload: Value,
) -> Result<(), AgentFrameSendError> {
    sender
        .send(Ok(Frame::data(Bytes::from(format!("data: {payload}\n\n")))))
        .await
}

pub(super) fn agent_frame_channel() -> (AgentFrameSender, AgentFrameStream) {
    let (sender, receiver) = mpsc::channel(AGENT_STREAM_CHANNEL_SIZE);
    (sender, ReceiverStream::new(receiver))
}

pub(super) fn spawn_upstream_sse_response<F>(
    response: reqwest::Response,
    map_frame: F,
) -> AgentFrameStream
where
    F: FnMut(&str) -> Vec<AgentStreamFrame> + Send + 'static,
{
    let (sender, receiver) = agent_frame_channel();
    tokio::spawn(stream_upstream_sse_response(
        response.bytes_stream(),
        sender,
        map_frame,
    ));
    receiver
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AgentStreamFrame {
    Delta(String),
    Completed,
    Error(&'static str),
}

pub(super) async fn stream_upstream_sse_response<S, F>(
    mut upstream: S,
    sender: AgentFrameSender,
    mut map_frame: F,
) where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin + Send + 'static,
    F: FnMut(&str) -> Vec<AgentStreamFrame> + Send + 'static,
{
    let mut parser = SseFrameParser::default();
    while let Some(chunk) = upstream.next().await {
        match chunk {
            Ok(bytes) => {
                for frame in parser.feed(&bytes) {
                    if forward_parsed_sse_frame(&sender, frame, &mut map_frame)
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
            Err(_) => {
                let _ = send_agent_error(&sender, "upstream_unavailable").await;
                return;
            }
        }
    }

    for frame in parser.finish() {
        if forward_parsed_sse_frame(&sender, frame, &mut map_frame)
            .await
            .is_err()
        {
            return;
        }
    }
}

async fn forward_parsed_sse_frame<F>(
    sender: &AgentFrameSender,
    frame: SseFrameParseResult,
    map_frame: &mut F,
) -> Result<(), AgentFrameSendError>
where
    F: FnMut(&str) -> Vec<AgentStreamFrame>,
{
    match frame {
        Ok(frame) => send_agent_stream_frames(sender, map_frame(&frame)).await,
        Err(SseFrameParseError::InvalidUtf8) => {
            send_agent_error(sender, "invalid_upstream_event").await
        }
    }
}

pub(super) async fn send_agent_stream_frames(
    sender: &AgentFrameSender,
    frames: Vec<AgentStreamFrame>,
) -> Result<(), AgentFrameSendError> {
    for frame in frames {
        match frame {
            AgentStreamFrame::Delta(delta) => {
                send_agent_frame(
                    sender,
                    json!({
                        "event": "delta",
                        "delta": delta
                    }),
                )
                .await?;
            }
            AgentStreamFrame::Completed => {
                send_agent_frame(sender, json!({ "event": "completed" })).await?;
            }
            AgentStreamFrame::Error(code) => {
                send_agent_error(sender, code).await?;
            }
        }
    }
    Ok(())
}

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

type SseFrameParseResult = Result<String, SseFrameParseError>;

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

pub(super) fn sse_data_payload(frame: &str) -> Option<String> {
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
    fn sse_parser_maps_openai_chunks_to_agent_frames() {
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
