use crate::models::ArtifactTextField;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const MAX_TEXT_SURFACES: usize = 64;
const MAX_TEXT_SURFACE_PATH_BYTES: usize = 256;
const MAX_AVOID_TERMS: usize = 128;
const MAX_AVOID_TERM_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextSurfaceValue {
    pub(crate) field_path: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextGuardrailHit {
    pub(crate) field_path: String,
    pub(crate) card_id: String,
    pub(crate) entry_id: String,
    pub(crate) title: String,
    pub(crate) term: String,
}

pub(crate) fn validate_declarations(fields: &[ArtifactTextField]) -> Result<()> {
    if fields.len() > MAX_TEXT_SURFACES {
        return Err(anyhow!("artifact-text-surface-limit-exceeded"));
    }
    let mut paths = BTreeSet::new();
    let mut adapters = BTreeSet::new();
    for field in fields {
        validate_pointer(&field.path)?;
        if !field.path.starts_with("/artifact/") {
            return Err(anyhow!(
                "artifact-text-surface-invalid: path must be below /artifact"
            ));
        }
        if !paths.insert(field.path.as_str()) {
            return Err(anyhow!("artifact-text-surface-invalid: duplicate path"));
        }
        if let Some(adapter) = field.legacy_input.as_deref() {
            if !matches!(adapter, "text" | "subject") || !adapters.insert(adapter) {
                return Err(anyhow!(
                    "artifact-text-surface-invalid: legacy_input must be unique text or subject"
                ));
            }
        }
    }
    Ok(())
}

fn validate_pointer(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > MAX_TEXT_SURFACE_PATH_BYTES
        || !path.starts_with('/')
        || path.ends_with('/')
        || path.split('/').skip(1).any(|segment| {
            segment.is_empty()
                || segment.len() > 64
                || segment.contains("~2")
                || invalid_tilde_escape(segment)
        })
    {
        return Err(anyhow!(
            "artifact-text-surface-invalid: invalid JSON Pointer"
        ));
    }
    Ok(())
}

fn invalid_tilde_escape(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    (0..bytes.len()).any(|index| {
        bytes[index] == b'~'
            && (index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1'))
    })
}

pub(crate) fn declared_values(
    instance: &Value,
    fields: &[ArtifactTextField],
) -> Result<Vec<TextSurfaceValue>> {
    validate_declarations(fields)?;
    let mut values = Vec::new();
    for field in fields {
        let Some(value) = instance.pointer(&field.path) else {
            continue;
        };
        match value {
            Value::String(text) => values.push(TextSurfaceValue {
                field_path: field.path.clone(),
                text: text.clone(),
            }),
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    let text = item.as_str().ok_or_else(|| {
                        anyhow!("artifact-text-surface-invalid: declared arrays must contain only strings")
                    })?;
                    values.push(TextSurfaceValue {
                        field_path: format!("{}/{index}", field.path),
                        text: text.to_string(),
                    });
                }
            }
            _ => {
                return Err(anyhow!(
                    "artifact-text-surface-invalid: declared value must be a string or string array"
                ));
            }
        }
    }
    Ok(values)
}

