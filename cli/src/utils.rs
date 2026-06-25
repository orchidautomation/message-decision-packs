use crate::models::{Manifest, Prospect};
use serde::Serialize;

pub(crate) fn prospect_haystack_with_persona(
    prospect: &Prospect,
    resolved_persona: Option<&str>,
) -> String {
    let mut parts = vec![
        prospect.name.clone(),
        prospect.title.clone(),
        prospect.company.clone(),
    ];
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
    parts.join(" ").to_lowercase()
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PersonaResolution {
    pub(crate) persona: String,
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
        return PersonaResolution {
            persona: persona.to_string(),
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
                persona: mapping.persona.clone(),
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
            persona: persona.to_string(),
            source: "builtin.title_keywords".to_string(),
            confidence: "low".to_string(),
            matched_keywords: matched_keywords.iter().map(|value| value.to_string()).collect(),
            fit_usable: false,
            needs_review: true,
            reason: "title matched legacy fallback keywords; add persona_mappings to make this pack-owned".to_string(),
        };
    }

    PersonaResolution {
        persona: "Operator".to_string(),
        source: "fallback".to_string(),
        confidence: "none".to_string(),
        matched_keywords: Vec::new(),
        fit_usable: false,
        needs_review: true,
        reason: "no explicit persona or pack-owned title mapping matched".to_string(),
    }
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
            source_kind: None,
            synthetic: false,
            linkedin_url: None,
            company_url: None,
            background: None,
            trigger: None,
            persona: None,
            segment: None,
            signals: Vec::new(),
        };
        let manifest = Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            personas: vec!["PMM".to_string()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            persona_mappings: vec![crate::models::PersonaMapping {
                persona: "PMM".to_string(),
                title_keywords: vec!["demand gen".to_string()],
            }],
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
        assert_eq!(
            resolution.source,
            "manifest.persona_mappings.title_keywords"
        );
        assert!(resolution.fit_usable);
        assert!(!resolution.needs_review);
    }
}
