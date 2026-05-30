use oar_lark_adapter::{
    FeishuOAuthLogin, FeishuOAuthLoginToken, FeishuOAuthLoginUser, SecretString,
};

pub(super) fn sample_feishu_login(refresh_token: Option<&str>) -> FeishuOAuthLogin {
    FeishuOAuthLogin {
        token: FeishuOAuthLoginToken {
            access_token: SecretString::new("access-token-sensitive"),
            refresh_token: refresh_token.map(SecretString::new),
            expires_in_seconds: 7_200,
            refresh_token_expires_in_seconds: Some(30 * 86_400),
            token_type: Some("Bearer".to_string()),
            scope: Some("offline_access auth:user.id:read offline_access".to_string()),
        },
        user: FeishuOAuthLoginUser {
            open_id: "ou_123".to_string(),
            union_id: Some("on_123".to_string()),
            tenant_key: Some("tenant_1".to_string()),
            display_name: "Alice".to_string(),
        },
    }
}

pub(super) fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
