use serde_json::Value;

pub const REDACTED: &str = "[redacted]";

pub fn redact_metadata(value: Value) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| {
                    if is_sensitive_key(&key) {
                        (key, Value::String(REDACTED.to_owned()))
                    } else {
                        (key, redact_metadata(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_metadata).collect()),
        value => value,
    }
}

pub fn redact_text(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            let Some((key, _)) = part.split_once('=') else {
                return part.to_owned();
            };

            if is_sensitive_key(key) {
                format!("{key}={REDACTED}")
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        "apikey"
            | "authorization"
            | "authtoken"
            | "bearertoken"
            | "accesstoken"
            | "refreshtoken"
            | "password"
            | "privatekey"
            | "licensekey"
            | "licensesecret"
            | "prompt"
            | "fullprompt"
            | "prompttext"
            | "promptbody"
            | "systemprompt"
            | "userprompt"
            | "sourcebody"
            | "fullsourcebody"
            | "bodytext"
            | "fullbodytext"
            | "transcripttext"
            | "fulltranscripttext"
            | "rawresponse"
            | "providerrawresponse"
    ) || normalized.contains("secret")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_sensitive_keys_recursively() {
        let redacted = redact_metadata(json!({
            "apiKey": "secret-key",
            "prompt": "full prompt",
            "safe": "kept",
            "nested": {
                "rawResponse": "raw provider response",
                "count": 2
            },
            "items": [
                { "licenseSecret": "private" },
                { "adapterId": "bankier-company-komunikaty" }
            ]
        }));

        assert_eq!(redacted["apiKey"], REDACTED);
        assert_eq!(redacted["prompt"], REDACTED);
        assert_eq!(redacted["safe"], "kept");
        assert_eq!(redacted["nested"]["rawResponse"], REDACTED);
        assert_eq!(redacted["nested"]["count"], 2);
        assert_eq!(redacted["items"][0]["licenseSecret"], REDACTED);
        assert_eq!(
            redacted["items"][1]["adapterId"],
            "bankier-company-komunikaty"
        );
    }

    #[test]
    fn detects_sensitive_keys_after_normalization() {
        assert!(is_sensitive_key("api_key"));
        assert!(is_sensitive_key("Bearer-Token"));
        assert!(is_sensitive_key("providerRawResponse"));
        assert!(is_sensitive_key("clientSecretValue"));
        assert!(!is_sensitive_key("adapterId"));
    }

    #[test]
    fn redacts_sensitive_key_value_text_parts() {
        let redacted = redact_text(
            "provider=gemini apiKey=secret prompt=full sourceBody=body adapterId=bankier",
        );

        assert_eq!(
            redacted,
            "provider=gemini apiKey=[redacted] prompt=[redacted] sourceBody=[redacted] adapterId=bankier"
        );
    }
}