pub(crate) fn avoid_terms(routed_context: &Value) -> Result<Vec<String>> {
    let mut terms = BTreeSet::new();
    for value in routed_context
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|entry| {
            entry
                .get("avoid")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
    {
        let term = value.as_str().filter(|term| {
            !term.trim().is_empty() && term.trim() == *term && term.len() <= MAX_AVOID_TERM_BYTES
        });
        let Some(term) = term else {
            return Err(anyhow!("artifact-text-guardrail-invalid"));
        };
        terms.insert(term.to_string());
        if terms.len() > MAX_AVOID_TERMS {
            return Err(anyhow!("artifact-text-guardrail-limit-exceeded"));
        }
    }
    Ok(terms.into_iter().collect())
}

pub(crate) fn guardrail_hits(
    instance: &Value,
    fields: &[ArtifactTextField],
    routed_context: &Value,
) -> Result<Vec<TextGuardrailHit>> {
    let values = declared_values(instance, fields)?;
    if fields.is_empty() {
        return Ok(Vec::new());
    }
    // Validate the same bounded vocabulary compiled into the provider schema so
    // local validation cannot accept context that provider preparation rejects.
    let _ = avoid_terms(routed_context)?;
    let mut hits = Vec::new();
    for entry in routed_context
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let card_id = entry
            .get("card_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let entry_id = entry
            .get("entry_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let title = entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        for term in entry
            .get("avoid")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if term.trim().is_empty() || term.len() > MAX_AVOID_TERM_BYTES {
                return Err(anyhow!("artifact-text-guardrail-invalid"));
            }
            for value in &values {
                if contains_ascii_case_insensitive(&value.text, term) {
                    hits.push(TextGuardrailHit {
                        field_path: value.field_path.clone(),
                        card_id: card_id.to_string(),
                        entry_id: entry_id.to_string(),
                        title: title.to_string(),
                        term: term.to_string(),
                    });
                }
            }
        }
    }
    if hits.len() > MAX_TEXT_SURFACES * MAX_AVOID_TERMS {
        return Err(anyhow!("artifact-text-guardrail-limit-exceeded"));
    }
    Ok(hits)
}

pub(crate) fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    !needle.is_empty()
        && haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

pub(crate) fn compile_provider_schema(
    schema: &Value,
    fields: &[ArtifactTextField],
    routed_context: &Value,
) -> Result<Value> {
    validate_declarations(fields)?;
    if fields.is_empty() {
        return Ok(schema.clone());
    }
    let terms = avoid_terms(routed_context)?;
    if terms.is_empty() {
        return Ok(schema.clone());
    }
    let pattern = negative_contains_pattern(&terms)?;
    let mut compiled = schema.clone();
    for field in fields {
        let node = schema_node_mut(&mut compiled, &field.path)?;
        let target = match node.get("type").and_then(Value::as_str) {
            Some("string") => node,
            Some("array") => node
                .get_mut("items")
                .filter(|items| items.get("type").and_then(Value::as_str) == Some("string"))
                .ok_or_else(|| anyhow!("artifact-text-surface-schema-invalid"))?,
            _ => return Err(anyhow!("artifact-text-surface-schema-invalid")),
        };
        let object = target
            .as_object_mut()
            .ok_or_else(|| anyhow!("artifact-text-surface-schema-invalid"))?;
        if object.contains_key("pattern") {
            return Err(anyhow!("artifact-text-surface-schema-pattern-conflict"));
        }
        object.insert("pattern".into(), Value::String(pattern.clone()));
    }
    Ok(compiled)
}

fn schema_node_mut<'a>(schema: &'a mut Value, instance_path: &str) -> Result<&'a mut Value> {
    let mut node = schema;
    for encoded in instance_path.split('/').skip(1) {
        let segment = encoded.replace("~1", "/").replace("~0", "~");
        node = node
            .get_mut("properties")
            .and_then(|properties| properties.get_mut(&segment))
            .ok_or_else(|| anyhow!("artifact-text-surface-schema-missing"))?;
    }
    Ok(node)
}

fn negative_contains_pattern(terms: &[String]) -> Result<String> {
    let alternatives = terms
        .iter()
        .map(|term| ascii_case_insensitive_regex(term))
        .collect::<Vec<_>>()
        .join("|");
    let pattern = format!("^(?!(?:[\\s\\S])*(?:{alternatives}))[\\s\\S]*$");
    if pattern.len() > 32_768 {
        return Err(anyhow!("artifact-text-guardrail-limit-exceeded"));
    }
    Ok(pattern)
}

fn ascii_case_insensitive_regex(term: &str) -> String {
    let mut escaped = String::new();
    for character in term.chars() {
        if character.is_ascii_alphabetic() {
            escaped.push('[');
            escaped.push(character.to_ascii_lowercase());
            escaped.push(character.to_ascii_uppercase());
            escaped.push(']');
        } else {
            if matches!(
                character,
                '\\' | '.'
                    | '^'
                    | '$'
                    | '|'
                    | '?'
                    | '*'
                    | '+'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '-'
            ) {
                escaped.push('\\');
            }
            escaped.push(character);
        }
    }
    escaped
}

