use reqwest::Url;

use super::AgentModelSettingsError;
use crate::agent::is_allowed_agent_base_url;

pub(super) fn parse_base_url(value: &str) -> Result<Url, AgentModelSettingsError> {
    let base_url = required_trimmed(value.to_string(), AgentModelSettingsError::MissingBaseURL)?;
    let mut base_url = Url::parse(&base_url)
        .ok()
        .filter(is_allowed_agent_base_url)
        .ok_or(AgentModelSettingsError::InvalidBaseURL)?;
    base_url.set_query(None);
    base_url.set_fragment(None);
    Ok(base_url)
}

pub(super) fn required_trimmed(
    value: String,
    missing: AgentModelSettingsError,
) -> Result<String, AgentModelSettingsError> {
    let value = value.trim();
    if value.is_empty() {
        Err(missing)
    } else {
        Ok(value.to_string())
    }
}

pub(super) fn optional_trimmed_api_key(
    value: Option<String>,
) -> Result<Option<String>, AgentModelSettingsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let mut parts = value.splitn(2, char::is_whitespace);
    if parts
        .next()
        .map(|scheme| scheme.eq_ignore_ascii_case("bearer"))
        .unwrap_or(false)
    {
        let value = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(AgentModelSettingsError::MissingApiKey)?;
        return Ok(Some(value.to_string()));
    }

    Ok(Some(value.to_string()))
}

pub(super) fn agent_base_url_candidates(base_url: &Url) -> Vec<Url> {
    let mut candidates = Vec::new();
    if let Some(trimmed) = trim_known_endpoint_suffix(base_url) {
        push_api_base_url_candidates(&mut candidates, trimmed);
    }

    push_api_base_url_candidates(&mut candidates, base_url.clone());
    candidates
}

pub(super) fn base_urls_share_detection_candidate(left: &Url, right: &Url) -> bool {
    left == right
        || agent_base_url_candidates(left)
            .iter()
            .any(|candidate| candidate == right)
        || agent_base_url_candidates(right)
            .iter()
            .any(|candidate| candidate == left)
}

fn push_api_base_url_candidates(candidates: &mut Vec<Url>, base_url: Url) {
    let is_root = is_root_path(&base_url);
    let prefer_versioned = is_root && prefers_versioned_api_base(&base_url);
    if prefer_versioned {
        push_versioned_api_base_url(candidates, &base_url);
    }

    push_unique_url(candidates, base_url.clone());

    if is_root && !prefer_versioned {
        push_versioned_api_base_url(candidates, &base_url);
    }
}

fn trim_known_endpoint_suffix(base_url: &Url) -> Option<Url> {
    let path = base_url.path().trim_end_matches('/');
    for suffix in ["/chat/completions", "/messages", "/models"] {
        if let Some(prefix) = path.strip_suffix(suffix) {
            let mut trimmed = base_url.clone();
            trimmed.set_path(if prefix.is_empty() { "/" } else { prefix });
            trimmed.set_query(None);
            trimmed.set_fragment(None);
            return Some(trimmed);
        }
    }
    None
}

fn push_versioned_api_base_url(candidates: &mut Vec<Url>, base_url: &Url) {
    let mut versioned = base_url.clone();
    versioned.set_path("/v1");
    versioned.set_query(None);
    versioned.set_fragment(None);
    push_unique_url(candidates, versioned);
}

fn is_root_path(base_url: &Url) -> bool {
    matches!(base_url.path(), "" | "/")
}

fn prefers_versioned_api_base(base_url: &Url) -> bool {
    base_url
        .host_str()
        .map(|host| host.contains("openai") || host.contains("anthropic"))
        .unwrap_or(false)
}

fn push_unique_url(urls: &mut Vec<Url>, url: Url) {
    if !urls.iter().any(|existing| existing == &url) {
        urls.push(url);
    }
}

pub(super) fn agent_endpoint_url(base_url: &Url, suffix: &str) -> Url {
    let mut endpoint = base_url.clone();
    let path = format!("{}/{}", endpoint.path().trim_end_matches('/'), suffix);
    endpoint.set_path(&path);
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    endpoint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_detection_base_url_candidates_normalize_common_inputs() {
        let openai_root = Url::parse("https://api.openai.com").expect("url");
        let openai_candidates = agent_base_url_candidates(&openai_root)
            .into_iter()
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            openai_candidates,
            vec!["https://api.openai.com/v1", "https://api.openai.com/"]
        );

        let chat_completions =
            Url::parse("https://api.openai.com/v1/chat/completions").expect("url");
        let chat_candidates = agent_base_url_candidates(&chat_completions)
            .into_iter()
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            chat_candidates,
            vec![
                "https://api.openai.com/v1",
                "https://api.openai.com/v1/chat/completions"
            ]
        );

        let generic_root = Url::parse("https://llm.example.test").expect("url");
        let generic_candidates = agent_base_url_candidates(&generic_root)
            .into_iter()
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            generic_candidates,
            vec!["https://llm.example.test/", "https://llm.example.test/v1"]
        );
    }

    #[test]
    fn saved_api_key_base_url_comparison_accepts_detection_equivalents() {
        let root = Url::parse("https://llm.example.test").expect("url");
        let versioned = Url::parse("https://llm.example.test/v1").expect("url");
        let other = Url::parse("https://other.example.test/v1").expect("url");

        assert!(base_urls_share_detection_candidate(&root, &versioned));
        assert!(base_urls_share_detection_candidate(&versioned, &root));
        assert!(!base_urls_share_detection_candidate(&versioned, &other));
    }

    #[test]
    fn parsed_base_url_drops_query_and_fragment() {
        let base_url =
            parse_base_url("https://llm.example.test/v1/models?foo=bar#frag").expect("url");

        assert_eq!(base_url.as_str(), "https://llm.example.test/v1/models");
    }

    #[test]
    fn optional_api_key_accepts_plain_or_bearer_pasted_values() {
        assert_eq!(
            optional_trimmed_api_key(Some("  sk-test  ".to_string())).expect("api key"),
            Some("sk-test".to_string())
        );
        assert_eq!(
            optional_trimmed_api_key(Some("Bearer sk-test".to_string())).expect("bearer api key"),
            Some("sk-test".to_string())
        );
        assert_eq!(
            optional_trimmed_api_key(Some("bearer   sk-test  ".to_string()))
                .expect("lowercase bearer api key"),
            Some("sk-test".to_string())
        );
        assert_eq!(
            optional_trimmed_api_key(Some("   ".to_string())).expect("empty api key"),
            None
        );
        assert_eq!(
            optional_trimmed_api_key(Some("Bearer   ".to_string())).expect_err("missing api key"),
            AgentModelSettingsError::MissingApiKey
        );
    }
}
