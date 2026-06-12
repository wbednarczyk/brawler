use serde::Deserialize;
use std::error::Error as StdError;

pub(crate) fn extract_json_object<'a>(
    text: &'a str,
    response_label: &str,
) -> Result<&'a str, String> {
    let trimmed = text.trim();
    if let Some(stripped) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
    {
        return extract_json_object(stripped, response_label);
    }

    let Some(start) = trimmed.find('{') else {
        return Err(format!("{response_label} did not include a JSON object"));
    };

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, character) in trimmed[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match character {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + character.len_utf8();
                    return Ok(&trimmed[start..end]);
                }
            }
            _ => {}
        }
    }

    Err(format!(
        "{response_label} did not include a complete JSON object"
    ))
}

pub(crate) fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(crate) fn effective_timeout_seconds(env_key: &str, configured_timeout_seconds: u64) -> u64 {
    std::env::var(env_key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| configured_timeout_seconds.max(1))
}

pub(crate) fn describe_reqwest_error(error: &reqwest::Error) -> String {
    let mut details = vec![error.to_string()];
    let mut kinds = Vec::new();

    if error.is_timeout() {
        kinds.push("timeout");
    }
    if error.is_connect() {
        kinds.push("connect");
    }
    if error.is_request() {
        kinds.push("request");
    }
    if error.is_body() {
        kinds.push("body");
    }
    if error.is_decode() {
        kinds.push("decode");
    }
    if error.is_status() {
        kinds.push("status");
    }
    if !kinds.is_empty() {
        details.push(format!("kind={}", kinds.join(",")));
    }
    if let Some(url) = error.url() {
        details.push(format!("url={url}"));
    }

    let mut source = error.source();
    while let Some(cause) = source {
        details.push(format!("cause={cause}"));
        source = cause.source();
    }

    details.join("; ")
}

pub(crate) fn summarize_provider_error_body(body: &str) -> String {
    #[derive(Deserialize)]
    struct ErrorEnvelope {
        error: Option<ErrorBody>,
    }

    #[derive(Deserialize)]
    struct ErrorBody {
        message: Option<String>,
        status: Option<String>,
    }

    serde_json::from_str::<ErrorEnvelope>(body)
        .ok()
        .and_then(|envelope| envelope.error)
        .map(|error| match (error.status, error.message) {
            (Some(status), Some(message)) => format!("{status}: {message}"),
            (Some(status), None) => status,
            (None, Some(message)) => message,
            (None, None) => "provider returned an error without a message".to_owned(),
        })
        .unwrap_or_else(|| {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                "provider returned an empty error body".to_owned()
            } else {
                trimmed.chars().take(500).collect()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_balanced_json_object_from_fenced_response_with_trailing_text() {
        let text = "```json\n{\"summary\":\"ok\",\"nested\":{\"value\":\"}\"}}\n```\nignored";

        let extracted =
            extract_json_object(text, "provider response").expect("balanced object should extract");

        assert_eq!(
            extracted,
            "{\"summary\":\"ok\",\"nested\":{\"value\":\"}\"}}"
        );
    }

    #[test]
    fn summarizes_structured_provider_error_body() {
        let summary = summarize_provider_error_body(
            r#"{"error":{"status":"RESOURCE_EXHAUSTED","message":"quota exceeded"}}"#,
        );

        assert_eq!(summary, "RESOURCE_EXHAUSTED: quota exceeded");
    }
}
