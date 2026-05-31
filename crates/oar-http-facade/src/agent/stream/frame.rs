use bytes::Bytes;
use hyper::body::Frame;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    AgentFrameSendError, AgentFrameSender, AgentFrameStream, AgentStreamFrame,
    AGENT_STREAM_CHANNEL_SIZE,
};

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

pub(in crate::agent) async fn send_agent_stream_frames(
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
