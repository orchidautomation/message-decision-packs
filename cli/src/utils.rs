use crate::models::{Manifest, Prospect};
use anyhow::{Result, anyhow};
use serde::Serialize;

pub(crate) fn normalize_supplied_company_domain(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("company_domain is empty"));
    }
    if trimmed.eq_ignore_ascii_case("n/a") {
        return Err(anyhow!("company_domain is N/A"));
    }

    let lower = trimmed.to_ascii_lowercase();
    let without_scheme = if let Some((_, rest)) = lower.split_once("://") {
        rest
    } else {
        lower.as_str()
    };
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.');
    let authority = authority
        .strip_prefix("www.")
        .unwrap_or(authority)
        .trim_end_matches('.');

    let host = if let Some((candidate, port)) = authority.rsplit_once(':') {
        if port.chars().all(|c| c.is_ascii_digit()) {
            candidate
        } else {
            authority
        }
    } else {
        authority
    };

    if !valid_domain_host(host) {
        return Err(anyhow!(
            "company_domain must be a supplied domain or URL host, found {trimmed}"
        ));
    }

    Ok(host.to_string())
}

pub(crate) fn prospect_haystack_with_persona(
    prospect: &Prospect,
    resolved_persona: Option<&str>,
) -> String {
    let mut parts = vec![
        prospect.name.clone(),
        prospect.title.clone(),
        prospect.company.clone(),
    ];
    if let Some(value) = &prospect.company_domain {
        parts.push(value.clone());
    }
    for value in [
        &prospect.linkedin_url,
        &prospect.company_url,
        &prospect.background,
        &prospect.trigger,
        &prospect.persona,
        &prospect.segment,
    ] {
        if let Some(value) = value {
            parts.push(value.clone());
        }
    }
    if let Some(persona) = resolved_persona {
        parts.push(persona.to_string());
    }
    for signal in &prospect.signals {
        parts.push(signal.id.clone());
        parts.push(signal.title.clone());
        for value in [
            &signal.source,
            &signal.confidence,
            &signal.freshness,
            &signal.state_as,
        ] {
            if let Some(value) = value {
                parts.push(value.clone());
            }
        }
    }
    for (key, value) in &prospect.attributes {
        parts.push(key.clone());
        if let Some(value) = value.as_str() {
            parts.push(value.to_string());
        } else if !value.is_object() && !value.is_array() && !value.is_null() {
            parts.push(value.to_string());
        }
    }
    parts.join(" ").to_lowercase()
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PersonaResolution {
    pub(crate) input: String,
    pub(crate) persona: String,
    pub(crate) resolved: bool,
    pub(crate) source: String,
    pub(crate) confidence: String,
    pub(crate) matched_keywords: Vec<String>,
    pub(crate) fit_usable: bool,
    pub(crate) needs_review: bool,
    pub(crate) reason: String,
}

pub(crate) fn resolve_persona(manifest: &Manifest, prospect: &Prospect) -> PersonaResolution {
    if let Some(persona) = prospect
        .persona
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(resolution) = resolve_pack_persona_label(manifest, persona, "prospect.persona")
        {
            return resolution;
        }
        return PersonaResolution {
            input: persona.to_string(),
            persona: persona.to_string(),
            resolved: false,
            source: "prospect.persona".to_string(),
            confidence: "explicit".to_string(),
            matched_keywords: Vec::new(),
            fit_usable: true,
            needs_review: false,
            reason: "prospect persona was provided explicitly".to_string(),
        };
    }

    for mapping in &manifest.persona_mappings {
        let matched_keywords: Vec<String> = mapping
            .title_keywords
            .iter()
            .filter(|keyword| title_keyword_matches(&prospect.title, keyword))
            .cloned()
            .collect();
        if !matched_keywords.is_empty() {
            return PersonaResolution {
                input: prospect.title.clone(),
                persona: mapping.persona.clone(),
                resolved: true,
                source: "manifest.persona_mappings.title_keywords".to_string(),
                confidence: "medium".to_string(),
                matched_keywords,
                fit_usable: true,
                needs_review: false,
                reason: "prospect title matched pack-owned persona mapping".to_string(),
            };
        }
    }

    if let Some((persona, matched_keywords)) = builtin_persona_match(&prospect.title) {
        return PersonaResolution {
            input: prospect.title.clone(),
            persona: persona.to_string(),
            resolved: true,
            source: "builtin.title_keywords".to_string(),
            confidence: "low".to_string(),
            matched_keywords: matched_keywords.iter().map(|value| value.to_string()).collect(),
            fit_usable: false,
            needs_review: true,
            reason: "title matched legacy fallback keywords; add persona_mappings to make this pack-owned".to_string(),
        };
    }

    PersonaResolution {
        input: prospect.title.clone(),
        persona: "Operator".to_string(),
        resolved: false,
        source: "fallback".to_string(),
        confidence: "none".to_string(),
        matched_keywords: Vec::new(),
        fit_usable: false,
        needs_review: true,
        reason: "no explicit persona or pack-owned title mapping matched".to_string(),
    }
}

