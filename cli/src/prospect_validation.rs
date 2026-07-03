use serde_json::{Value, json};

const PROSPECT_FIELDS: &[&str] = &[
    "name",
    "title",
    "company",
    "company_domain",
    "source_kind",
    "synthetic",
    "linkedin_url",
    "company_url",
    "background",
    "trigger",
    "persona",
    "segment",
    "signals",
    "attributes",
];

const SIGNAL_FIELDS: &[&str] = &[
    "id",
    "title",
    "source",
    "confidence",
    "freshness",
    "state_as",
];

pub(crate) fn validate_prospect_value(value: &Value, path: &str) -> Vec<Value> {
    let mut issues = Vec::new();
    let Some(prospect) = value.as_object() else {
        return issues;
    };

    for key in prospect.keys() {
        if !PROSPECT_FIELDS.contains(&key.as_str()) {
            issues.push(issue(
                "prospect_unknown_field",
                format!("{path}#/{key}"),
                format!(
                    "unsupported prospect field {key}; move bounded reviewed metadata to attributes.{key}, or preserve evidence and provenance in signals[].source"
                ),
            ));
        }
    }

    if let Some(signals) = prospect.get("signals").and_then(Value::as_array) {
        for (index, signal) in signals.iter().enumerate() {
            let Some(signal) = signal.as_object() else {
                continue;
            };
            for key in signal.keys() {
                if !SIGNAL_FIELDS.contains(&key.as_str()) {
                    issues.push(issue(
                        "prospect_signal_unknown_field",
                        format!("{path}#/signals/{index}/{key}"),
                        format!(
                            "unsupported prospect signal field {key}; keep signal evidence and provenance in signals[].source"
                        ),
                    ));
                }
            }
        }
    }

    issues
}

pub(crate) fn prospect_validation_error(issues: &[Value]) -> String {
    let details = issues
        .iter()
        .filter_map(|issue| {
            let code = issue.get("code")?.as_str()?;
            let path = issue.get("path")?.as_str()?;
            let message = issue.get("message")?.as_str()?;
            Some(format!("{code} at {path}: {message}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("invalid prospect input: {details}")
}

fn issue(code: &str, path: String, message: String) -> Value {
    json!({
        "code": code,
        "severity": "error",
        "path": path,
        "message": message
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_unknown_top_level_fields_with_attribute_guidance() {
        let value = json!({
            "name": "Taylor Lee",
            "title": "GTM Engineering Lead",
            "company": "ExampleCo",
            "territory": "enterprise"
        });

        let issues = validate_prospect_value(&value, "prospect.json");

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0]["code"], "prospect_unknown_field");
        assert_eq!(issues[0]["path"], "prospect.json#/territory");
        assert!(
            issues[0]["message"]
                .as_str()
                .expect("message should be present")
                .contains("attributes.territory")
        );
    }

    #[test]
    fn reports_unknown_signal_fields_with_source_guidance() {
        let value = json!({
            "name": "Taylor Lee",
            "title": "GTM Engineering Lead",
            "company": "ExampleCo",
            "signals": [{
                "id": "workflow-change",
                "title": "Workflow change",
                "url": "https://example.com"
            }]
        });

        let issues = validate_prospect_value(&value, "prospect.json");

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0]["code"], "prospect_signal_unknown_field");
        assert_eq!(issues[0]["path"], "prospect.json#/signals/0/url");
        assert!(
            issues[0]["message"]
                .as_str()
                .expect("message should be present")
                .contains("signals[].source")
        );
    }

    #[test]
    fn accepts_attributes_as_the_extension_point() {
        let value = json!({
            "name": "Taylor Lee",
            "title": "GTM Engineering Lead",
            "company": "ExampleCo",
            "attributes": {
                "territory": "enterprise"
            }
        });

        let issues = validate_prospect_value(&value, "prospect.json");

        assert!(issues.is_empty());
    }
}
