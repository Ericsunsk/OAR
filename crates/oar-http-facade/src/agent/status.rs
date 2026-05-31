use serde::Serialize;

use super::context_text::safe_prompt_context_text;
pub(crate) use super::live_context::status::LiveFeishuReadState as AgentLiveReadState;
use super::live_context::status::LiveFeishuReadStatus as LiveFeishuReadStatusRecord;
use super::request::{AgentConversationContextDTO, AgentStreamRequest};
use super::skills::AgentSkill;

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

impl AgentContextStatus {
    pub(crate) fn from_request(request: &AgentStreamRequest) -> Self {
        Self::from_context(&request.context)
    }

    pub(in crate::agent) fn from_context(context: &AgentConversationContextDTO) -> Self {
        Self {
            activated_skills: safe_activated_skill_statuses(
                &context.activated_skill_statuses,
                AGENT_CONTEXT_STATUS_SUMMARY_LIMIT,
            ),
            live_reads: safe_live_read_statuses(
                &context.live_feishu_read_statuses,
                AGENT_CONTEXT_STATUS_SUMMARY_LIMIT,
            ),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.activated_skills.is_empty() && self.live_reads.is_empty()
    }
}

impl AgentActivatedSkillStatus {
    pub(in crate::agent) fn from_skill(skill: AgentSkill) -> Self {
        let spec = skill.spec();
        let tools = spec
            .tools
            .iter()
            .map(|tool| tool.spec().name)
            .collect::<Vec<_>>()
            .join("；");
        Self {
            id: spec.id.to_string(),
            name: spec.display_name.to_string(),
            summary: format!(
                "{}｜{}｜用途：{}｜可用后端工具：{}",
                spec.id, spec.display_name, spec.purpose, tools
            ),
        }
    }

    fn sanitized(&self) -> Option<Self> {
        let id = compact_status_id(&self.id).unwrap_or_else(|| FALLBACK_SKILL_ID.to_string());
        let name = safe_prompt_context_text(&self.name).unwrap_or_else(|| id.clone());
        let summary = safe_prompt_context_text(&self.summary)?;
        Some(Self { id, name, summary })
    }
}

impl AgentLiveReadStatus {
    fn from_live_read_status(status: &LiveFeishuReadStatusRecord, summary: String) -> Self {
        let tool = status.tool;
        let id = tool
            .map(|tool| tool.spec().name.to_string())
            .unwrap_or_else(|| FALLBACK_LIVE_READ_ID.to_string());
        let label = tool
            .map(|tool| tool.spec().name.to_string())
            .unwrap_or_else(|| "Feishu live read".to_string());
        Self {
            id,
            label,
            state: status.state,
            summary,
        }
    }
}

fn compact_status_id(value: &str) -> Option<String> {
    let id = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!id.is_empty()).then_some(id)
}

fn safe_activated_skill_statuses(
    statuses: &[AgentActivatedSkillStatus],
    limit: usize,
) -> Vec<AgentActivatedSkillStatus> {
    let mut seen_ids = std::collections::HashSet::new();
    statuses
        .iter()
        .filter_map(AgentActivatedSkillStatus::sanitized)
        .filter(|status| seen_ids.insert(status.id.to_ascii_lowercase()))
        .take(limit)
        .collect()
}

fn safe_live_read_statuses(
    statuses: &[LiveFeishuReadStatusRecord],
    limit: usize,
) -> Vec<AgentLiveReadStatus> {
    let mut seen_summaries = std::collections::HashSet::new();
    statuses
        .iter()
        .filter_map(|status| {
            let summary = safe_prompt_context_text(&status.summary)?;
            if seen_summaries.insert(live_read_status_key(status, &summary)) {
                Some(AgentLiveReadStatus::from_live_read_status(status, summary))
            } else {
                None
            }
        })
        .take(limit)
        .collect()
}

fn live_read_status_key(status: &LiveFeishuReadStatusRecord, summary: &str) -> String {
    let id = status
        .tool
        .map(|tool| tool.spec().name)
        .unwrap_or(FALLBACK_LIVE_READ_ID);
    let state = match status.state {
        AgentLiveReadState::Ready => "ready",
        AgentLiveReadState::Degraded => "degraded",
    };
    let summary = summary
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    format!("{id}:{state}:{summary}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO, AgentStreamRequest};
    use crate::agent::tools::AgentReadTool;

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
                live_feishu_read_statuses: vec![
                    LiveFeishuReadStatusRecord::ready_for_tool(
                        AgentReadTool::OkrSummary,
                        "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。".to_string(),
                    ),
                    LiveFeishuReadStatusRecord::degraded("refresh_token rt_live_fake".to_string()),
                    LiveFeishuReadStatusRecord::ready_for_tool(
                        AgentReadTool::OkrSummary,
                        "工具 feishu.okr.summarize_my_okr｜实时：读取到 2 条目标。".to_string(),
                    ),
                ],
                activated_skill_summaries: vec![
                    "feishu.okr｜Feishu OKR｜用途：读取 OKR".to_string(),
                    "  feishu.okr｜Feishu OKR｜用途：读取   OKR  ".to_string(),
                ],
                activated_skill_statuses: vec![
                    AgentActivatedSkillStatus::from_skill(AgentSkill::Okr),
                    AgentActivatedSkillStatus {
                        id: " feishu.okr ".to_string(),
                        name: "Feishu OKR".to_string(),
                        summary: "  feishu.okr｜Feishu OKR｜用途：读取   OKR  ".to_string(),
                    },
                ],
            },
        };
        request.context.live_feishu_read_summaries.extend(
            ["实时 3", "实时 4", "实时 5"]
                .into_iter()
                .map(str::to_string),
        );
        request.context.live_feishu_read_statuses.extend(
            ["实时 3", "实时 4", "实时 5"]
                .into_iter()
                .map(|summary| LiveFeishuReadStatusRecord::ready(summary.to_string())),
        );

        let status = AgentContextStatus::from_request(&request);

        assert_eq!(
            status.activated_skills[0].summary,
            "feishu.okr｜Feishu OKR｜用途：理解用户关于本人飞书 OKR、目标、关键结果、数量、内容概览和 progress 进展的查询。｜可用后端工具：feishu.okr.summarize_my_okr；feishu.okr.summarize_my_progress"
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
        assert_eq!(status.live_reads[1].state, AgentLiveReadState::Degraded);
        assert!(!format!("{status:?}").contains("rt_live_fake"));
    }

    #[test]
    fn context_status_uses_typed_live_read_state_instead_of_summary_text() {
        let request = AgentStreamRequest {
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
                    "工具 feishu.okr.summarize_my_okr｜实时读取降级：旧字段不应驱动 SSE。"
                        .to_string(),
                ],
                live_feishu_read_statuses: vec![LiveFeishuReadStatusRecord::ready_for_tool(
                    AgentReadTool::OkrSummary,
                    "工具 feishu.okr.summarize_my_okr｜实时：摘要文本里出现 降级 也保持 typed ready。"
                        .to_string(),
                )],
                activated_skill_statuses: vec![],
                activated_skill_summaries: vec![],
            },
        };

        let status = AgentContextStatus::from_request(&request);

        assert_eq!(status.live_reads.len(), 1);
        assert_eq!(status.live_reads[0].state, AgentLiveReadState::Ready);
        assert!(status.live_reads[0].summary.contains("typed ready"));
    }
}
