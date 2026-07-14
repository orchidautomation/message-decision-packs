use crate::commands::proof_output::verify_output_value;
use crate::pack_io::read_manifest;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

const HUMAN_BRIEF_CONTRACT: &str = "mdp.human-brief.v0";

pub(crate) fn render_human_brief_file(
    root: &Path,
    file: Option<&Path>,
    template: &str,
    strict: bool,
) -> Result<Value> {
    let (raw, source_artifact) = match file {
        Some(path) => (
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
            path.display().to_string(),
        ),
        None => {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("reading artifact JSON from stdin")?;
            (buffer, "stdin".to_string())
        }
    };
    let artifact: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing human brief source artifact from {source_artifact}"))?;
    render_human_brief(root, &artifact, &source_artifact, template, strict)
}

pub(crate) fn render_human_brief(
    root: &Path,
    artifact: &Value,
    source_artifact: &str,
    template: &str,
    strict: bool,
) -> Result<Value> {
    match template {
        "gtm-prospect" => render_gtm_prospect(root, artifact, source_artifact, strict),
        "proposal-review" => render_proposal_review(root, artifact, source_artifact, strict),
        "proof-report" => render_proof_report(root, artifact, source_artifact, strict),
        other => bail!(
            "unsupported template `{other}`; available templates: gtm-prospect, proposal-review, proof-report"
        ),
    }
}

pub(crate) fn render_human_brief_markdown(brief: &Value) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    frontmatter(&mut out, "artifact_type", brief["artifact_type"].as_str());
    frontmatter(&mut out, "template_id", brief["template_id"].as_str());
    frontmatter(&mut out, "decision", brief["decision"].as_str());
    frontmatter(&mut out, "pack_id", brief["pack_id"].as_str());
    frontmatter(&mut out, "pack_version", brief["pack_version"].as_str());
    frontmatter(
        &mut out,
        "source_artifact_type",
        brief["source_artifact_type"].as_str(),
    );
    out.push_str("---\n\n");

    writeln!(
        out,
        "# Human Brief: {}\n",
        brief["title"].as_str().unwrap_or("MDP artifact")
    )
    .ok();
    for section in brief["sections"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "## {}\n",
            section["title"].as_str().unwrap_or("Section")
        )
        .ok();
        if let Some(body) = section["body"].as_str() {
            out.push_str(body.trim());
            out.push_str("\n\n");
        }
        if let Some(refs) = section["refs"].as_array().filter(|refs| !refs.is_empty()) {
            out.push_str("Refs:\n");
            for reference in refs {
                writeln!(out, "- `{}`", display_value(reference)).ok();
            }
            out.push('\n');
        }
    }
    if let Some(warnings) = brief["audit"]["warnings"]
        .as_array()
        .filter(|warnings| !warnings.is_empty())
    {
        out.push_str("## Renderer Warnings\n\n");
        for warning in warnings {
            writeln!(out, "- {}", display_value(warning)).ok();
        }
        out.push('\n');
    }
    out
}

