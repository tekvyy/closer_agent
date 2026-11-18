//! Turns the people a company research session wrote into rows we can trust.
//!
//! Two things are decided here rather than left to the AI, because it got both wrong in
//! testing: a shared mailbox (membership@, info@) handed to a named person, and a company
//! email pattern that was plainly there but not applied to the people still missing an
//! address.

use super::enrichment::{clean_email, clean_email_source, LeadEnrichment};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// One person from company research, ready to store
#[derive(Debug, Clone, PartialEq)]
pub struct FoundPerson {
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub linkedin_url: Option<String>,
    pub management_level: Option<String>,
    pub year_joined: Option<i64>,
    pub email: Option<String>,
    /// "published" or "pattern"; only set next to an email
    pub email_source: Option<&'static str>,
    pub email_evidence: Option<String>,
}

/// Ways a company builds addresses from a name, as the research prompt names them
const PATTERNS: [&str; 8] = [
    "first.last",
    "first_last",
    "firstlast",
    "flast",
    "f.last",
    "first",
    "firstl",
    "last",
];

/// The front-door mailbox. In a business of one or two people it is the owner's own inbox;
/// anywhere larger, a personal email sent there reaches the wrong reader.
const GENERAL_MAILBOXES: [&str; 14] = [
    "admin",
    "contact",
    "contactus",
    "email",
    "enquiries",
    "general",
    "hello",
    "hi",
    "info",
    "information",
    "inquiries",
    "mail",
    "office",
    "welcome",
];

/// A department's mailbox, which is never one person's address whatever the size
const TEAM_MAILBOXES: [&str; 26] = [
    "admissions",
    "billing",
    "careers",
    "communications",
    "development",
    "donate",
    "donations",
    "events",
    "finance",
    "giving",
    "help",
    "hr",
    "jobs",
    "marketing",
    "media",
    "membership",
    "news",
    "partnerships",
    "press",
    "programs",
    "sales",
    "staff",
    "support",
    "team",
    "training",
    "volunteer",
];

/// Personal mail providers. An address there says nothing about how a company writes its own.
const FREE_MAIL: [&str; 16] = [
    "aol.com",
    "att.net",
    "bellsouth.net",
    "comcast.net",
    "gmail.com",
    "googlemail.com",
    "hotmail.com",
    "icloud.com",
    "live.com",
    "me.com",
    "msn.com",
    "outlook.com",
    "proton.me",
    "protonmail.com",
    "verizon.net",
    "yahoo.com",
];

/// `site_domain` is the company's website domain ("acme.org"), when known. A pattern is only
/// learned from, and only builds, addresses that belong to the company.
pub fn finalize_people(
    people: &[Value],
    enrichment: Option<&LeadEnrichment>,
    site_domain: Option<&str>,
) -> Vec<FoundPerson> {
    let mut found: Vec<FoundPerson> = people.iter().map(read_person).collect();
    drop_shared_mailboxes(&mut found);
    if let Some((pattern, domain, examples)) = company_pattern(&found, enrichment, site_domain) {
        apply_pattern(&mut found, pattern, &domain, &examples);
    }
    found
}

/// "covenanthousega.org" and "covenanthouse.org" are the same organisation; "bigbank.com" on
/// a nonprofit's board page is somebody's day job
fn same_organisation(domain: &str, site: &str) -> bool {
    if domain == site
        || domain.ends_with(&format!(".{site}"))
        || site.ends_with(&format!(".{domain}"))
    {
        return true;
    }
    let name = |d: &str| d.split('.').next().unwrap_or_default().to_string();
    let (a, b) = (name(domain), name(site));
    a.len().min(b.len()) >= 5 && (a.starts_with(&b) || b.starts_with(&a))
}

/// Whether `examples` real addresses at `domain` are enough to build more with `pattern`
fn enough_evidence(pattern: &str, domain: &str, examples: usize, site: Option<&str>) -> bool {
    if FREE_MAIL.contains(&domain) {
        return false;
    }
    // A lone greg@ or smith@ is often the owner's alias rather than the rule
    let needed = if matches!(pattern, "first" | "last") {
        2
    } else {
        1
    };
    if examples < needed {
        return false;
    }
    // Off the company's own domain, one address could be anyone's other job
    site.is_none_or(|site| same_organisation(domain, site)) || examples >= 2
}

