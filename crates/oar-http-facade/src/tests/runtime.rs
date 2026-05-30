use crate::OarHttpFacadeRuntime;

#[test]
fn runtime_disables_auth_when_env_absent_and_rejects_partial_auth_config() {
    let disabled = OarHttpFacadeRuntime::from_env_map(&|_| None).expect("disabled runtime");
    assert!(disabled.feishu_login.is_none());
    assert!(disabled.agent.is_none());

    let partial = OarHttpFacadeRuntime::from_env_map(&|key| {
        (key == "OAR_FEISHU_APP_ID").then(|| "cli_test".to_string())
    })
    .expect_err("partial auth config");

    assert_eq!(
        partial.to_string(),
        "oar_feishu_auth_config_partial".to_string()
    );
    assert!(!format!("{partial:?}").contains("cli_test"));
}

#[test]
fn runtime_accepts_agent_config_without_leaking_secret() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_AGENT_OPENAI_BASE_URL" => Some("https://llm.example.test/v1".to_string()),
        "OAR_AGENT_OPENAI_API_KEY" => Some("sk-sensitive".to_string()),
        "OAR_AGENT_OPENAI_MODEL" => Some("agent-model".to_string()),
        _ => None,
    })
    .expect("runtime");

    assert!(runtime.feishu_login.is_none());
    assert!(runtime.agent.is_some());
    assert!(!format!("{runtime:?}").contains("sk-sensitive"));
}

#[test]
fn runtime_accepts_anthropic_agent_config_without_leaking_secret() {
    let runtime = OarHttpFacadeRuntime::from_env_map(&|key| match key {
        "OAR_AGENT_PROVIDER" => Some("anthropic".to_string()),
        "OAR_AGENT_ANTHROPIC_API_KEY" => Some("sk-ant-sensitive".to_string()),
        "OAR_AGENT_ANTHROPIC_MODEL" => Some("claude-sonnet-test".to_string()),
        _ => None,
    })
    .expect("runtime");

    assert!(runtime.feishu_login.is_none());
    assert!(runtime.agent.is_some());
    assert!(!format!("{runtime:?}").contains("sk-ant-sensitive"));
}

#[test]
fn runtime_rejects_partial_agent_config_without_leaking_secret() {
    let error = OarHttpFacadeRuntime::from_env_map(&|key| {
        (key == "OAR_AGENT_OPENAI_API_KEY").then(|| "sk-sensitive".to_string())
    })
    .expect_err("partial agent config");

    assert_eq!(error.to_string(), "oar_agent_config_partial");
    assert!(!format!("{error:?}").contains("sk-sensitive"));
}
