use super::{
    calendar, select_feishu_okr_read_intents, select_skills,
    support::request_with_latest_user_text, task, AgentSkill, FeishuOkrReadIntent,
};

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
fn selects_feishu_read_tools_when_user_explicitly_retries_tool_ids() {
    let okr_summary = request_with_latest_user_text("请重试 `feishu.okr.summarize_my_okr`");
    assert_eq!(
        select_feishu_okr_read_intents(&okr_summary),
        vec![FeishuOkrReadIntent::Summary]
    );
    assert_eq!(select_skills(&okr_summary), vec![AgentSkill::Okr]);

    let okr_progress = request_with_latest_user_text("retry feishu.okr.summarize_my_progress");
    assert_eq!(
        select_feishu_okr_read_intents(&okr_progress),
        vec![FeishuOkrReadIntent::Progress]
    );
    assert_eq!(select_skills(&okr_progress), vec![AgentSkill::Okr]);

    let task = request_with_latest_user_text("重新读取 feishu.task.summarize_my_tasks");
    assert!(super::task::latest_user_requests_feishu_task_summary(&task));
    assert_eq!(select_skills(&task), vec![AgentSkill::Task]);

    let calendar = request_with_latest_user_text("run feishu.calendar.summarize_my_free_busy");
    assert!(super::calendar::latest_user_requests_feishu_calendar_free_busy(&calendar));
    assert_eq!(select_skills(&calendar), vec![AgentSkill::Calendar]);
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
