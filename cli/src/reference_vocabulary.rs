use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) const REFERENCE_ANNOTATION: &str = "x-mdp-reference";
const MAX_REFERENCE_ANNOTATIONS: usize = 128;
const MAX_VOCABULARY_VALUES: usize = 1024;
const MAX_REFERENCE_BYTES: usize = 256;
const MAX_REFERENCE_VIOLATIONS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceViolation {
    pub(crate) path: String,
    pub(crate) expected: String,
    pub(crate) observed: &'static str,
}

#[derive(Debug, Clone)]
struct ReferenceDeclaration {
    select: String,
    card_kinds: BTreeSet<String>,
    allow: BTreeSet<String>,
}

pub(crate) fn compile_reference_schema(schema: &Value, routed_context: &Value) -> Result<Value> {
    validate_reference_annotations(schema)?;
    let mut compiled = schema.clone();
    let mut count = 0;
    walk_schema_mut(&mut compiled, routed_context, &mut count, "#")?;
    Ok(compiled)
}

pub(crate) fn validate_declared_references(
    schema: &Value,
    routed_context: &Value,
    instance: &Value,
) -> Result<Vec<ReferenceViolation>> {
    validate_reference_annotations(schema)?;
    let mut violations = Vec::new();
    walk_instance(schema, routed_context, instance, "#", &mut violations)?;
    Ok(violations)
}

pub(crate) fn validate_reference_annotations(schema: &Value) -> Result<()> {
    let mut count = 0;
    validate_annotation_walk(schema, &mut count, "#", true)
}

fn declaration(node: &Value, path: &str) -> Result<Option<ReferenceDeclaration>> {
    let Some(raw) = node.get(REFERENCE_ANNOTATION) else {
        return Ok(None);
    };
    let object = raw.as_object().ok_or_else(|| {
        anyhow!("reference-annotation-invalid at {path}: annotation must be an object")
    })?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "source" | "select" | "card_kinds" | "allow"))
    {
        return Err(anyhow!(
            "reference-annotation-invalid at {path}: unsupported annotation field"
        ));
    }
    if object.get("source").and_then(Value::as_str) != Some("routed-context") {
        return Err(anyhow!(
            "reference-annotation-invalid at {path}: source must be routed-context"
        ));
    }
    let select = object
        .get("select")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "qualified-entry" | "entry-id" | "evidence-id"))
        .ok_or_else(|| {
            anyhow!(
                "reference-annotation-invalid at {path}: select must be qualified-entry, entry-id, or evidence-id"
            )
        })?
        .to_string();
    let card_kinds = string_set(object.get("card_kinds"), path, "card_kinds")?;
    let allow = string_set(object.get("allow"), path, "allow")?;
    if select == "evidence-id" && !card_kinds.is_empty() {
        return Err(anyhow!(
            "reference-annotation-invalid at {path}: evidence-id does not accept card_kinds"
        ));
    }
    Ok(Some(ReferenceDeclaration {
        select,
        card_kinds,
        allow,
    }))
}

fn string_set(value: Option<&Value>, path: &str, field: &str) -> Result<BTreeSet<String>> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let values = value.as_array().ok_or_else(|| {
        anyhow!("reference-annotation-invalid at {path}: {field} must be an array")
    })?;
    let mut result = BTreeSet::new();
    for value in values {
        let value = value.as_str().filter(|value| {
            !value.is_empty() && value.len() <= MAX_REFERENCE_BYTES && value.trim() == *value
        });
        let Some(value) = value else {
            return Err(anyhow!(
                "reference-annotation-invalid at {path}: {field} values must be bounded non-blank strings"
            ));
        };
        result.insert(value.to_string());
    }
    Ok(result)
}

