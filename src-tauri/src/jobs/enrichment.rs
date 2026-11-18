//! Enrichment data structures for research jobs
//!
//! These structures represent the additional metadata that can be extracted
//! during research and used to populate database columns.

use serde::Deserialize;

/// Enrichment data for leads (companies)
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LeadEnrichment {
    pub website: Option<String>,
    pub industry: Option<String>,
    pub sub_industry: Option<String>,
    pub employees: Option<i64>,
    pub employee_range: Option<String>,
    pub revenue: Option<f64>,
    pub revenue_range: Option<String>,
    pub company_linkedin_url: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    /// How the company builds addresses from names, e.g. "first.last" or "flast"
    #[serde(default, deserialize_with = "lenient_string")]
    pub email_pattern: Option<String>,
    /// Real published addresses that show the pattern
    #[serde(default, deserialize_with = "lenient_strings")]
    pub email_examples: Option<Vec<String>>,
}

// These two fields are a bonus. Research writing one in the wrong shape (a single address
// instead of a list, say) must not cost the company its website, industry and location, which
// a strict parse of the whole file would.

fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(text) => Some(text),
        _ => None,
    })
}

fn lenient_strings<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<String>>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(text) => Some(vec![text]),
        serde_json::Value::Array(items) => Some(
            items
                .into_iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect(),
        ),
        _ => None,
    })
}

/// Enrichment data for people
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PersonEnrichment {
    pub email: Option<String>,
    /// "published" or "pattern"
    pub email_source: Option<String>,
    pub title: Option<String>,
    pub management_level: Option<String>,
    pub linkedin_url: Option<String>,
    pub year_joined: Option<i64>,
}

/// An email address as research reported it, or None when it isn't one ("Not found",
/// "N/A", a sentence, a mailto: link with extras). Trimmed and lowercased.
pub fn clean_email(value: Option<&str>) -> Option<String> {
    let text = value?
        .trim()
        .trim_start_matches("mailto:")
        .trim()
        .to_lowercase();
    if text.len() > 254
        || text
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '<' | '>' | '"' | ',' | ';' | '?'))
    {
        return None;
    }
    let (local, domain) = text.split_once('@')?;
    let domain_ok = domain.contains('.')
        && !domain.contains('@')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..");
    if local.is_empty() || !domain_ok {
        return None;
    }
    Some(text)
}

/// "published" or "pattern", or None for anything else research wrote
pub fn clean_email_source(value: Option<&str>) -> Option<&'static str> {
    match value?.trim().to_lowercase().as_str() {
        "published" => Some("published"),
        "pattern" => Some("pattern"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_email, clean_email_source};

    #[test]
    fn a_misshapen_pattern_field_does_not_lose_the_rest() {
        let parsed: super::LeadEnrichment = serde_json::from_str(
            r#"{"website": "https://acme.org", "emailPattern": ["first.last"],
                "emailExamples": "jane.doe@acme.org"}"#,
        )
        .unwrap();
        assert_eq!(parsed.website.as_deref(), Some("https://acme.org"));
        assert_eq!(parsed.email_pattern, None);
        assert_eq!(
            parsed.email_examples,
            Some(vec!["jane.doe@acme.org".to_string()])
        );

        let mixed: super::LeadEnrichment =
            serde_json::from_str(r#"{"emailExamples": ["a@acme.org", 7, null]}"#).unwrap();
        assert_eq!(mixed.email_examples, Some(vec!["a@acme.org".to_string()]));
        let none: super::LeadEnrichment = serde_json::from_str("{}").unwrap();
        assert_eq!((none.email_pattern, none.email_examples), (None, None));
    }

    #[test]
    fn email_source_is_one_of_two_words() {
        assert_eq!(clean_email_source(Some(" Published ")), Some("published"));
        assert_eq!(clean_email_source(Some("pattern")), Some("pattern"));
        assert_eq!(clean_email_source(Some("guessed")), None);
        assert_eq!(clean_email_source(None), None);
    }

    #[test]
    fn keeps_real_addresses_and_drops_the_rest() {
        assert_eq!(
            clean_email(Some(" Jane.Doe@Example.com ")),
            Some("jane.doe@example.com".into())
        );
        assert_eq!(
            clean_email(Some("mailto:jane@example.co.uk")),
            Some("jane@example.co.uk".into())
        );
        assert_eq!(clean_email(Some("Not found")), None);
        assert_eq!(clean_email(Some("N/A")), None);
        assert_eq!(clean_email(Some("")), None);
        assert_eq!(clean_email(Some("jane@localhost")), None);
        assert_eq!(clean_email(Some("jane@example.com (guessed)")), None);
        assert_eq!(clean_email(Some("a@b@example.com")), None);
        assert_eq!(clean_email(None), None);
    }
}
