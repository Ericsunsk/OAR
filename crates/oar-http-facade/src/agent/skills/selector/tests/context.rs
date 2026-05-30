use super::{
    select_feishu_okr_read_intents, select_skills, support::request_with_latest_user_text,
    AgentSkill, FeishuOkrReadIntent,
};
use crate::agent::request::AgentMessageDTO;

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
