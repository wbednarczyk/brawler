use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesCData, BytesRef, BytesText};
use time::{format_description::well_known::Rfc2822, format_description::well_known::Rfc3339};
use url::Url;

pub(crate) fn decode_xml_text_value(text: BytesText<'_>) -> Result<String, String> {
    let decoded = text.decode().map_err(|error| error.to_string())?;
    unescape_xml_text_value(&decoded)
}

pub(crate) fn decode_xml_cdata_value(text: BytesCData<'_>) -> Result<String, String> {
    let decoded = text.decode().map_err(|error| error.to_string())?;
    unescape_xml_text_value(&decoded)
}

pub(crate) fn decode_xml_reference_value(reference: BytesRef<'_>) -> Result<String, String> {
    let decoded = reference.decode().map_err(|error| error.to_string())?;

    Ok(resolve_xml_entity(&decoded)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("&{decoded};")))
}

pub(crate) fn unescape_xml_text_value(value: &str) -> Result<String, String> {
    unescape(value)
        .map(|value| value.into_owned())
        .map_err(|error| error.to_string())
}

pub(crate) fn normalize_link_without_tracking(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_owned();
    };

    url.set_fragment(None);

    let retained_query_pairs = url
        .query_pairs()
        .filter(|(key, _)| !key.to_ascii_lowercase().starts_with("utm_"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    url.set_query(None);

    if !retained_query_pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(
            retained_query_pairs
                .iter()
                .map(|(key, value)| (&**key, &**value)),
        );
    }

    url.to_string()
}

pub(crate) fn parse_rfc2822_timestamp(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    time::OffsetDateTime::parse(value, &Rfc2822)
        .ok()
        .and_then(|timestamp| timestamp.format(&Rfc3339).ok())
}

pub(crate) fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a str>) -> &'a str {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or("missing")
}

pub(crate) fn strip_html(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;

    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }

    let stripped = output.split_whitespace().collect::<Vec<_>>().join(" ");
    unescape(&stripped)
        .map(|value| value.into_owned())
        .unwrap_or(stripped)
}

pub(crate) fn slug_part(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn normalize_polish(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| match character {
            'ą' | 'Ą' => 'a',
            'ć' | 'Ć' => 'c',
            'ę' | 'Ę' => 'e',
            'ł' | 'Ł' => 'l',
            'ń' | 'Ń' => 'n',
            'ó' | 'Ó' => 'o',
            'ś' | 'Ś' => 's',
            'ż' | 'Ż' | 'ź' | 'Ź' => 'z',
            other => other.to_lowercase().next().unwrap_or(other),
        })
        .collect::<String>()
}

pub(crate) fn polish_slug_part(value: &str) -> String {
    normalize_polish(value)
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_tracking_from_links_without_dropping_useful_query() {
        let normalized =
            normalize_link_without_tracking("https://example.test/a?utm_source=x&id=7#fragment");

        assert_eq!(normalized, "https://example.test/a?id=7");
    }

    #[test]
    fn builds_ascii_slugs() {
        assert_eq!(
            slug_part("CD Projekt: raport 1/2026"),
            "cd-projekt-raport-1-2026"
        );
        assert_eq!(
            polish_slug_part("Walne zgromadzenie spółki"),
            "walne-zgromadzenie-spolki"
        );
    }
}