fn render_gtm_prospect(
    root: &Path,
    artifact: &Value,
    source_artifact: &str,
    strict: bool,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut warnings = Vec::new();
    require_contract(artifact, "mdp.message-brief.v0", &mut warnings);
    require_field(artifact, "draft_status", &mut warnings);
    require_field(artifact, "fit.status", &mut warnings);
    require_field(artifact, "prospect", &mut warnings);
    fail_on_strict_warnings(strict, &warnings)?;

    let decision = match artifact["draft_status"].as_str() {
        Some("ready") => "ready",
        Some("no-draft") => "no-draft",
        _ => match artifact["fit"]["status"].as_str() {
            Some("disqualified") => "blocked",
            Some("insufficient-context") => "needs-review",
            _ => "needs-review",
        },
    };
    let prospect = &artifact["prospect"];
    let title = format!(
        "GTM Prospect: {}",
        prospect_title(
            prospect["name"].as_str(),
            prospect["company"].as_str(),
            prospect["company_domain"].as_str()
        )
    );
    let mut sections = Vec::new();
    sections.push(section(
        "status",
        "Status and Gate",
        lines(&[
            ("fit_status", artifact["fit"]["status"].as_str()),
            ("fit_decision", artifact["fit"]["decision"].as_str()),
            ("draft_status", artifact["draft_status"].as_str()),
            ("draft_decision", artifact["draft_decision"].as_str()),
            ("no_draft_reason", artifact["no_draft_reason"].as_str()),
        ]),
        vec![],
    ));
    sections.push(section(
        "snapshot",
        "Person and Account Snapshot",
        lines(&[
            ("person", prospect["name"].as_str()),
            ("role", prospect["title"].as_str()),
            ("company", prospect["company"].as_str()),
            ("company_domain", prospect["company_domain"].as_str()),
            ("tier_or_segment", prospect["segment"].as_str()),
            ("persona", artifact["persona"].as_str()),
            (
                "relationship_path",
                prospect["attributes"]["relationship_path"].as_str(),
            ),
            ("current_role_caveat", current_role_caveat(prospect)),
        ]),
        vec![],
    ));
    sections.push(section(
        "why-this-person",
        "Why This Person",
        first_non_empty(&[
            prospect["trigger"].as_str(),
            prospect["background"].as_str(),
            Some("No concise source-backed reason was present in the artifact."),
        ]),
        collect_refs(
            &artifact["context"]["entries"],
            &["signals", "pains", "hooks"],
        ),
    ));
    sections.push(section(
        "icp-fit",
        "ICP Fit and Tier Rationale",
        list_fit(&artifact["fit"]),
        collect_refs(&artifact["context"]["entries"], &["fit-rules"]),
    ));
    sections.push(section(
        "proof-used",
        "Proof Used",
        list_proof(artifact),
        collect_refs(
            &artifact["context"]["entries"],
            &["claims", "signals", "positioning"],
        ),
    ));
    sections.push(section(
        "gaps-caveats",
        "Gaps and Caveats",
        list_gaps(artifact),
        collect_refs(&artifact["context"]["entries"], &["gaps"]),
    ));
    sections.push(section(
        "recommended-angle",
        "Recommended Angle",
        list_context_bodies(
            &artifact["context"]["entries"],
            &[
                "hooks",
                "pains",
                "ctas",
                "copy-patterns",
                "channel-policies",
            ],
            "No safe angle was available from routed context.",
        ),
        collect_refs(
            &artifact["context"]["entries"],
            &[
                "hooks",
                "pains",
                "ctas",
                "copy-patterns",
                "channel-policies",
            ],
        ),
    ));
    sections.push(section(
        "draft-next-action",
        "Draft or Next Action",
        if decision == "ready" {
            "Draft from the routed context, then run `mdp check-claims` before approval.".to_string()
        } else {
            "Do not draft usable outreach copy from this artifact. Resolve the gate, gaps, or missing proof first.".to_string()
        },
        vec![],
    ));
    sections.push(audit_section(source_artifact, artifact, "gtm-prospect"));
    Ok(human_brief(
        &manifest,
        source_artifact_type(artifact),
        "gtm-prospect",
        decision,
        &title,
        sections,
        source_artifact,
        warnings,
    ))
}