pub(crate) fn direct_instance(
    fields: &[ArtifactTextField],
    artifact: Option<Value>,
    generic_fields: &[String],
    text: Option<&str>,
    subject: Option<&str>,
) -> Result<Value> {
    validate_declarations(fields)?;
    let mut instance = artifact.unwrap_or_else(|| json!({}));
    if !instance.is_object() {
        return Err(anyhow!("--artifact must contain a JSON object"));
    }
    for raw in generic_fields {
        let (path, value) = raw
            .split_once('=')
            .ok_or_else(|| anyhow!("--field must be JSON_POINTER=TEXT"))?;
        if !fields.iter().any(|field| field.path == path) {
            return Err(anyhow!("--field path is not declared by the selected job"));
        }
        insert_pointer_repeatable(&mut instance, path, value)?;
    }
    for (adapter, value) in [("text", text), ("subject", subject)] {
        let Some(value) = value else { continue };
        let path = fields
            .iter()
            .find(|field| field.legacy_input.as_deref() == Some(adapter))
            .map(|field| field.path.as_str())
            .ok_or_else(|| anyhow!("selected job does not declare a {adapter} legacy adapter"))?;
        insert_pointer(&mut instance, path, Value::String(value.to_string()))?;
    }
    Ok(instance)
}

fn insert_pointer(root: &mut Value, path: &str, value: Value) -> Result<()> {
    validate_pointer(path)?;
    let segments = path
        .split('/')
        .skip(1)
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    let mut current = root;
    for segment in &segments[..segments.len() - 1] {
        let object = current
            .as_object_mut()
            .ok_or_else(|| anyhow!("declared field overlaps a non-object value"))?;
        current = object.entry(segment.clone()).or_insert_with(|| json!({}));
    }
    let object = current
        .as_object_mut()
        .ok_or_else(|| anyhow!("declared field overlaps a non-object value"))?;
    let leaf = segments.last().expect("validated pointer is nonempty");
    if object.insert(leaf.clone(), value).is_some() {
        return Err(anyhow!("declared field was supplied more than once"));
    }
    Ok(())
}

