use super::catalog::AgentSkill;
use crate::agent::request::AgentStreamRequest;

pub(in crate::agent) fn select_skills(request: &AgentStreamRequest) -> Vec<AgentSkill> {
    if latest_user_requests_feishu_okr_summary(request) {
        return vec![AgentSkill::FeishuOkr];
    }

    vec![]
}

pub(super) fn latest_user_requests_feishu_okr_summary(request: &AgentStreamRequest) -> bool {
    let Some(latest_user_text) = latest_user_text(request) else {
        return false;
    };

    latest_user_has_explicit_self_okr_read_intent(latest_user_text)
        || (latest_user_has_contextual_feishu_count_intent(latest_user_text)
            && request_has_recent_okr_topic(request))
}

fn latest_user_text(request: &AgentStreamRequest) -> Option<&str> {
    request
        .recent_messages()
        .filter(|message| message.role == "user")
        .filter_map(|message| {
            let text = message.text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .last()
}

fn latest_user_has_explicit_self_okr_read_intent(text: &str) -> bool {
    mentions_okr(text)
        && asks_to_read(text)
        && is_self_scoped(text)
        && !targets_non_self(text)
        && !mentions_non_okr_goal_context(text)
}

fn latest_user_has_contextual_feishu_count_intent(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    mentions_feishu(text)
        && is_self_scoped(text)
        && !targets_non_self(text)
        && !mentions_non_okr_feishu_domain(text)
        && (asks_to_count(text)
            || contains_latin_token(&normalized, "current")
            || text.contains("目前"))
}

fn request_has_recent_okr_topic(request: &AgentStreamRequest) -> bool {
    request
        .recent_messages()
        .any(|message| mentions_okr(&message.text))
        || mentions_okr(&request.context.title)
        || mentions_okr(&request.context.risk_reason)
        || mentions_okr(&request.context.action_summary)
        || mentions_okr(&request.context.workspace_summary)
        || request
            .context
            .workspace_signals
            .iter()
            .any(|value| mentions_okr(value))
        || request
            .context
            .evidence_summaries
            .iter()
            .any(|value| mentions_okr(value))
        || request.context.evidence_refs.iter().any(|evidence_ref| {
            mentions_okr(&evidence_ref.source_type)
                || mentions_okr(&evidence_ref.source_ref)
                || mentions_okr(&evidence_ref.summary)
        })
}

fn mentions_okr(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    contains_latin_token(&normalized, "okr")
        || contains_latin_token(&normalized, "okrs")
        || contains_latin_token(&normalized, "kr")
        || contains_latin_token(&normalized, "krs")
        || text.contains("关键结果")
        || text.contains("飞书 OKR")
        || text.contains("飞书okr")
        || text.contains("飞书目标")
}

fn mentions_feishu(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("飞书")
        || contains_latin_token(&normalized, "feishu")
        || contains_latin_token(&normalized, "lark")
}

fn asks_to_read(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("查")
        || text.contains("看")
        || text.contains("读")
        || text.contains("有没有")
        || text.contains("是否")
        || contains_latin_token(&normalized, "show")
        || contains_latin_token(&normalized, "list")
        || contains_latin_token(&normalized, "read")
        || asks_to_count(text)
}

fn asks_to_count(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("几条")
        || text.contains("多少条")
        || text.contains("多少个")
        || text.contains("条数")
        || text.contains("数量")
        || text.contains("总数")
        || contains_latin_token(&normalized, "count")
        || normalized.contains("how many")
}

fn is_self_scoped(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    let compact = text.split_whitespace().collect::<String>();
    let compact_normalized = compact.to_ascii_lowercase();
    text.contains("我的")
        || text.contains("本人")
        || text.contains("我飞书")
        || compact_normalized.contains("我okr")
        || compact_normalized.contains("我kr")
        || contains_latin_token(&normalized, "my")
}

fn targets_non_self(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    text.contains("团队")
        || text.contains("我们")
        || text.contains("部门")
        || text.contains("别人")
        || text.contains("其他人")
        || text.contains("同事")
        || text.contains("他的")
        || text.contains("她的")
        || text.contains("他人")
        || text.contains("她人")
        || contains_latin_token(&normalized, "team")
        || contains_latin_token(&normalized, "teammate")
        || contains_latin_token(&normalized, "teammates")
        || contains_latin_token(&normalized, "department")
        || contains_latin_token(&normalized, "dept")
        || contains_latin_token(&normalized, "colleague")
        || contains_latin_token(&normalized, "colleagues")
        || contains_latin_token(&normalized, "others")
}

fn mentions_non_okr_goal_context(text: &str) -> bool {
    text.contains("目标客户") || text.contains("客户目标")
}

fn mentions_non_okr_feishu_domain(text: &str) -> bool {
    mentions_non_okr_goal_context(text)
        || text.contains("消息")
        || text.contains("聊天")
        || text.contains("会话")
        || text.contains("任务")
        || text.contains("日程")
        || text.contains("会议")
        || text.contains("文档")
        || text.contains("审批")
        || text.contains("邮件")
}

fn contains_latin_token(text: &str, token: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|part| part == token)
}

#[cfg(test)]
mod tests {
    use super::super::catalog::AgentSkillSpec;
    use super::*;
    use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};

    #[test]
    fn selects_feishu_okr_for_explicit_user_okr_read() {
        let request = request_with_latest_user_text("查下我的飞书 OKR 有没有内容");

        assert!(latest_user_requests_feishu_okr_summary(&request));
        assert_eq!(select_skills(&request), vec![AgentSkill::FeishuOkr]);
        assert_feishu_okr_spec(AgentSkill::FeishuOkr.spec());
    }

    #[test]
    fn selects_feishu_okr_for_compact_self_okr_read() {
        assert_eq!(
            select_skills(&request_with_latest_user_text("查我 OKR 当前有几条")),
            vec![AgentSkill::FeishuOkr]
        );
    }

    #[test]
    fn selects_feishu_okr_for_independent_kr_token() {
        assert_eq!(
            select_skills(&request_with_latest_user_text("show my KR count")),
            vec![AgentSkill::FeishuOkr]
        );
    }

    #[test]
    fn selects_feishu_okr_for_contextual_feishu_count_after_okr_topic() {
        let mut request = request_with_latest_user_text("你看下我飞书目前有几条?");
        request.messages.insert(
            0,
            AgentMessageDTO {
                role: "user".to_string(),
                text: "能看到我的 OKR 有几条记录吗".to_string(),
            },
        );

        assert!(latest_user_requests_feishu_okr_summary(&request));
        assert_eq!(select_skills(&request), vec![AgentSkill::FeishuOkr]);
    }

    #[test]
    fn does_not_select_feishu_okr_for_ambiguous_feishu_count_without_okr_context() {
        assert!(
            select_skills(&request_with_latest_user_text("你看下我飞书目前有几条?")).is_empty()
        );
    }

    #[test]
    fn does_not_select_feishu_okr_for_non_self_or_non_read_questions() {
        assert!(select_skills(&request_with_latest_user_text("解释这个风险")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("查团队 OKR")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("帮我查团队 OKR")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("我们团队 OKR 有几条")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("看下张三 OKR")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("show my team OKR")).is_empty());
    }

    #[test]
    fn does_not_select_feishu_okr_for_generic_goal_queries() {
        assert!(select_skills(&request_with_latest_user_text("查我的目标客户数量")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("查我的飞书目标客户数量")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("我的项目目标有几条")).is_empty());
    }

    #[test]
    fn does_not_select_feishu_okr_for_kr_substrings_inside_other_words() {
        assert!(select_skills(&request_with_latest_user_text("show my kraken balance")).is_empty());
        assert!(select_skills(&request_with_latest_user_text("show my okra recipe")).is_empty());
    }

    #[test]
    fn does_not_use_kr_substring_as_recent_okr_context() {
        let mut request = request_with_latest_user_text("你看下我飞书目前有几条?");
        request.messages.insert(
            0,
            AgentMessageDTO {
                role: "user".to_string(),
                text: "show my kraken balance".to_string(),
            },
        );

        assert!(select_skills(&request).is_empty());
    }

    #[test]
    fn does_not_select_contextual_okr_for_other_feishu_domains() {
        let mut request = request_with_latest_user_text("你看下我飞书消息目前有几条?");
        request.messages.insert(
            0,
            AgentMessageDTO {
                role: "user".to_string(),
                text: "能看到我的 OKR 有几条记录吗".to_string(),
            },
        );

        assert!(select_skills(&request).is_empty());
    }

    fn assert_feishu_okr_spec(spec: AgentSkillSpec) {
        assert_eq!(spec.id, "feishu.okr");
        assert_eq!(spec.display_name, "Feishu OKR");
        assert_eq!(spec.tools.len(), 1);
        assert_eq!(spec.tools[0].name, "feishu.okr.summarize_my_okr");
        assert!(spec.safety.contains("后端 tool runtime"));
        assert!(spec.manifest_markdown.contains("## Activation"));
        assert!(spec
            .manifest_markdown
            .contains("feishu.okr.summarize_my_okr"));
    }

    fn request_with_latest_user_text(text: &str) -> AgentStreamRequest {
        AgentStreamRequest {
            messages: vec![AgentMessageDTO {
                role: "user".to_string(),
                text: text.to_string(),
            }],
            context: AgentConversationContextDTO {
                title: "未选择风险".to_string(),
                risk_reason: "暂无风险说明。".to_string(),
                action_summary: "暂无建议动作。".to_string(),
                evidence_summaries: vec![],
                evidence_refs: vec![],
                workspace_summary: "暂无工作区摘要。".to_string(),
                workspace_signals: vec![],
                pending_action_summaries: vec![],
                live_feishu_read_summaries: vec![],
                activated_skill_summaries: vec![],
            },
        }
    }
}