fn read_person(p: &Value) -> FoundPerson {
    let text = |key: &str| {
        p.get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from)
    };
    let (first_name, last_name) = match (text("firstName"), text("lastName")) {
        (Some(first), last) => (first, last.unwrap_or_default()),
        (None, last) => match text("name") {
            Some(name) => {
                let mut parts = name.split_whitespace();
                let first = parts.next().unwrap_or("Unknown").to_string();
                (first, parts.collect::<Vec<_>>().join(" "))
            }
            None => ("Unknown".to_string(), last.unwrap_or_default()),
        },
    };
    let email = clean_email(text("email").as_deref());
    // A source only means something next to the address it describes. With no source given
    // the address counts as published; anything else research wrote there ("inferred",
    // "guess") is the weaker claim, so it is never shown as verified or used as evidence.
    let email_source = email.as_ref().map(|_| match text("emailSource") {
        None => "published",
        Some(source) => clean_email_source(Some(&source)).unwrap_or("pattern"),
    });
    let email_evidence = email.as_ref().and_then(|_| text("emailEvidence"));
    FoundPerson {
        first_name,
        last_name,
        title: text("title"),
        linkedin_url: text("linkedinUrl"),
        management_level: text("managementLevel"),
        year_joined: p.get("yearJoined").and_then(|v| v.as_i64()),
        email,
        email_source,
        email_evidence,
    }
}

fn split_email(email: &str) -> Option<(&str, &str)> {
    email.split_once('@')
}

/// Takes away addresses that aren't the person's own: a department mailbox always, and the
/// front-door mailbox or an address given to two people once the list is longer than two
/// (a one or two person business often does run on info@).
fn drop_shared_mailboxes(people: &mut [FoundPerson]) {
    let small = people.len() <= 2;
    let mut uses: HashMap<String, usize> = HashMap::new();
    for email in people.iter().filter_map(|person| person.email.clone()) {
        *uses.entry(email).or_default() += 1;
    }
    for person in people.iter_mut() {
        let Some(email) = person.email.as_deref() else {
            continue;
        };
        let name = split_email(email)
            .map(|(local, _)| local.replace(['-', '_', '.'], ""))
            .unwrap_or_default();
        let name = name.as_str();
        let shared = TEAM_MAILBOXES.contains(&name)
            || (!small
                && (GENERAL_MAILBOXES.contains(&name)
                    || uses.get(email).copied().unwrap_or(0) > 1));
        if shared {
            person.email = None;
            person.email_source = None;
            person.email_evidence = None;
        }
    }
}

/// A name part we can safely build an address from: plain letters only, so "Williams-Neal",
/// "de la Cruz" and "O'Brien" are left alone rather than guessed at
fn simple_name(name: &str) -> Option<String> {
    let name = name.trim().to_lowercase();
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase())).then_some(name)
}

fn build_local(pattern: &str, first: &str, last: &str) -> Option<String> {
    let f = first.chars().next()?;
    let l = last.chars().next()?;
    Some(match pattern {
        "first.last" => format!("{first}.{last}"),
        "first_last" => format!("{first}_{last}"),
        "firstlast" => format!("{first}{last}"),
        "flast" => format!("{f}{last}"),
        "f.last" => format!("{f}.{last}"),
        "first" => first.to_string(),
        "firstl" => format!("{first}{l}"),
        "last" => last.to_string(),
        _ => return None,
    })
}

/// The pattern this person's published address follows, if it follows one
fn pattern_of(person: &FoundPerson) -> Option<(&'static str, String)> {
    if person.email_source != Some("published") {
        return None;
    }
    let (local, domain) = split_email(person.email.as_deref()?)?;
    let first = simple_name(&person.first_name)?;
    let last = simple_name(&person.last_name)?;
    PATTERNS
        .iter()
        .find(|pattern| build_local(pattern, &first, &last).as_deref() == Some(local))
        .map(|pattern| (*pattern, domain.to_string()))
}