pub(crate) fn resolve_persona_label(manifest: &Manifest, persona: &str) -> PersonaResolution {
    let input = persona.trim();
    if let Some(resolution) = resolve_pack_persona_label(manifest, input, "input.persona") {
        return resolution;
    }

    if let Some((persona, matched_keywords)) = builtin_persona_match(input) {
        return PersonaResolution {
            input: input.to_string(),
            persona: persona.to_string(),
            resolved: !persona.eq_ignore_ascii_case(input),
            source: "builtin.title_keywords".to_string(),
            confidence: "low".to_string(),
            matched_keywords: matched_keywords.iter().map(|value| value.to_string()).collect(),
            fit_usable: false,
            needs_review: true,
            reason: "input persona matched legacy fallback keywords; add persona_mappings to make this pack-owned".to_string(),
        };
    }

    PersonaResolution {
        input: input.to_string(),
        persona: input.to_string(),
        resolved: false,
        source: "input.persona".to_string(),
        confidence: "explicit".to_string(),
        matched_keywords: Vec::new(),
        fit_usable: true,
        needs_review: false,
        reason: "input persona was provided explicitly but did not match a pack-owned alias"
            .to_string(),
    }
}

pub(crate) fn routable_persona<'a>(
    requested_persona: &'a str,
    resolution: &'a PersonaResolution,
) -> &'a str {
    if resolution.fit_usable {
        resolution.persona.as_str()
    } else {
        requested_persona
    }
}

pub(crate) fn resolve_pack_persona_label(
    manifest: &Manifest,
    input: &str,
    source: &str,
) -> Option<PersonaResolution> {
    for persona in manifest
        .personas
        .iter()
        .chain(manifest.target_personas.iter())
        .chain(manifest.operator_roles.iter())
        .chain(
            manifest
                .persona_mappings
                .iter()
                .map(|mapping| &mapping.persona),
        )
    {
        if persona.eq_ignore_ascii_case(input) {
            return Some(PersonaResolution {
                input: input.to_string(),
                persona: persona.clone(),
                resolved: persona != input,
                source: source.to_string(),
                confidence: "explicit".to_string(),
                matched_keywords: Vec::new(),
                fit_usable: true,
                needs_review: false,
                reason: "input matched a pack-owned persona".to_string(),
            });
        }
    }

    for mapping in &manifest.persona_mappings {
        let matched_keywords: Vec<String> = mapping
            .title_keywords
            .iter()
            .filter(|keyword| title_keyword_matches(input, keyword))
            .cloned()
            .collect();
        if !matched_keywords.is_empty() {
            return Some(PersonaResolution {
                input: input.to_string(),
                persona: mapping.persona.clone(),
                resolved: !mapping.persona.eq_ignore_ascii_case(input),
                source: "manifest.persona_mappings.title_keywords".to_string(),
                confidence: "medium".to_string(),
                matched_keywords,
                fit_usable: true,
                needs_review: false,
                reason: "input matched pack-owned persona mapping".to_string(),
            });
        }
    }

    None
}

#[cfg(test)]
pub(crate) fn infer_persona(title: &str) -> &str {
    builtin_persona_match(title)
        .map(|(persona, _)| persona)
        .unwrap_or("Operator")
}

fn builtin_persona_match(title: &str) -> Option<(&'static str, Vec<&'static str>)> {
    let keyword_groups = [
        (
            "VP Finance",
            ["cfo", "controller", "finance", "accounting"].as_slice(),
        ),
        ("GTM Engineering", ["revops", "gtm", "growth"].as_slice()),
    ];
    for (persona, keywords) in keyword_groups {
        let matched_keywords: Vec<&str> = keywords
            .iter()
            .copied()
            .filter(|keyword| title_keyword_matches(title, keyword))
            .collect();
        if !matched_keywords.is_empty() {
            return Some((persona, matched_keywords));
        }
    }
    None
}

fn title_keyword_matches(title: &str, keyword: &str) -> bool {
    let title = normalize_keyword(title);
    let keyword = normalize_keyword(keyword);
    !keyword.is_empty() && title.contains(&keyword)
}

fn normalize_keyword(input: &str) -> String {
    input
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn valid_domain_host(host: &str) -> bool {
    if host.len() > 253 || !host.contains('.') {
        return false;
    }
    host.split('.').all(valid_domain_label)
}

fn valid_domain_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && label
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        && label
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        && label
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_alphanumeric())
}

