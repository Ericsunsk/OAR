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
                    if send_agent_stream_frames(&sender, map_frame(&frame))
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
        if send_agent_stream_frames(&sender, map_frame(&frame))
            .await
            .is_err()
        {
            return;
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
    buffer: String,
}

impl SseFrameParser {
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.push_str(&String::from_utf8_lossy(bytes));
        self.drain_complete_frames()
    }

    pub(super) fn finish(&mut self) -> Vec<String> {
        if self.buffer.trim().is_empty() {
            self.buffer.clear();
            return vec![];
        }
        let remaining = std::mem::take(&mut self.buffer);
        vec![remaining]
    }

    fn drain_complete_frames(&mut self) -> Vec<String> {
        let mut frames = Vec::new();
        while let Some((index, separator_len)) = next_sse_boundary(&self.buffer) {
            let frame = self.buffer[..index].to_string();
            self.buffer.drain(..index + separator_len);
            frames.push(frame);
        }
        frames
    }
}

fn next_sse_boundary(buffer: &str) -> Option<(usize, usize)> {
    match (buffer.find("\r\n\r\n"), buffer.find("\n\n")) {
        (Some(crlf), Some(lf)) if crlf < lf => Some((crlf, 4)),
        (Some(_), Some(lf)) => Some((lf, 2)),
        (Some(crlf), None) => Some((crlf, 4)),
        (None, Some(lf)) => Some((lf, 2)),
        (None, None) => None,
    }
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
        assert_eq!(
            sse_data_payload(&frames[0]).expect("payload"),
            "{\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}"
        );
        assert_eq!(sse_data_payload(&frames[1]).expect("done"), "[DONE]");
    }
}