/// The company's pattern, its domain and the real addresses that show it. The published
/// addresses in the list decide when they all agree; one is enough, as it was for the
/// per-person search this replaces, and in testing a single address predicted the rest
/// correctly. Otherwise we take the pattern research reported, which it may have seen on
/// people outside the list. Addresses built this way are marked "pattern" for the user.
fn company_pattern(
    people: &[FoundPerson],
    enrichment: Option<&LeadEnrichment>,
    site_domain: Option<&str>,
) -> Option<(&'static str, String, Vec<String>)> {
    let shown: Vec<(&'static str, String, String)> = people
        .iter()
        .filter_map(|person| {
            let (pattern, domain) = pattern_of(person)?;
            Some((pattern, domain, person.email.clone()?))
        })
        .filter(|(_, domain, _)| !FREE_MAIL.contains(&domain.as_str()))
        .collect();
    if let Some((pattern, domain, _)) = shown.first() {
        // The list itself shows two patterns or two domains, so no single one can be applied
        let agree = shown.iter().all(|(p, d, _)| p == pattern && d == domain);
        if !agree || !enough_evidence(pattern, domain, shown.len(), site_domain) {
            return None;
        }
        let examples = shown.iter().map(|(_, _, email)| email.clone()).collect();
        return Some((*pattern, domain.clone(), examples));
    }

    let enrichment = enrichment?;
    let reported = enrichment.email_pattern.as_deref()?.trim().to_lowercase();
    let pattern = PATTERNS.iter().find(|pattern| **pattern == reported)?;
    let examples: Vec<String> = enrichment
        .email_examples
        .iter()
        .flatten()
        .filter_map(|example| clean_email(Some(example)))
        .collect();
    let domain = split_email(examples.first()?)?.1.to_string();
    let one_domain = examples
        .iter()
        .all(|example| split_email(example).is_some_and(|(_, d)| d == domain));
    (one_domain && enough_evidence(pattern, &domain, examples.len(), site_domain))
        .then_some((*pattern, domain, examples))
}

fn apply_pattern(people: &mut [FoundPerson], pattern: &str, domain: &str, examples: &[String]) {
    // Two J. Smiths can't both be jsmith@: the second one built is left without an address
    let mut taken: HashSet<String> = people.iter().filter_map(|p| p.email.clone()).collect();
    for person in people.iter_mut().filter(|person| person.email.is_none()) {
        let (Some(first), Some(last)) = (
            simple_name(&person.first_name),
            simple_name(&person.last_name),
        ) else {
            continue;
        };
        let Some(local) = build_local(pattern, &first, &last) else {
            continue;
        };
        let address = format!("{local}@{domain}");
        if !taken.insert(address.clone()) {
            continue;
        }
        person.email = Some(address);
        person.email_source = Some("pattern");
        person.email_evidence = Some(examples.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn emails(people: &[FoundPerson]) -> Vec<Option<&str>> {
        people.iter().map(|p| p.email.as_deref()).collect()
    }

    #[test]
    fn applies_a_pattern_two_published_addresses_agree_on() {
        // What research returned for a real legal-aid nonprofit: three flast@ addresses
        // published, the two most senior people left without one
        let people = [
            json!({"firstName": "Michael", "lastName": "Lucas", "title": "Executive Director"}),
            json!({"firstName": "Crystal", "lastName": "Redd"}),
            json!({"firstName": "Kizzy", "lastName": "Lewis", "email": "klewis@avlf.org", "emailSource": "published", "emailEvidence": "staff page"}),
            json!({"firstName": "Libby", "lastName": "Rother", "email": "lrother@avlf.org", "emailSource": "published"}),
        ];
        let found = finalize_people(&people, None, None);
        assert_eq!(
            emails(&found),
            [
                Some("mlucas@avlf.org"),
                Some("credd@avlf.org"),
                Some("klewis@avlf.org"),
                Some("lrother@avlf.org")
            ]
        );
        assert_eq!(found[0].email_source, Some("pattern"));
        assert_eq!(
            found[0].email_evidence.as_deref(),
            Some("klewis@avlf.org, lrother@avlf.org")
        );
        assert_eq!(found[2].email_source, Some("published"));
        assert_eq!(found[2].email_evidence.as_deref(), Some("staff page"));
    }

    #[test]
    fn one_published_address_is_enough_but_two_that_disagree_prove_nothing() {
        let one = [
            json!({"firstName": "Nicki", "lastName": "Brown", "email": "nbrown@hoa.org", "emailSource": "published"}),
            json!({"firstName": "Naomi", "lastName": "Green"}),
            // Built by research from a pattern, so it is no evidence of one
            json!({"firstName": "Bo", "lastName": "Li", "email": "bo.li@hoa.org", "emailSource": "pattern"}),
        ];
        let found = finalize_people(&one, None, None);
        assert_eq!(found[1].email.as_deref(), Some("ngreen@hoa.org"));
        assert_eq!(found[1].email_source, Some("pattern"));
        assert_eq!(found[2].email.as_deref(), Some("bo.li@hoa.org"));

        let mixed = [
            json!({"firstName": "Greg", "lastName": "Levine", "email": "greg@trees.org", "emailSource": "published"}),
            json!({"firstName": "Suzie", "lastName": "Sloan", "email": "ssloan@trees.org", "emailSource": "published"}),
            json!({"firstName": "Ann", "lastName": "Poe"}),
        ];
        // Even a reported pattern loses to a list that visibly uses two
        let reported = LeadEnrichment {
            email_pattern: Some("first".into()),
            email_examples: Some(vec!["greg@trees.org".into()]),
            ..Default::default()
        };
        assert_eq!(
            emails(&finalize_people(&mixed, Some(&reported), None))[2],
            None
        );
    }

    #[test]
    fn uses_the_pattern_research_reported_with_its_examples() {
        let people = [
            json!({"firstName": "Naomi", "lastName": "Green"}),
            json!({"firstName": "Jim", "lastName": "Williams-Neal"}),
            json!({"firstName": "Ann", "lastName": "Poe"}),
        ];
        let reported = LeadEnrichment {
            email_pattern: Some(" First.Last ".into()),
            email_examples: Some(vec!["Bob.Lee@hoa.org".into(), "not an email".into()]),
            ..Default::default()
        };
        let found = finalize_people(&people, Some(&reported), None);
        assert_eq!(found[0].email.as_deref(), Some("naomi.green@hoa.org"));
        assert_eq!(found[0].email_evidence.as_deref(), Some("bob.lee@hoa.org"));
        // A double-barrelled surname could be written several ways, so it isn't guessed
        assert_eq!(found[1].email, None);

        // No examples, an unknown pattern, or examples from two domains: nothing is applied
        for bad in [
            LeadEnrichment {
                email_pattern: Some("first.last".into()),
                ..Default::default()
            },
            LeadEnrichment {
                email_pattern: Some("initials".into()),
                email_examples: Some(vec!["bl@hoa.org".into()]),
                ..Default::default()
            },
            LeadEnrichment {
                email_pattern: Some("first.last".into()),
                email_examples: Some(vec!["bob.lee@hoa.org".into(), "al.ray@other.org".into()]),
                ..Default::default()
            },
        ] {
            assert_eq!(finalize_people(&people, Some(&bad), None)[0].email, None);
        }
    }

    #[test]
    fn a_team_mailbox_is_nobodys_address_in_a_larger_organisation() {
        let people = [
            json!({"firstName": "Karen", "lastName": "Beavor", "email": "info@gcn.org", "emailSource": "published"}),
            json!({"firstName": "Reggie", "lastName": "Seay", "email": "membership@gcn.org", "emailSource": "published"}),
            json!({"firstName": "Al", "lastName": "Ray", "email": "shared@gcn.org", "emailSource": "published"}),
            json!({"firstName": "Bo", "lastName": "Li", "email": "shared@gcn.org", "emailSource": "published"}),
            json!({"firstName": "Cy", "lastName": "Orr", "email": "cyrus.o@gcn.org", "emailSource": "published"}),
        ];
        let found = finalize_people(&people, None, None);
        assert_eq!(
            emails(&found),
            [None, None, None, None, Some("cyrus.o@gcn.org")]
        );
        assert_eq!(found[0].email_source, None);

        // A department mailbox is nobody's own even when only two people were listed, which
        // is what research returned for a real association
        let two = [
            json!({"firstName": "Karen", "lastName": "Beavor"}),
            json!({"firstName": "Reggie", "lastName": "Seay", "email": "membership@gcn.org", "emailSource": "published"}),
        ];
        assert_eq!(emails(&finalize_people(&two, None, None)), [None, None]);

        // A one or two person business often does run on info@, and info@ shows no pattern
        let small = [
            json!({"firstName": "Dee", "lastName": "Fox", "email": "info@deefox.com", "emailSource": "published"}),
        ];
        assert_eq!(
            emails(&finalize_people(&small, None, None)),
            [Some("info@deefox.com")]
        );
    }

    #[test]
    fn a_pattern_is_only_learned_from_the_companys_own_addresses() {
        let board = |email: &str| {
            [
                json!({"firstName": "John", "lastName": "Smith", "email": email, "emailSource": "published"}),
                json!({"firstName": "Ann", "lastName": "Poe"}),
                json!({"firstName": "Bo", "lastName": "Li"}),
            ]
        };
        let second = |people: &[Value], site| finalize_people(people, None, site)[1].email.clone();
        // A personal mailbox or somebody's day job proves nothing about the nonprofit
        assert_eq!(second(&board("jsmith@gmail.com"), Some("acme.org")), None);
        assert_eq!(second(&board("jsmith@gmail.com"), None), None);
        assert_eq!(second(&board("jsmith@bigbank.com"), Some("acme.org")), None);
        // Its own domain does, and so does the parent's (covenanthousega.org writes from
        // covenanthouse.org) or a subdomain's
        assert_eq!(
            second(&board("jsmith@acme.org"), Some("acme.org")).as_deref(),
            Some("apoe@acme.org")
        );
        assert_eq!(
            second(
                &board("jsmith@covenanthouse.org"),
                Some("covenanthousega.org")
            )
            .as_deref(),
            Some("apoe@covenanthouse.org")
        );
        assert_eq!(
            second(&board("jsmith@mail.acme.org"), Some("acme.org")).as_deref(),
            Some("apoe@mail.acme.org")
        );

        // A lone first-name address is often the owner's alias: it takes two
        assert_eq!(second(&board("john@acme.org"), Some("acme.org")), None);
        let two_first = [
            json!({"firstName": "John", "lastName": "Smith", "email": "john@acme.org", "emailSource": "published"}),
            json!({"firstName": "Ann", "lastName": "Poe"}),
            json!({"firstName": "Bo", "lastName": "Li", "email": "bo@acme.org", "emailSource": "published"}),
        ];
        assert_eq!(
            second(&two_first, Some("acme.org")).as_deref(),
            Some("ann@acme.org")
        );
    }

    #[test]
    fn a_built_address_is_never_given_to_two_people() {
        let people = [
            json!({"firstName": "Jane", "lastName": "Smith", "email": "jsmith@acme.org", "emailSource": "published"}),
            json!({"firstName": "John", "lastName": "Smith"}),
            json!({"firstName": "Amy", "lastName": "Poe"}),
            json!({"firstName": "Al", "lastName": "Poe"}),
        ];
        let found = finalize_people(&people, None, Some("acme.org"));
        assert_eq!(
            emails(&found),
            [Some("jsmith@acme.org"), None, Some("apoe@acme.org"), None]
        );
    }

    #[test]
    fn reads_names_and_tidies_what_research_wrote() {
        let people = [
            json!({"name": "Mary Ann Smith", "email": "Mary@X.org", "emailSource": "guess", "emailEvidence": " page "}),
            json!({"firstName": " Al ", "title": " ", "email": "Not found", "emailSource": "published"}),
        ];
        let found = finalize_people(&people, None, None);
        assert_eq!(found[0].first_name, "Mary");
        assert_eq!(found[0].last_name, "Ann Smith");
        assert_eq!(found[0].email.as_deref(), Some("mary@x.org"));
        // "guess" isn't one of the two sources, so it gets the weaker one
        assert_eq!(found[0].email_source, Some("pattern"));
        assert_eq!(found[0].email_evidence.as_deref(), Some("page"));
        assert_eq!(found[1].first_name, "Al");
        assert_eq!(found[1].last_name, "");
        assert_eq!(found[1].title, None);
        assert_eq!(
            (found[1].email.clone(), found[1].email_source),
            (None, None)
        );
    }
}
