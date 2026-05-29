use super::*;
use crate::agent::request::{AgentConversationContextDTO, AgentMessageDTO};

#[test]
fn selects_feishu_okr_for_explicit_user_okr_read() {
    let request = request_with_latest_user_text("查下我的飞书 OKR 有没有内容");

    assert_eq!(
        select_feishu_okr_read_intents(&request),
        vec![FeishuOkrReadIntent::Summary]
    );
    assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
}

#[test]
fn selects_feishu_task_for_explicit_user_task_read() {
    let request = request_with_latest_user_text("查下我的飞书任务有几条");

    assert!(task::latest_user_requests_feishu_task_summary(&request));
    assert_eq!(select_skills(&request), vec![AgentSkill::Task]);
}

#[test]
fn selects_feishu_calendar_for_explicit_user_free_busy_read() {
    let request = request_with_latest_user_text("查下我的飞书日历今天有没有空");

    assert!(calendar::latest_user_requests_feishu_calendar_free_busy(
        &request
    ));
    assert_eq!(select_skills(&request), vec![AgentSkill::Calendar]);
}

#[test]
fn selects_feishu_calendar_for_availability_variants() {
    assert_eq!(
        select_skills(&request_with_latest_user_text("看下我的日历忙闲")),
        vec![AgentSkill::Calendar]
    );
    assert_eq!(
        select_skills(&request_with_latest_user_text(
            "show my Feishu availability"
        )),
        vec![AgentSkill::Calendar]
    );
    assert_eq!(
        select_skills(&request_with_latest_user_text("查我的飞书今天能不能开会")),
        vec![AgentSkill::Calendar]
    );
}

#[test]
fn selects_feishu_task_for_todo_read_variants() {
    assert_eq!(
        select_skills(&request_with_latest_user_text("看下我的待办")),
        vec![AgentSkill::Task]
    );
    assert_eq!(
        select_skills(&request_with_latest_user_text("show my tasks")),
        vec![AgentSkill::Task]
    );
}

#[test]
fn selects_feishu_okr_for_compact_self_okr_read() {
    assert_eq!(
        select_skills(&request_with_latest_user_text("查我 OKR 当前有几条")),
        vec![AgentSkill::Okr]
    );
}

#[test]
fn selects_feishu_okr_for_independent_kr_token() {
    assert_eq!(
        select_skills(&request_with_latest_user_text("show my KR count")),
        vec![AgentSkill::Okr]
    );
}

#[test]
fn selects_feishu_okr_progress_intent_for_self_progress_variants() {
    for text in [
        "我的 OKR 进展",
        "我的 OKR 进度",
        "我的 OKR 更新记录",
        "我的 OKR 最近更新",
        "我的 OKR 上次更新",
        "我的 OKR 风险",
        "我的 OKR 延期",
        "show my OKR progress",
        "show my OKR update records",
        "show my OKR latest updates",
        "show my OKR risk",
        "my OKR stale",
    ] {
        let request = request_with_latest_user_text(text);
        assert_eq!(
            select_feishu_okr_read_intents(&request),
            vec![FeishuOkrReadIntent::Progress],
            "{text}"
        );
        assert_eq!(select_skills(&request), vec![AgentSkill::Okr], "{text}");
    }
}

#[test]
fn selects_both_okr_intents_when_latest_request_asks_count_and_progress() {
    let request = request_with_latest_user_text("查我的 OKR 有几条，以及最近进展");

    assert_eq!(
        select_feishu_okr_read_intents(&request),
        vec![FeishuOkrReadIntent::Summary, FeishuOkrReadIntent::Progress]
    );
    assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
}

#[test]
fn selects_only_progress_for_target_progress_phrasing() {
    for text in ["看我的 OKR 目标进展", "show my OKR objective progress"] {
        let request = request_with_latest_user_text(text);

        assert_eq!(
            select_feishu_okr_read_intents(&request),
            vec![FeishuOkrReadIntent::Progress],
            "{text}"
        );
        assert_eq!(select_skills(&request), vec![AgentSkill::Okr], "{text}");
    }
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

    assert_eq!(
        select_feishu_okr_read_intents(&request),
        vec![FeishuOkrReadIntent::Summary]
    );
    assert_eq!(select_skills(&request), vec![AgentSkill::Okr]);
}

#[test]
fn does_not_select_feishu_okr_for_ambiguous_feishu_count_without_okr_context() {
    assert!(select_skills(&request_with_latest_user_text("你看下我飞书目前有几条?")).is_empty());
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
fn does_not_select_feishu_okr_progress_for_writes_non_self_or_generic_risk() {
    for text in [
        "帮我更新我的 OKR 进度",
        "新增我的 OKR 进展",
        "创建我的 OKR 进展",
        "删除我的 OKR 进展",
        "提交我的 OKR 进展",
        "发布我的 OKR 进展",
        "评论我的 OKR 进展",
        "提醒我的 OKR 进展",
        "update my OKR progress",
        "set my OKR progress",
        "write my OKR progress",
        "delete my OKR progress",
        "submit my OKR progress",
        "post my OKR progress",
        "comment my OKR progress",
        "remind my OKR progress",
        "查团队 OKR 进展",
        "看其他人 OKR 风险",
        "show my team OKR progress",
        "查我的飞书目标客户风险",
        "解释这个风险",
    ] {
        let request = request_with_latest_user_text(text);
        assert!(
            select_feishu_okr_read_intents(&request).is_empty(),
            "{text}"
        );
        assert!(select_skills(&request).is_empty(), "{text}");
    }
}

#[test]
fn does_not_select_feishu_task_for_writes_or_non_self_requests() {
    assert!(select_skills(&request_with_latest_user_text("创建一个任务")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("帮我更新我的任务")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("查团队任务")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("看下张三任务")).is_empty());
}

#[test]
fn does_not_select_feishu_calendar_for_writes_event_lists_or_non_self_requests() {
    assert!(select_skills(&request_with_latest_user_text("创建一个日程")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("帮我预约会议")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("查团队忙闲")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("张三今天有空吗")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("看我的日程列表")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("show my schedule")).is_empty());
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
