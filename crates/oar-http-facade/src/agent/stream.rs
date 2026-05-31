mod frame;
mod parser;
mod upstream;

use std::convert::Infallible;

use bytes::Bytes;
use hyper::body::Frame;

pub(crate) use self::frame::prepend_agent_context_status_frame;
#[cfg(test)]
pub(super) use self::frame::send_agent_stream_frames;
pub(super) use self::parser::sse_data_payload;
pub(super) use self::upstream::spawn_upstream_sse_response;
use super::status::AgentContextStatus;

const AGENT_STREAM_CHANNEL_SIZE: usize = 16;
pub(crate) type AgentFrameStream =
    tokio_stream::wrappers::ReceiverStream<Result<Frame<Bytes>, Infallible>>;
pub(super) type AgentFrameSender = tokio::sync::mpsc::Sender<Result<Frame<Bytes>, Infallible>>;
pub(super) type AgentFrameSendError =
    tokio::sync::mpsc::error::SendError<Result<Frame<Bytes>, Infallible>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AgentStreamFrame {
    ContextStatus(AgentContextStatus),
    Delta(String),
    Completed,
    Error(&'static str),
}

impl AgentStreamFrame {
    pub(super) const INVALID_UPSTREAM_EVENT_CODE: &'static str = "invalid_upstream_event";

    pub(super) fn invalid_upstream_event() -> Vec<Self> {
        vec![Self::Error(Self::INVALID_UPSTREAM_EVENT_CODE)]
    }
}
