use super::{
    select_feishu_minutes_summary_requested, select_feishu_okr_read_intents, select_skills,
    support::request_with_latest_user_text,
};

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
fn does_not_select_feishu_calendar_for_writes_or_non_self_requests() {
    assert!(select_skills(&request_with_latest_user_text("创建一个日程")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("帮我预约会议")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("查团队忙闲")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("张三今天有空吗")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("查团队日程列表")).is_empty());
    assert!(select_skills(&request_with_latest_user_text("show my team schedule")).is_empty());
}

#[test]
fn does_not_select_feishu_minutes_for_writes_exports_or_non_self_requests() {
    for text in [
        "查团队妙记",
        "看张三会议纪要",
        "下载我的妙记",
        "导出我的妙记逐字稿",
        "删除我的会议记录",
        "export my meeting notes transcript",
        "share my Feishu minutes",
    ] {
        let request = request_with_latest_user_text(text);

        assert!(!select_feishu_minutes_summary_requested(&request), "{text}");
        assert!(select_skills(&request).is_empty(), "{text}");
    }
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
fn does_not_select_read_tool_id_mentions_without_run_or_retry_intent() {
    assert!(select_skills(&request_with_latest_user_text(
        "feishu.okr.summarize_my_okr 是什么"
    ))
    .is_empty());
    assert!(select_skills(&request_with_latest_user_text(
        "解释 feishu.task.summarize_my_tasks 的作用"
    ))
    .is_empty());
    assert!(select_skills(&request_with_latest_user_text(
        "feishu.minutes.summarize_my_minutes 是什么"
    ))
    .is_empty());
}
