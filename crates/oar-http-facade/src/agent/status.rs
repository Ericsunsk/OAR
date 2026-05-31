use serde::Serialize;

use super::context_text::safe_context_summaries;
use super::request::{AgentConversationContextDTO, AgentStreamRequest};
use super::tools::AgentReadTool;

const AGENT_CONTEXT_STATUS_SUMMARY_LIMIT: usize = 4;
const FALLBACK_SKILL_ID: &str = "agent.skill";
const FALLBACK_LIVE_READ_ID: &str = "feishu.live_read";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentContextStatus {
    pub(crate) activated_skills: Vec<AgentActivatedSkillStatus>,
    pub(crate) live_reads: Vec<AgentLiveReadStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentActivatedSkillStatus {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AgentLiveReadStatus {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) state: AgentLiveReadState,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentLiveReadState {
    Ready,
    Degraded,
}

impl AgentContextStatus {
    pub(crate) fn from_request(request: &AgentStreamRequest) -> Self {
        Self::from_context(&request.context)
    }

    pub(in crate::agent) fn from_context(context: &AgentConversationContextDTO) -> Self {
        Self {
            activated_skills: safe_context_summaries(
                &context.activated_skill_summaries,
                AGENT_CONTEXT_STATUS_SUMMARY_LIMIT,
            )
            .into_iter()
            .map(AgentActivatedSkillStatus::from_summary)
            .collect(),
            live_reads: safe_context_summaries(
                &context.live_feishu_read_summaries,
                AGENT_CONTEXT_STATUS_SUMMARY_LIMIT,
            )
            .into_iter()
            .map(AgentLiveReadStatus::from_summary)
            .collect(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.activated_skills.is_empty() && self.live_reads.is_empty()
    }
}

impl AgentActivatedSkillStatus {
    fn from_summary(summary: String) -> Self {
        let mut parts = summary.split('｜').map(str::trim);
        let id = non_empty_string(parts.next()).unwrap_or_else(|| FALLBACK_SKILL_ID.to_string());
        let name = non_empty_string(parts.next()).unwrap_or_else(|| id.clone());
        Self { id, name, summary }
    }
}

impl AgentLiveReadStatus {
    fn from_summary(summary: String) -> Self {
        let tool = tool_from_summary(&summary);
        let id = tool
            .map(|tool| tool.spec().name.to_string())
            .unwrap_or_else(|| FALLBACK_LIVE_READ_ID.to_string());
        let label = tool
            .map(|tool| tool.spec().name.to_string())
            .unwrap_or_else(|| "Feishu live read".to_string());
        Self {
            id,
            label,
            state: AgentLiveReadState::from_summary(&summary),
            summary,
        }
    }
}

impl AgentLiveReadState {
    fn from_summary(summary: &str) -> Self {
        if summary.contains("降级")
            || summary.contains("失败")
            || summary.contains("缺少权限")
            || summary.contains("未配置")
            || summary.contains("未读取到")
            || summary.contains("无法")
        {
            return Self::Degraded;
        }
        Self::Ready
    }
}

fn tool_from_summary(summary: &str) -> Option<AgentReadTool> {
    let tool_name = summary
        .strip_prefix("工具 ")?
        .split_once('｜')
        .map(|(tool_name, _)| tool_name.trim())
        .unwrap_or_else(|| summary.trim_start_matches("工具 ").trim());
    AgentReadTool::from_name(tool_name)
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO, AgentStreamRequest};

    #[test]
    fn context_status_sanitizes_server_owned_live_context_for_sse() {
        let mut request = AgentStreamRequest {
            messages: vec![AgentMessageDTO {
                role: "user".to_string(),
                text: "查 OKR".to_string(),
            }],
            context: AgentConversationContextDTO {
                title: "KR 风险".to_string(),
                risk_reason: "连续延期".to_string(),
                action_summary: "更新进展".to_string(),
                evidence_summaries: vec![],
                evidence_refs: vec![],
                workspace_summary: "摘要".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
                ledger_event_summaries: vec![],
                live_feishu_read_summaries: vec![
                    "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。".to_string(),
                    "refresh_token rt_live_fake".to_string(),
                    "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。".to_string(),
                ],
                activated_skill_summaries: vec![
                    "feishu.okr｜Feishu OKR｜用途：读取 OKR".to_string(),
                    "  feishu.okr｜Feishu OKR｜用途：读取   OKR  ".to_string(),
                ],
            },
        };
        request.context.live_feishu_read_summaries.extend(
            ["实时 3", "实时 4", "实时 5"]
                .into_iter()
                .map(str::to_string),
        );

        let status = AgentContextStatus::from_request(&request);

        assert_eq!(
            status.activated_skills[0].summary,
            "feishu.okr｜Feishu OKR｜用途：读取 OKR"
        );
        assert_eq!(status.activated_skills[0].id, "feishu.okr");
        assert_eq!(status.activated_skills[0].name, "Feishu OKR");
        assert_eq!(
            status
                .live_reads
                .iter()
                .map(|read| read.summary.as_str())
                .collect::<Vec<_>>(),
            vec![
                "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。",
                "已隐藏敏感摘要。",
                "实时 3",
                "实时 4"
            ]
        );
        assert_eq!(status.live_reads[0].id, "feishu.okr.summarize_my_okr");
        assert_eq!(status.live_reads[0].state, AgentLiveReadState::Ready);
        assert_eq!(status.live_reads[1].state, AgentLiveReadState::Ready);
        assert!(!format!("{status:?}").contains("rt_live_fake"));
    }
}