fn vocabulary(
    declaration: &ReferenceDeclaration,
    context: &Value,
    path: &str,
) -> Result<Vec<Value>> {
    let entries = context
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("reference-context-invalid at {path}: entries must be an array"))?;
    let mut values = BTreeSet::new();
    for entry in entries {
        let Some(card_id) = entry.get("card_id").and_then(Value::as_str) else {
            continue;
        };
        let Some(entry_id) = entry.get("entry_id").and_then(Value::as_str) else {
            continue;
        };
        let kind = entry.get("card_kind").and_then(Value::as_str).unwrap_or("");
        if !declaration.card_kinds.is_empty() && !declaration.card_kinds.contains(kind) {
            continue;
        }
        match declaration.select.as_str() {
            "qualified-entry" => {
                values.insert(format!("{card_id}/{entry_id}"));
            }
            "entry-id" => {
                values.insert(entry_id.to_string());
            }
            "evidence-id" => {
                values.extend(
                    entry
                        .get("evidence")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned),
                );
            }
            _ => unreachable!(),
        }
    }
    if declaration.card_kinds.is_empty() && declaration.select != "evidence-id" {
        for reference in context
            .get("product_foundation_load_order")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|reference| {
                reference.get("reference_kind").and_then(Value::as_str) == Some("entry")
            })
        {
            let (Some(card_id), Some(entry_id)) = (
                reference.get("card_id").and_then(Value::as_str),
                reference.get("entry_id").and_then(Value::as_str),
            ) else {
                continue;
            };
            values.insert(if declaration.select == "qualified-entry" {
                format!("{card_id}/{entry_id}")
            } else {
                entry_id.to_string()
            });
        }
    }
    values.extend(declaration.allow.iter().cloned());
    if values.is_empty() {
        return Err(anyhow!("reference-vocabulary-empty at {path}"));
    }
    if values.len() > MAX_VOCABULARY_VALUES
        || values.iter().any(|value| value.len() > MAX_REFERENCE_BYTES)
    {
        return Err(anyhow!("reference-vocabulary-limit-exceeded at {path}"));
    }
    Ok(values.into_iter().map(Value::String).collect())
}

fn bump(count: &mut usize, path: &str) -> Result<()> {
    *count += 1;
    if *count > MAX_REFERENCE_ANNOTATIONS {
        return Err(anyhow!("reference-annotation-limit-exceeded at {path}"));
    }
    Ok(())
}

fn walk_schema_mut(node: &mut Value, context: &Value, count: &mut usize, path: &str) -> Result<()> {
    if let Some(declaration) = declaration(node, path)? {
        bump(count, path)?;
        let values = vocabulary(&declaration, context, path)?;
        let object = node
            .as_object_mut()
            .expect("annotated schema node is an object");
        object.remove(REFERENCE_ANNOTATION);
        object.insert("enum".into(), Value::Array(values));
    }
    walk_schema_children_mut(node, |child, segment| {
        walk_schema_mut(child, context, count, &format!("{path}/{segment}"))
    })
}

fn validate_annotation_walk(
    node: &Value,
    count: &mut usize,
    path: &str,
    placement_allowed: bool,
) -> Result<()> {
    if declaration(node, path)?.is_some() {
        if !placement_allowed
            || ["$ref", "allOf", "anyOf", "oneOf"]
                .iter()
                .any(|keyword| node.get(keyword).is_some())
        {
            return Err(anyhow!(
                "reference-annotation-invalid at {path}: annotations are not supported in or on $ref, allOf, anyOf, or oneOf"
            ));
        }
        bump(count, path)?;
        if node.get("type").and_then(Value::as_str) != Some("string") {
            return Err(anyhow!(
                "reference-annotation-invalid at {path}: annotated node type must be string"
            ));
        }
        if node.get("enum").is_some() || node.get("const").is_some() {
            return Err(anyhow!(
                "reference-annotation-invalid at {path}: enum and const are compiled from routed authority"
            ));
        }
    }
    let Some(object) = node.as_object() else {
        return Ok(());
    };
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            validate_annotation_walk(
                child,
                count,
                &format!("{path}/properties/{name}"),
                placement_allowed,
            )?;
        }
    }
    if let Some(child) = object.get("items") {
        validate_annotation_walk(child, count, &format!("{path}/items"), placement_allowed)?;
    }
    if let Some(definitions) = object.get("$defs").and_then(Value::as_object) {
        for (name, child) in definitions {
            validate_annotation_walk(child, count, &format!("{path}/$defs/{name}"), false)?;
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(children) = object.get(keyword).and_then(Value::as_array) {
            for (index, child) in children.iter().enumerate() {
                validate_annotation_walk(
                    child,
                    count,
                    &format!("{path}/{keyword}/{index}"),
                    false,
                )?;
            }
        }
    }
    Ok(())
}

