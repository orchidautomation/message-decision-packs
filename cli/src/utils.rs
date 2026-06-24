use crate::models::Prospect;

pub(crate) fn prospect_haystack(prospect: &Prospect) -> String {
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

pub(crate) fn infer_persona(title: &str) -> &str {
    let lower = title.to_lowercase();
    if lower.contains("cfo")
        || lower.contains("controller")
        || lower.contains("finance")
        || lower.contains("accounting")
    {
        "VP Finance"
    } else if lower.contains("revops") || lower.contains("gtm") || lower.contains("growth") {
        "GTM Engineering"
    } else {
        "Operator"
    }
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
}
