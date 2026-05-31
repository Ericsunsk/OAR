use bytes::Bytes;
use hyper::body::Frame;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

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
    let event = payload
        .get("event")
        .and_then(Value::as_str)
        .map(|event| format!("event: {event}\n"))
        .unwrap_or_default();
    sender
        .send(Ok(Frame::data(Bytes::from(format!(
            "{event}data: {payload}\n\n"
        )))))
        .await
}

pub(super) fn agent_frame_channel() -> (AgentFrameSender, AgentFrameStream) {
    let (sender, receiver) = mpsc::channel(AGENT_STREAM_CHANNEL_SIZE);
    (sender, ReceiverStream::new(receiver))
}

pub(crate) fn prepend_agent_context_status_frame(
    stream: AgentFrameStream,
    context_status: super::super::status::AgentContextStatus,
) -> AgentFrameStream {
    if context_status.is_empty() {
        return stream;
    }
    prepend_agent_stream_frame(stream, AgentStreamFrame::ContextStatus(context_status))
}

fn prepend_agent_stream_frame(
    mut stream: AgentFrameStream,
    frame: AgentStreamFrame,
) -> AgentFrameStream {
    let (sender, receiver) = agent_frame_channel();
    tokio::spawn(async move {
        if send_agent_stream_frames(&sender, vec![frame])
            .await
            .is_err()
        {
            return;
        }
        while let Some(item) = stream.next().await {
            if sender.send(item).await.is_err() {
                return;
            }
        }
    });
    receiver
}

pub(in crate::agent) async fn send_agent_stream_frames(
    sender: &AgentFrameSender,
    frames: Vec<AgentStreamFrame>,
) -> Result<(), AgentFrameSendError> {
    for frame in frames {
        match frame {
            AgentStreamFrame::ContextStatus(status) => {
                send_agent_frame(
                    sender,
                    json!({
                        "event": "context_status",
                        "status": status
                    }),
                )
                .await?;
            }
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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::agent::status::AgentContextStatus;

    #[tokio::test]
    async fn context_status_frame_is_sent_before_upstream_frames() {
        let (upstream_sender, upstream) = agent_frame_channel();
        let mut stream = prepend_agent_context_status_frame(
            upstream,
            AgentContextStatus {
                activated_skill_summaries: vec![
                    "feishu.okr｜Feishu OKR｜用途：读取 OKR".to_string()
                ],
                live_read_summaries: vec![
                    "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。".to_string(),
                ],
            },
        );
        send_agent_stream_frames(
            &upstream_sender,
            vec![AgentStreamFrame::Delta("收到".to_string())],
        )
        .await
        .expect("send upstream frame");
        drop(upstream_sender);

        let first = next_frame(&mut stream).await;
        let second = next_frame(&mut stream).await;

        assert!(first.raw.starts_with("event: context_status\n"));
        assert_eq!(first.payload["event"], "context_status");
        assert_eq!(
            first.payload["status"]["activated_skill_summaries"][0],
            "feishu.okr｜Feishu OKR｜用途：读取 OKR"
        );
        assert_eq!(
            first.payload["status"]["live_read_summaries"][0],
            "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。"
        );
        assert!(second.raw.starts_with("event: delta\n"));
        assert_eq!(second.payload["event"], "delta");
        assert_eq!(second.payload["delta"], "收到");
    }

    #[tokio::test]
    async fn empty_context_status_is_not_prepended() {
        let (upstream_sender, upstream) = agent_frame_channel();
        let mut stream = prepend_agent_context_status_frame(
            upstream,
            AgentContextStatus {
                activated_skill_summaries: vec![],
                live_read_summaries: vec![],
            },
        );
        send_agent_stream_frames(
            &upstream_sender,
            vec![AgentStreamFrame::Delta("收到".to_string())],
        )
        .await
        .expect("send upstream frame");
        drop(upstream_sender);

        let first = next_frame(&mut stream).await;

        assert_eq!(first.payload["event"], "delta");
        assert_eq!(first.payload["delta"], "收到");
    }

    struct SseFrame {
        raw: String,
        payload: Value,
    }

    async fn next_frame(stream: &mut AgentFrameStream) -> SseFrame {
        let frame = stream
            .next()
            .await
            .expect("stream item")
            .expect("frame result");
        let bytes = frame.into_data().expect("data frame");
        let text = String::from_utf8(bytes.to_vec()).expect("utf8");
        let payload = text
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("sse data payload")
            .to_string();
        SseFrame {
            raw: text,
            payload: serde_json::from_str(&payload).expect("json payload"),
        }
    }
}