fn walk_instance(
    schema: &Value,
    context: &Value,
    instance: &Value,
    path: &str,
    violations: &mut Vec<ReferenceViolation>,
) -> Result<()> {
    if let Some(declaration) = declaration(schema, path)? {
        let expected_values = vocabulary(&declaration, context, path)?;
        let accepted = instance.as_str().is_some_and(|observed| {
            expected_values
                .iter()
                .any(|value| value.as_str() == Some(observed))
        });
        if !accepted && violations.len() < MAX_REFERENCE_VIOLATIONS {
            violations.push(ReferenceViolation {
                path: path.to_string(),
                expected: declaration.select,
                observed: json_type(instance),
            });
        }
    }
    if let (Some(properties), Some(instance)) = (
        schema.get("properties").and_then(Value::as_object),
        instance.as_object(),
    ) {
        for (name, child_schema) in properties {
            if let Some(child) = instance.get(name) {
                walk_instance(
                    child_schema,
                    context,
                    child,
                    &format!("{path}/{name}"),
                    violations,
                )?;
            }
        }
    }
    if let (Some(items), Some(values)) = (schema.get("items"), instance.as_array()) {
        for (index, value) in values.iter().enumerate() {
            walk_instance(
                items,
                context,
                value,
                &format!("{path}/{index}"),
                violations,
            )?;
        }
    }
    Ok(())
}