pub(crate) fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_collapses_non_alphanumeric_runs() {
        assert_eq!(
            slugify("  Example Message Pack!! 2026  "),
            "example-message-pack-2026"
        );
    }

    #[test]
    fn infer_persona_preserves_existing_title_mapping() {
        assert_eq!(infer_persona("Corporate Controller"), "VP Finance");
        assert_eq!(infer_persona("GTM Engineering Lead"), "GTM Engineering");
        assert_eq!(infer_persona("Chief of Staff"), "Operator");
    }

    #[test]
    fn resolve_persona_uses_pack_owned_title_mapping() {
        let prospect = Prospect {
            name: "Taylor Lee".to_string(),
            title: "Director of Demand Gen".to_string(),
            company: "ExampleCo".to_string(),
            company_domain: None,
            source_kind: None,
            synthetic: false,
            linkedin_url: None,
            company_url: None,
            background: None,
            trigger: None,
            persona: None,
            segment: None,
            signals: Vec::new(),
            attributes: std::collections::BTreeMap::new(),
        };
        let manifest = Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            profile: None,
            personas: vec!["PMM".to_string()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            persona_mappings: vec![crate::models::PersonaMapping {
                persona: "PMM".to_string(),
                title_keywords: vec!["demand gen".to_string()],
            }],
            lead_input_requirements: crate::models::LeadInputRequirements::default(),
            required_primitives: Vec::new(),
            primitive_map: std::collections::BTreeMap::new(),
            input_contracts: Vec::new(),
            jobs: Vec::new(),
            profile_eval: crate::models::ProfileEval::default(),
            cards: vec![],
            policy: crate::models::Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route: 12,
                json_contract: "mdp.cli.v0".to_string(),
                no_auth_required: true,
            },
            provenance: crate::models::Provenance {
                owner: "test".to_string(),
                created_by: "test".to_string(),
                notes: vec![],
            },
        };

        let resolution = resolve_persona(&manifest, &prospect);

        assert_eq!(resolution.persona, "PMM");
        assert_eq!(resolution.input, "Director of Demand Gen");
        assert!(resolution.resolved);
        assert_eq!(
            resolution.source,
            "manifest.persona_mappings.title_keywords"
        );
        assert!(resolution.fit_usable);
        assert!(!resolution.needs_review);
    }

    #[test]
    fn resolve_persona_label_uses_pack_owned_alias_mapping() {
        let manifest = Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            profile: None,
            personas: vec!["GTM Engineering".to_string()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            persona_mappings: vec![crate::models::PersonaMapping {
                persona: "GTM Engineering".to_string(),
                title_keywords: vec!["growth engineer".to_string()],
            }],
            lead_input_requirements: crate::models::LeadInputRequirements::default(),
            required_primitives: Vec::new(),
            primitive_map: std::collections::BTreeMap::new(),
            input_contracts: Vec::new(),
            jobs: Vec::new(),
            profile_eval: crate::models::ProfileEval::default(),
            cards: vec![],
            policy: crate::models::Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route: 12,
                json_contract: "mdp.cli.v0".to_string(),
                no_auth_required: true,
            },
            provenance: crate::models::Provenance {
                owner: "test".to_string(),
                created_by: "test".to_string(),
                notes: vec![],
            },
        };

        let resolution = resolve_persona_label(&manifest, "Growth Engineer");

        assert_eq!(resolution.input, "Growth Engineer");
        assert_eq!(resolution.persona, "GTM Engineering");
        assert!(resolution.resolved);
        assert_eq!(
            resolution.source,
            "manifest.persona_mappings.title_keywords"
        );
        assert!(resolution.fit_usable);
        assert_eq!(
            routable_persona("Growth Engineer", &resolution),
            "GTM Engineering"
        );
    }

    #[test]
    fn normalizes_supplied_company_domains_without_lookup() {
        assert_eq!(
            normalize_supplied_company_domain("https://www.apple.com/").unwrap(),
            "apple.com"
        );
        assert_eq!(
            normalize_supplied_company_domain("http://apple.com/path?x=1").unwrap(),
            "apple.com"
        );
        assert_eq!(
            normalize_supplied_company_domain("WWW.APPLE.COM").unwrap(),
            "apple.com"
        );
        assert_eq!(
            normalize_supplied_company_domain("app.store.apple.com/").unwrap(),
            "app.store.apple.com"
        );
    }

    #[test]
    fn rejects_non_domain_company_values() {
        assert!(normalize_supplied_company_domain("Apple Inc").is_err());
        assert!(normalize_supplied_company_domain("not_a_domain").is_err());
        assert!(normalize_supplied_company_domain("https://").is_err());
    }
}
