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
}

/// Enrichment data for people
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PersonEnrichment {
    pub email: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::clean_email;

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