fn render_proposal_review(
    root: &Path,
    artifact: &Value,
    source_artifact: &str,
    strict: bool,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut warnings = Vec::new();
    require_contract(artifact, "mdp.proof-output.v0", &mut warnings);
    require_field(artifact, "output.text", &mut warnings);
    require_field(artifact, "segments", &mut warnings);
    fail_on_strict_warnings(strict, &warnings)?;
    let verification = verify_output_value(root, artifact, source_artifact)?;
    let decision = if verification["valid"].as_bool() == Some(true) {
        "ready"
    } else if has_issue(&verification, "proof_output_insufficient_binding") {
        "proof-gap"
    } else {
        "blocked"
    };
    let title = format!(
        "Proposal Review: {}",
        artifact["route"]["job"]
            .as_str()
            .filter(|job| !job.trim().is_empty())
            .unwrap_or_else(|| artifact["output"]["kind"]
                .as_str()
                .unwrap_or("proof output"))
    );
    let mut sections = Vec::new();
    sections.push(section(
        "opportunity-section",
        "Opportunity and Section",
        lines(&[
            ("pack", artifact["pack"]["id"].as_str()),
            ("profile", artifact["pack"]["profile_id"].as_str()),
            ("persona", artifact["route"]["persona"].as_str()),
            ("job", artifact["route"]["job"].as_str()),
            ("output_kind", artifact["output"]["kind"].as_str()),
            ("output_format", artifact["output"]["format"].as_str()),
        ]),
        vec![],
    ));
    sections.push(section(
        "decision-gate",
        "Decision and Gate",
        lines(&[
            ("valid", verification["valid"].as_bool().map(bool_label)),
            ("decision", verification["decision"].as_str()),
            ("brief_decision", Some(decision)),
        ]),
        vec![],
    ));
    sections.push(section(
        "requirement-summary",
        "Requirement Summary",
        segment_text(
            artifact,
            &["requirement_status"],
            "No requirement summary segment was present.",
        ),
        segment_refs(artifact, &["requirement_status"]),
    ));
    sections.push(section(
        "approved-claims",
        "Approved Claims Available",
        segment_text(
            artifact,
            &["claim"],
            "No approved claim segments were present.",
        ),
        segment_refs(artifact, &["claim"]),
    ));
    sections.push(section(
        "evidence-used",
        "Evidence Used",
        evidence_summary(artifact),
        all_segment_refs(artifact),
    ));
    sections.push(section(
        "draft-safe-language",
        "Draft-Safe Language",
        if verification["valid"].as_bool() == Some(true) {
            quote_block(artifact["output"]["text"].as_str().unwrap_or(""))
        } else {
            format!(
                "Generated text is not approved for reuse. It is shown only as untrusted review input.\n\n{}",
                quote_block(artifact["output"]["text"].as_str().unwrap_or(""))
            )
        },
        vec![],
    ));
    sections.push(section(
        "blocked-missing",
        "Blocked or Missing Material",
        issues_and_gaps(&verification, artifact),
        vec![],
    ));
    sections.push(section(
        "reviewer-questions",
        "Reviewer Questions",
        reviewer_questions(&verification, artifact),
        vec![],
    ));
    sections.push(audit_section(source_artifact, artifact, "proposal-review"));
    Ok(human_brief(
        &manifest,
        source_artifact_type(artifact),
        "proposal-review",
        decision,
        &title,
        sections,
        source_artifact,
        warnings,
    ))
}

fn render_proof_report(
    root: &Path,
    artifact: &Value,
    source_artifact: &str,
    strict: bool,
) -> Result<Value> {
    let mut brief = render_proposal_review(root, artifact, source_artifact, strict)?;
    brief["template_id"] = json!("proof-report");
    brief["title"] = json!("Proof Report");
    Ok(brief)
}

fn human_brief(
    manifest: &crate::models::Manifest,
    source_artifact_type: String,
    template: &str,
    decision: &str,
    title: &str,
    sections: Vec<Value>,
    source_artifact: &str,
    warnings: Vec<String>,
) -> Value {
    json!({
        "artifact_type": HUMAN_BRIEF_CONTRACT,
        "pack_id": manifest.id,
        "pack_version": manifest.version,
        "source_artifact_type": source_artifact_type,
        "template_id": template,
        "decision": decision,
        "title": title,
        "sections": sections,
        "audit": {
            "source_artifact": source_artifact,
            "mdp_commands": [
                format!("mdp render-brief --dir <pack-dir> --file {source_artifact} --template {template}")
            ],
            "warnings": warnings
        }
    })
}

fn section(id: &str, title: &str, body: String, refs: Vec<String>) -> Value {
    json!({"id": id, "title": title, "body": body, "refs": refs})
}

fn audit_section(source_artifact: &str, artifact: &Value, template: &str) -> Value {
    section(
        "audit-trail",
        "Audit Trail",
        lines(&[
            ("source_artifact", Some(source_artifact)),
            (
                "source_artifact_type",
                Some(&source_artifact_type(artifact)),
            ),
            ("template_id", Some(template)),
        ]),
        vec![source_artifact.to_string()],
    )
}

fn require_contract(artifact: &Value, expected: &str, warnings: &mut Vec<String>) {
    let found = source_artifact_type(artifact);
    if found != expected {
        warnings.push(format!(
            "expected source artifact `{expected}`, found `{found}`"
        ));
    }
}

fn require_field(artifact: &Value, dotted_path: &str, warnings: &mut Vec<String>) {
    if read_path(artifact, dotted_path).is_none_or(is_empty_value) {
        warnings.push(format!("missing required field `{dotted_path}`"));
    }
}