fn walk_schema_children_mut(
    node: &mut Value,
    mut visit: impl FnMut(&mut Value, String) -> Result<()>,
) -> Result<()> {
    let Some(object) = node.as_object_mut() else {
        return Ok(());
    };
    for keyword in ["properties", "$defs"] {
        if let Some(children) = object.get_mut(keyword).and_then(Value::as_object_mut) {
            for (name, child) in children {
                visit(child, format!("{keyword}/{name}"))?;
            }
        }
    }
    if let Some(child) = object.get_mut("items") {
        visit(child, "items".into())?;
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(children) = object.get_mut(keyword).and_then(Value::as_array_mut) {
            for (index, child) in children.iter_mut().enumerate() {
                visit(child, format!("{keyword}/{index}"))?;
            }
        }
    }
    Ok(())
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string-outside-declared-vocabulary",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn context() -> Value {
        json!({"entries": [
            {"card_id":"claims","card_kind":"claims","entry_id":"supported","evidence":["proof-1"]},
            {"card_id":"policies","card_kind":"domain-policy","entry_id":"safe","evidence":[]}
        ]})
    }

    #[test]
    fn compiles_nested_and_domain_extension_references() {
        let schema = json!({"type":"object","properties":{"rows":{"type":"array","items":{"type":"object","properties":{
            "claim":{"type":"string","x-mdp-reference":{"source":"routed-context","select":"entry-id","card_kinds":["claims"]}},
            "policy":{"type":"string","x-mdp-reference":{"source":"routed-context","select":"entry-id","card_kinds":["domain-policy"]}}
        }}}}});
        let compiled = compile_reference_schema(&schema, &context()).unwrap();
        assert_eq!(
            compiled.pointer("/properties/rows/items/properties/claim/enum"),
            Some(&json!(["supported"]))
        );
        assert_eq!(
            compiled.pointer("/properties/rows/items/properties/policy/enum"),
            Some(&json!(["safe"]))
        );
        assert!(compiled.to_string().find(REFERENCE_ANNOTATION).is_none());
    }

    #[test]
    fn rejects_unknown_without_retaining_observed_content() {
        let schema = json!({"type":"string","x-mdp-reference":{"source":"routed-context","select":"evidence-id"}});
        let violations =
            validate_declared_references(&schema, &context(), &json!("private invented prose"))
                .unwrap();
        assert_eq!(violations[0].observed, "string-outside-declared-vocabulary");
        assert!(!format!("{violations:?}").contains("private invented prose"));
    }

    #[test]
    fn rejects_ref_backed_annotations_at_both_provider_and_local_boundaries() {
        let schema = json!({
            "$defs": {
                "authorityReference": {
                    "type":"string",
                    "x-mdp-reference":{"source":"routed-context","select":"qualified-entry"}
                }
            },
            "type":"object",
            "properties":{"authority":{"$ref":"#/$defs/authorityReference"}}
        });
        let provider_error = compile_reference_schema(&schema, &context()).unwrap_err();
        let local_error = validate_declared_references(
            &schema,
            &context(),
            &json!({"authority":"claims/supported"}),
        )
        .unwrap_err();
        assert!(
            provider_error
                .to_string()
                .contains("reference-annotation-invalid")
        );
        assert_eq!(provider_error.to_string(), local_error.to_string());
    }

    #[test]
    fn rejects_composed_annotations_instead_of_diverging_on_branch_selection() {
        for keyword in ["anyOf", "oneOf", "allOf"] {
            let schema = json!({
                "type":"object",
                "properties":{"reference":{
                    (keyword): [
                        {"type":"string","x-mdp-reference":{"source":"routed-context","select":"entry-id","card_kinds":["claims"]}},
                        {"type":"null"}
                    ]
                }}
            });
            let provider_error = compile_reference_schema(&schema, &context()).unwrap_err();
            let local_error =
                validate_declared_references(&schema, &context(), &json!({"reference":null}))
                    .unwrap_err();
            assert!(
                provider_error
                    .to_string()
                    .contains("reference-annotation-invalid")
            );
            assert_eq!(provider_error.to_string(), local_error.to_string());
        }

        let annotation_on_composition = json!({
            "type":"string",
            "anyOf":[{"type":"string"}],
            "x-mdp-reference":{"source":"routed-context","select":"entry-id"}
        });
        assert!(
            validate_reference_annotations(&annotation_on_composition)
                .unwrap_err()
                .to_string()
                .contains("reference-annotation-invalid")
        );
    }

    #[test]
    fn fails_closed_when_vocabulary_exceeds_bound() {
        let entries = (0..=MAX_VOCABULARY_VALUES)
            .map(|index| json!({"card_id":"c","card_kind":"claims","entry_id":format!("e-{index}"),"evidence":[]}))
            .collect::<Vec<_>>();
        let schema = json!({"type":"string","x-mdp-reference":{"source":"routed-context","select":"entry-id"}});
        assert!(
            compile_reference_schema(&schema, &json!({"entries":entries}))
                .unwrap_err()
                .to_string()
                .contains("reference-vocabulary-limit-exceeded")
        );
    }

    #[test]
    fn domain_shaped_extensions_share_the_same_neutral_contract() {
        for (kind, entry_id) in [
            ("claims", "gtm-claim"),
            ("resolution-play", "support-resolution"),
            ("interview-rubric", "recruiting-rubric"),
            ("proposal-policy", "proposal-boundary"),
        ] {
            let context = json!({"entries":[{
                "card_id":"authority",
                "card_kind":kind,
                "entry_id":entry_id,
                "evidence":[]
            }]});
            let schema = json!({
                "type":"array",
                "items":{
                    "type":"object",
                    "properties":{
                        "reference":{
                            "type":"string",
                            "x-mdp-reference":{
                                "source":"routed-context",
                                "select":"entry-id",
                                "card_kinds":[kind]
                            }
                        }
                    }
                }
            });
            let compiled = compile_reference_schema(&schema, &context).unwrap();
            assert_eq!(
                compiled.pointer("/items/properties/reference/enum"),
                Some(&json!([entry_id]))
            );
        }
    }
}
