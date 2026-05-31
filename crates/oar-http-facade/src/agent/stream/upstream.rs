use bytes::Bytes;
use tokio_stream::{Stream, StreamExt};

use super::frame::{agent_frame_channel, send_agent_error, send_agent_stream_frames};
use super::parser::{SseFrameParseError, SseFrameParseResult, SseFrameParser};
use super::{AgentFrameSendError, AgentFrameSender, AgentFrameStream, AgentStreamFrame};

pub(in crate::agent) fn spawn_upstream_sse_response<F>(
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

async fn stream_upstream_sse_response<S, F>(
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
            send_agent_error(sender, AgentStreamFrame::INVALID_UPSTREAM_EVENT_CODE).await
        }
    }
}