fn fail_on_strict_warnings(strict: bool, warnings: &[String]) -> Result<()> {
    if strict && !warnings.is_empty() {
        bail!("render-brief strict mode failed: {}", warnings.join("; "));
    }
    Ok(())
}

fn read_path<'a>(value: &'a Value, dotted_path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in dotted_path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn is_empty_value(value: &Value) -> bool {
    value.is_null()
        || value.as_str().is_some_and(|text| text.trim().is_empty())
        || value.as_array().is_some_and(Vec::is_empty)
}

fn source_artifact_type(artifact: &Value) -> String {
    artifact["artifact_type"]
        .as_str()
        .or_else(|| artifact["contract"].as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn prospect_title(name: Option<&str>, company: Option<&str>, domain: Option<&str>) -> String {
    match (clean(name), clean(company), clean(domain)) {
        (Some(name), Some(company), _) => format!("{name} at {company}"),
        (Some(name), None, _) => name.to_string(),
        (None, Some(company), _) => company.to_string(),
        (None, None, Some(domain)) => domain.to_string(),
        _ => "Unknown prospect".to_string(),
    }
}

fn current_role_caveat(prospect: &Value) -> Option<&str> {
    prospect["attributes"]["current_role_caveat"].as_str().or_else(|| {
        if prospect["title"].as_str().is_none_or(|title| title.trim().is_empty()) {
            Some("No reviewed current role/title was supplied.")
        } else {
            Some("Current role is based on supplied prospect data and has not been independently verified.")
        }
    })
}

fn lines(items: &[(&str, Option<&str>)]) -> String {
    let mut out = String::new();
    for (label, value) in items {
        if let Some(value) = clean(*value) {
            writeln!(out, "- {label}: {value}").ok();
        }
    }
    if out.is_empty() {
        "- unknown".to_string()
    } else {
        out
    }
}

fn first_non_empty(items: &[Option<&str>]) -> String {
    items
        .iter()
        .find_map(|value| clean(*value))
        .unwrap_or("unknown")
        .to_string()
}

fn list_fit(fit: &Value) -> String {
    let mut out = lines(&[
        ("status", fit["status"].as_str()),
        ("decision", fit["decision"].as_str()),
    ]);
    out.push_str(&list_items(
        "matches",
        &fit["matches"],
        "No accepted fit signals were present.",
    ));
    out.push_str(&list_items(
        "disqualifiers",
        &fit["disqualifiers"],
        "No disqualifiers were present.",
    ));
    out
}

fn list_proof(artifact: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(signals) = artifact["prospect"]["signals"].as_array() {
        for signal in signals {
            parts.push(format!(
                "- {}",
                compact_item(signal, &["title", "source", "confidence", "state_as"])
            ));
        }
    }
    let context = list_context_bodies(
        &artifact["context"]["entries"],
        &["claims", "signals", "positioning"],
        "",
    );
    if !context.trim().is_empty() {
        parts.push(context);
    }
    if parts.is_empty() {
        "No source/proof refs were present in the artifact.".to_string()
    } else {
        parts.join("\n")
    }
}

fn list_gaps(artifact: &Value) -> String {
    let mut out = String::new();
    out.push_str(&list_items(
        "missing_requirements",
        &artifact["fit"]["context"]["missing_requirements"],
        "",
    ));
    out.push_str(&list_items(
        "invalid_requirements",
        &artifact["fit"]["context"]["invalid_requirements"],
        "",
    ));
    let gaps = list_context_bodies(&artifact["context"]["entries"], &["gaps"], "");
    out.push_str(&gaps);
    if out.trim().is_empty() {
        "No explicit gaps or caveats were present in the artifact.".to_string()
    } else {
        out
    }
}

fn list_items(label: &str, value: &Value, fallback: &str) -> String {
    let Some(items) = value.as_array() else {
        return fallback.to_string();
    };
    if items.is_empty() {
        return fallback.to_string();
    }
    let mut out = String::new();
    writeln!(out, "{label}:").ok();
    for item in items {
        writeln!(
            out,
            "- {}",
            compact_item(item, &["title", "field", "reason", "path"])
        )
        .ok();
    }
    out
}

fn list_context_bodies(entries: &Value, kinds: &[&str], fallback: &str) -> String {
    let mut out = String::new();
    for entry in entries.as_array().into_iter().flatten() {
        if entry["card_kind"]
            .as_str()
            .is_some_and(|kind| kinds.contains(&kind))
        {
            writeln!(
                out,
                "- {}: {}",
                display_value(&entry["title"]),
                display_value(&entry["body"])
            )
            .ok();
        }
    }
    if out.trim().is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

fn compact_item(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|key| {
            clean(value[*key].as_str()).map(|text| {
                if *key == "title" || *key == "reason" {
                    text.to_string()
                } else {
                    format!("{key}: {text}")
                }
            })
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn collect_refs(entries: &Value, kinds: &[&str]) -> Vec<String> {
    entries
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry["card_kind"]
                .as_str()
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .filter_map(|entry| {
            let card_id = entry["card_id"].as_str()?;
            let entry_id = entry["entry_id"].as_str()?;
            Some(format!("{card_id}#{entry_id}"))
        })
        .collect()
}

fn segment_text(artifact: &Value, kinds: &[&str], fallback: &str) -> String {
    let mut out = String::new();
    for segment in artifact["segments"].as_array().into_iter().flatten() {
        if segment["kind"]
            .as_str()
            .is_some_and(|kind| kinds.contains(&kind))
        {
            writeln!(out, "- {}", quote_inline(display_value(&segment["text"]))).ok();
        }
    }
    if out.trim().is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

fn segment_refs(artifact: &Value, kinds: &[&str]) -> Vec<String> {
    artifact["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|segment| {
            segment["kind"]
                .as_str()
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .flat_map(refs_from_segment)
        .collect()
}

fn all_segment_refs(artifact: &Value) -> Vec<String> {
    artifact["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(refs_from_segment)
        .collect()
}

fn refs_from_segment(segment: &Value) -> Vec<String> {
    segment["refs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(
            |reference| match reference["type"].as_str().unwrap_or("ref") {
                "card_entry" => format!(
                    "card_entry:{}#{}",
                    display_value(&reference["card_id"]),
                    display_value(&reference["entry_id"])
                ),
                "source" => format!("source:{}", display_value(&reference["source_id"])),
                "prompt_input" => format!(
                    "prompt_input:{}:{}",
                    display_value(&reference["prompt_id"]),
                    display_value(&reference["input_name"])
                ),
                "input_contract" => {
                    format!(
                        "input_contract:{}",
                        display_value(&reference["input_contract_id"])
                    )
                }
                "route" => format!(
                    "route:{}:{}",
                    display_value(&reference["persona"]),
                    display_value(&reference["job"])
                ),
                other => other.to_string(),
            },
        )
        .collect()
}

fn evidence_summary(artifact: &Value) -> String {
    let refs = all_segment_refs(artifact);
    if refs.is_empty() {
        "No refs were present on proof-output segments.".to_string()
    } else {
        refs.iter()
            .map(|reference| format!("- {reference}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn issues_and_gaps(verification: &Value, artifact: &Value) -> String {
    let mut out = String::new();
    for issue in verification["issues"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "- {}: {}",
            display_value(&issue["code"]),
            display_value(&issue["message"])
        )
        .ok();
    }
    let gaps = segment_text(artifact, &["gap"], "");
    if !gaps.trim().is_empty() {
        out.push_str(&gaps);
    }
    if out.trim().is_empty() {
        "No blocked or missing material was reported.".to_string()
    } else {
        out
    }
}

fn reviewer_questions(verification: &Value, artifact: &Value) -> String {
    if verification["valid"].as_bool() == Some(true) {
        return "- Does the reviewer approve adapting the quoted text as-is?\n- Are any requirement IDs or evidence refs missing from the source artifact?".to_string();
    }
    let gaps = segment_text(artifact, &["gap"], "");
    if gaps.trim().is_empty() {
        "- Which missing proof, source document, or SME input resolves the failed verification gate?"
            .to_string()
    } else {
        format!("- Resolve these proof gaps before reuse:\n{gaps}")
    }
}

fn has_issue(verification: &Value, code: &str) -> bool {
    verification["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|issue| issue["code"].as_str() == Some(code))
}

fn bool_label(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn quote_block(text: &str) -> String {
    if text.trim().is_empty() {
        return "> unknown".to_string();
    }
    text.lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn quote_inline(text: String) -> String {
    text.replace('\n', " / ")
}

fn clean(value: Option<&str>) -> Option<&str> {
    let text = value?.trim();
    if text.is_empty()
        || matches!(
            text.to_ascii_lowercase().as_str(),
            "unknown" | "n/a" | "na" | "none" | "null"
        )
    {
        None
    } else {
        Some(text)
    }
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(text) if clean(Some(text)).is_some() => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Array(items) if !items.is_empty() => items
            .iter()
            .map(display_value)
            .collect::<Vec<_>>()
            .join(", "),
        _ => "unknown".to_string(),
    }
}

fn frontmatter(out: &mut String, key: &str, value: Option<&str>) {
    writeln!(out, "{key}: {}", yaml_scalar(value.unwrap_or("unknown"))).ok();
}

fn yaml_scalar(value: &str) -> String {
    if clean(Some(value)).is_none() {
        return "unknown".to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::briefs::prospect_brief_with_context;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(template: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-human-brief-{template}-{nonce}"));
        init_pack(&root, "Human Brief Pack", template, true, false)
            .expect("starter pack should initialize");
        root
    }

    #[test]
    fn gtm_template_renders_structured_ready_brief() {
        let root = temp_pack("gtm");
        let prospect = root.join("examples").join("clay-row.json");
        let artifact = prospect_brief_with_context(&root, &prospect, "linkedin", None, true)
            .expect("brief should render");

        let brief = render_human_brief(&root, &artifact, "inline", "gtm-prospect", true)
            .expect("human brief should render");
        let markdown = render_human_brief_markdown(&brief);

        assert_eq!(brief["artifact_type"], HUMAN_BRIEF_CONTRACT);
        assert_eq!(brief["template_id"], "gtm-prospect");
        assert_eq!(brief["decision"], "ready");
        assert!(markdown.contains("## Status and Gate"));
        assert!(markdown.contains("- draft_status: ready"));
        assert!(markdown.contains("## Proof Used"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gtm_template_preserves_no_draft_gate() {
        let root = temp_pack("gtm");
        let prospect = root.join("examples").join("thin.json");
        std::fs::write(
            &prospect,
            r#"{"name":"Taylor Lee","title":"GTM Lead","company":"ExampleCo"}"#,
        )
        .expect("prospect should write");
        let artifact = prospect_brief_with_context(&root, &prospect, "linkedin", None, true)
            .expect("brief should render");

        let brief = render_human_brief(&root, &artifact, "inline", "gtm-prospect", true)
            .expect("human brief should render");
        let markdown = render_human_brief_markdown(&brief);

        assert_eq!(brief["decision"], "no-draft");
        assert!(markdown.contains("- draft_status: no-draft"));
        assert!(markdown.contains("Do not draft usable outreach copy"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proposal_template_marks_unbound_claim_as_proof_gap() {
        let root = temp_pack("proposal");
        let artifact = json!({
            "contract": "mdp.proof-output.v0",
            "pack": {"id": "proposal-mdp-sample", "profile_id": "proposal"},
            "route": {"persona": "Proposal Lead", "job": "compliance review"},
            "output": {"kind": "proposal-review-section", "format": "markdown", "text": "We are fully certified."},
            "coverage": {"mode": "full-segmentation", "material_policy": "bound-or-gap"},
            "segments": [
                {"id": "seg-001", "kind": "claim", "text": "We are fully certified.", "refs": []}
            ]
        });

        let brief = render_human_brief(&root, &artifact, "inline", "proposal-review", true)
            .expect("human brief should render");
        let markdown = render_human_brief_markdown(&brief);

        assert_eq!(brief["decision"], "proof-gap");
        assert!(markdown.contains("Generated text is not approved for reuse"));
        assert!(markdown.contains("proof_output_insufficient_binding"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn strict_mode_rejects_missing_required_gate_fields() {
        let root = temp_pack("gtm");
        let err = render_human_brief(
            &root,
            &json!({"contract": "mdp.message-brief.v0"}),
            "inline",
            "gtm-prospect",
            true,
        )
        .expect_err("strict mode should fail");

        assert!(
            err.to_string()
                .contains("missing required field `draft_status`")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