fn insert_pointer_repeatable(root: &mut Value, path: &str, value: &str) -> Result<()> {
    if let Some(existing) = root.pointer_mut(path) {
        match existing {
            Value::String(first) => {
                *existing = Value::Array(vec![
                    Value::String(std::mem::take(first)),
                    Value::String(value.to_string()),
                ]);
            }
            Value::Array(items) if items.iter().all(Value::is_string) => {
                items.push(Value::String(value.to_string()));
            }
            _ => return Err(anyhow!("declared field overlaps a non-text value")),
        }
        return Ok(());
    }
    insert_pointer(root, path, Value::String(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ArtifactTextField;

    fn fields() -> Vec<ArtifactTextField> {
        vec![
            ArtifactTextField {
                path: "/artifact/body".into(),
                legacy_input: Some("text".into()),
            },
            ArtifactTextField {
                path: "/artifact/options".into(),
                legacy_input: Some("subject".into()),
            },
        ]
    }

    #[test]
    fn walks_only_declared_scalar_and_array_paths_across_domain_shapes() {
        for (path, artifact) in [
            (
                "/artifact/message",
                json!({"artifact":{"message":"safe","metadata":"forbidden"}}),
            ),
            (
                "/artifact/reply",
                json!({"artifact":{"reply":"safe","ticket_id":"forbidden"}}),
            ),
            (
                "/artifact/summary",
                json!({"artifact":{"summary":"safe","candidate_id":"forbidden"}}),
            ),
            (
                "/artifact/sections",
                json!({"artifact":{"sections":["one","two"],"metadata":"forbidden"}}),
            ),
        ] {
            let values = declared_values(
                &artifact,
                &[ArtifactTextField {
                    path: path.into(),
                    legacy_input: None,
                }],
            )
            .unwrap();
            assert!(!values.is_empty());
            assert!(values.iter().all(|value| value.text != "forbidden"));
        }
    }

    #[test]
    fn direct_adapters_and_generic_fields_share_declared_paths() {
        let value = direct_instance(
            &fields(),
            None,
            &["/artifact/options=generic".into()],
            Some("body"),
            None,
        )
        .unwrap();
        assert_eq!(value.pointer("/artifact/body"), Some(&json!("body")));
        assert_eq!(value.pointer("/artifact/options"), Some(&json!("generic")));
    }

    #[test]
    fn provider_schema_constrains_declared_fields_not_metadata() {
        let schema = json!({"type":"object","properties":{"artifact":{"type":"object","properties":{"body":{"type":"string"},"options":{"type":"array","items":{"type":"string"}},"metadata":{"type":"string"}}}}});
        let context = json!({"entries":[{"avoid":["Forbidden", "—"]}]});
        let compiled = compile_provider_schema(&schema, &fields(), &context).unwrap();
        assert!(
            compiled
                .pointer("/properties/artifact/properties/body/pattern")
                .is_some()
        );
        assert!(
            compiled
                .pointer("/properties/artifact/properties/options/items/pattern")
                .is_some()
        );
        assert!(
            compiled
                .pointer("/properties/artifact/properties/metadata/pattern")
                .is_none()
        );
        assert!(
            jsonschema::draft202012::validate(
                &compiled,
                &json!({"artifact":{"body":"safe","options":["also safe"],"metadata":"Forbidden"}}),
            )
            .is_ok()
        );
        assert!(
            jsonschema::draft202012::validate(
                &compiled,
                &json!({"artifact":{"body":"safe","options":["FORBIDDEN"],"metadata":"safe"}}),
            )
            .is_err()
        );
        assert!(contains_ascii_case_insensitive("CAFÉ", "CAFÉ"));
        assert!(!contains_ascii_case_insensitive("CAFÉ", "café"));
    }

    #[test]
    fn local_guardrails_reject_oversized_routed_vocabularies() {
        let entries = (0..=MAX_AVOID_TERMS)
            .map(|index| json!({"avoid":[format!("term-{index}")]}))
            .collect::<Vec<_>>();
        assert!(
            guardrail_hits(
                &json!({"artifact":{"body":"safe"}}),
                &[ArtifactTextField {
                    path: "/artifact/body".into(),
                    legacy_input: None,
                }],
                &json!({"entries":entries}),
            )
            .unwrap_err()
            .to_string()
            .contains("limit-exceeded")
        );
    }

    #[test]
    fn rejects_duplicate_adapters_and_non_artifact_paths() {
        assert!(
            validate_declarations(&[
                ArtifactTextField {
                    path: "/artifact/a".into(),
                    legacy_input: Some("text".into())
                },
                ArtifactTextField {
                    path: "/artifact/b".into(),
                    legacy_input: Some("text".into())
                },
            ])
            .is_err()
        );
        assert!(
            validate_declarations(&[ArtifactTextField {
                path: "/metadata/a".into(),
                legacy_input: None
            }])
            .is_err()
        );
    }

    #[test]
    fn every_shipped_job_declaration_resolves_in_its_owned_schema() {
        for template in ["basic", "proposal"] {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../plugin/assets/templates")
                .join(template);
            let manifest = crate::pack_io::read_manifest(&root).unwrap();
            for job in manifest
                .jobs
                .iter()
                .filter(|job| !job.artifact_text_fields.is_empty())
            {
                let prompt_id = &job.model_task.as_ref().unwrap().prompt;
                let (_, prompt) = crate::pack_io::read_canonical_prompt_by_id(&root, prompt_id)
                    .unwrap()
                    .unwrap();
                let schema = prompt.output_contract.schema.as_ref().unwrap();
                compile_provider_schema(
                    schema,
                    &job.artifact_text_fields,
                    &json!({"entries":[{"avoid":["synthetic forbidden"]}]}),
                )
                .unwrap_or_else(|error| panic!("{template}/{prompt_id}: {error}"));
            }
        }
    }
}
