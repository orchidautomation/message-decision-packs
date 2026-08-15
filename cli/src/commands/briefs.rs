use crate::models::{Manifest, Prospect};
use crate::pack_io::{read_manifest, read_prospect};
use crate::routing::{entry_context_with_runtime_scoped, select_cards};
use crate::runtime_context::current_runtime_context;
use crate::scope::{parse_scope_selectors, resolve_runtime_scope, scope_from_prospect};
use crate::utils::{resolve_persona, resolve_persona_label, routable_persona};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

#[cfg(test)]
pub(crate) fn emit_brief(
    root: &Path,
    persona: &str,
    motion: Option<&str>,
    job: Option<&str>,
) -> Result<Value> {
    emit_brief_scoped(root, persona, motion, job, &[])
}

pub(crate) fn emit_brief_scoped(
    root: &Path,
    persona: &str,
    motion: Option<&str>,
    job: Option<&str>,
    scope_selectors: &[String],
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let runtime_context = current_runtime_context()?;
    let job_text = brief_job(&manifest, job, "unspecified GTM decision task");
    let persona_resolution = resolve_persona_label(&manifest, persona);
    let resolved_persona = routable_persona(persona, &persona_resolution);
    let scope = resolve_runtime_scope(&manifest, parse_scope_selectors(scope_selectors)?);
    let selected = select_cards(&manifest, Some(resolved_persona), Some(&job_text));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    let context = entry_context_with_runtime_scoped(
        root,
        &manifest,
        resolved_persona,
        &job_text,
        true,
        &runtime_context,
        &scope,
    )?;
    let portfolio_sensitive = context["portfolio_sensitive"].as_bool().unwrap_or(false);
    Ok(json!({
        "contract": "mdp.brief.v0",
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "runtime_context": runtime_context.clone(),
        "persona": resolved_persona,
        "requested_persona": persona,
        "persona_resolution": persona_resolution,
        "scope": scope,
        "portfolio_sensitive": portfolio_sensitive,
        "draft_status": context["status"],
        "inputs": {"persona": resolved_persona, "requested_persona": persona, "motion": motion, "job": job_text},
        "required_load_order": if portfolio_sensitive { Vec::<String>::new() } else { load_order },
        "product_foundation": context["product_foundation"].clone(),
        "product_foundation_load_order": context["product_foundation_load_order"].clone(),
        "profile_activation": context["profile_activation"].clone(),
        "context": context,
        "decision_trace": [
            {"step": "load_manifest", "reason": "discover pack metadata and card index"},
            {"step": "resolve_persona", "reason": "map aliases through pack-owned persona mappings when available"},
            {"step": "route_cards", "reason": "preserve progressive disclosure"},
            {"step": "apply_avoid_rules", "reason": "prevent category drift and unsupported claims"},
            {"step": "apply_output_rules", "reason": "preserve global style and output-structure constraints"},
            {"step": "draft_or_decide", "reason": "use only loaded card evidence and cite gaps"}
        ],
        "output_requirements": {"state_assumptions": true, "cite_loaded_cards": true, "surface_gaps": true, "avoid_execution_claims": true, "use_loaded_cta_policy": true, "use_loaded_output_rules": true}
    }))
}

pub(crate) fn prospect_brief(
    root: &Path,
    prospect_path: &Path,
    channel: &str,
    job: Option<&str>,
) -> Result<Value> {
    prospect_brief_with_context(root, prospect_path, channel, job, false)
}

pub(crate) fn prospect_brief_with_context(
    root: &Path,
    prospect_path: &Path,
    channel: &str,
    job: Option<&str>,
    include_context: bool,
) -> Result<Value> {
    let prospect = read_prospect(prospect_path)?;
    prospect_brief_from_value_with_context(root, prospect, channel, job, include_context)
}

pub(crate) fn prospect_brief_from_value(
    root: &Path,
    prospect: Prospect,
    channel: &str,
    job: Option<&str>,
) -> Result<Value> {
    prospect_brief_from_value_with_context(root, prospect, channel, job, false)
}

pub(crate) fn prospect_brief_from_value_with_context(
    root: &Path,
    prospect: Prospect,
    channel: &str,
    job: Option<&str>,
    include_context: bool,
) -> Result<Value> {
    let fit_result = crate::commands::routing::fit_prospect_for_job(root, prospect.clone(), job)?;
    prospect_brief_from_fit_with_context(root, prospect, fit_result, channel, job, include_context)
}

pub(crate) fn prospect_brief_from_fit_with_context(
    root: &Path,
    prospect: Prospect,
    fit_result: Value,
    channel: &str,
    job: Option<&str>,
    include_context: bool,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let runtime_context = current_runtime_context()?;
    let persona_resolution = resolve_persona(&manifest, &prospect);
    let scope = scope_from_prospect(&manifest, &prospect);
    let fit_status = fit_result["status"]
        .as_str()
        .unwrap_or("insufficient-context");
    let persona = persona_resolution.persona.clone();
    let prospect_source_kind = prospect
        .source_kind
        .clone()
        .unwrap_or_else(|| "prospect-json".to_string());
    let prospect_is_synthetic = prospect.synthetic;
    let job_text = brief_job(&manifest, job, &format!("write {channel} outbound message"));
    let routing_task = brief_routing_task(job, &job_text, channel);
    let route = select_cards(&manifest, Some(&persona), Some(&routing_task));
    let load_order: Vec<String> = route
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    let fit_draft_ready = fit_status == "fit";
    let initial_draft_status = if fit_draft_ready { "ready" } else { "no-draft" };
    let mut context = entry_context_with_runtime_scoped(
        root,
        &manifest,
        &persona,
        &routing_task,
        fit_draft_ready,
        &runtime_context,
        &scope,
    )?;
    if routing_task != job_text {
        let foundation_context = entry_context_with_runtime_scoped(
            root,
            &manifest,
            &persona,
            &job_text,
            fit_draft_ready,
            &runtime_context,
            &scope,
        )?;
        context["product_foundation"] = foundation_context["product_foundation"].clone();
        context["product_foundation_load_order"] =
            foundation_context["product_foundation_load_order"].clone();
        if foundation_context["product_foundation"]["status"] == "blocked"
            || foundation_context["reason"] == "pack validation failed for this job"
        {
            context["status"] = json!("blocked");
            context["reason"] = foundation_context["reason"].clone();
            context["entries"] = json!([]);
            context["full_card_required"] = json!([]);
            context["summary"]["entry_count"] = json!(0);
            context["summary"]["required_entry_count"] = json!(0);
            context["summary"]["supporting_entry_count"] = json!(0);
            context["summary"]["guardrail_entry_count"] = json!(0);
        }
    }
    let portfolio_sensitive = context["portfolio_sensitive"].as_bool().unwrap_or(false);
    let bounded_context = include_context || portfolio_sensitive;
    let draft_status = if initial_draft_status == "ready" && context["status"] == "ready" {
        "ready"
    } else {
        "no-draft"
    };
    let no_draft_reason = if draft_status == "ready" {
        Value::Null
    } else if fit_status != "fit" {
        json!(brief_no_draft_reason(&fit_result))
    } else {
        json!(
            context["reason"]
                .as_str()
                .unwrap_or("Portfolio scope did not resolve to draft-safe bounded context.")
        )
    };
    let valid = fit_result["valid"].as_bool().unwrap_or(true);
    let mut payload = json!({
        "contract": "mdp.message-brief.v0",
        "valid": valid,
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "runtime_context": runtime_context.clone(),
        "channel": channel,
        "prospect": prospect,
        "prospect_source": {
            "kind": prospect_source_kind,
            "synthetic": prospect_is_synthetic,
            "guidance": if prospect_is_synthetic { "Synthetic example fixture. Replace with a real or intentionally sanitized prospect row before production use." } else { "User-provided or sanitized prospect row." }
        },
        "persona": persona,
        "persona_resolution": persona_resolution,
        "scope": scope,
        "portfolio_sensitive": portfolio_sensitive,
        "fit": fit_result,
        "draft_status": draft_status,
        "draft_decision": if draft_status == "ready" { "Proceed with routed brief using stated assumptions." } else { "Do not draft outbound copy unless the user explicitly overrides this fit gate." },
        "no_draft_reason": no_draft_reason,
        "job": job_text,
        "required_load_order": if portfolio_sensitive { Vec::<String>::new() } else { load_order },
        "product_foundation": context["product_foundation"].clone(),
        "product_foundation_load_order": context["product_foundation_load_order"].clone(),
        "profile_activation": context["profile_activation"].clone(),
        "route": route,
        "decision_trace": [
            {"step": "read_prospect", "reason": "use supplied prospect/account JSON as task input"},
            {"step": "infer_or_use_persona", "reason": "map person title to pack persona"},
            {"step": "route_cards", "reason": "load only relevant message decision cards"},
            {"step": "generate_or_handoff", "reason": "use the brief as the agent/model input contract"}
        ],
        "agent_instruction": if draft_status == "ready" {
            if bounded_context {
                "Use data.context.entries before opening card files. Open full_card_required paths only when present. Combine bounded context with prospect, use the routed CTA and output rules when present, and do not invent claims outside the loaded context."
            } else {
                "Read only required_load_order card files, combine them with prospect, then draft copy. Use the routed CTA policy and output rules when present. Do not invent claims outside the loaded cards."
            }
        } else { "Stop before drafting. Surface the fit status and missing context/disqualifiers, then ask for explicit user override before creating outbound copy." }
    });
    if bounded_context {
        payload["context"] = context;
    }
    Ok(payload)
}

fn brief_job(manifest: &Manifest, explicit_job: Option<&str>, legacy_default: &str) -> String {
    explicit_job
        .map(str::to_string)
        .or_else(|| {
            manifest
                .jobs
                .iter()
                .any(|job| job.id == "outbound-copy-brief")
                .then(|| "outbound-copy-brief".to_string())
        })
        .unwrap_or_else(|| legacy_default.to_string())
}

fn brief_routing_task(explicit_job: Option<&str>, foundation_job: &str, channel: &str) -> String {
    if explicit_job.is_some() {
        foundation_job.to_string()
    } else {
        format!("write {channel} outbound message")
    }
}

fn brief_no_draft_reason(fit_result: &Value) -> String {
    if fit_result["ingress"]["status"] == "blocked" {
        return "The selected job requires governed normalized input with exact lineage artifacts; detached prospect input is not qualification or draft authority.".to_string();
    }
    let status = fit_result["status"]
        .as_str()
        .unwrap_or("insufficient-context");
    if status == "disqualified" {
        return "The fit gate found a disqualifier; do not draft outbound copy unless the user explicitly overrides it.".to_string();
    }

    let missing = fit_result["context"]["missing_requirements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|requirement| requirement["field"].as_str())
        .collect::<BTreeSet<_>>();
    let invalid_count = fit_result["context"]["invalid_requirements"]
        .as_array()
        .map_or(0, Vec::len);

    if missing.contains("name") && missing.contains("title") {
        return "No person name or title was present in the prospect row; provide a reviewed contact before drafting.".to_string();
    }
    if missing.contains("name") {
        return "No person name was present in the prospect row; provide a reviewed contact before drafting.".to_string();
    }
    if missing.contains("title") {
        return "No person title was present in the prospect row; provide a reviewed title before drafting.".to_string();
    }
    if missing.contains("persona") {
        return "No pack-owned persona was available from the prospect row; provide a reviewed persona or title before drafting.".to_string();
    }
    if invalid_count > 0 {
        return "The prospect row contains invalid values for this pack; fix the invalid requirements before drafting.".to_string();
    }

    "The fit gate returned insufficient-context; review missing context before drafting."
        .to_string()
}

pub(crate) fn render_readable_prospect_brief(brief: &Value) -> String {
    let mut out = String::new();
    let prospect = &brief["prospect"];
    let fit = &brief["fit"];

    render_metadata_frontmatter(&mut out, brief);
    out.push_str("\n# Prospect Brief: ");
    out.push_str(&prospect_brief_title(brief));
    out.push_str("\n\n");

    out.push_str("## Fit / Draft Readiness\n\n");
    bullet(&mut out, "fit_status", display_value(&fit["status"]));
    bullet(&mut out, "fit_decision", display_value(&fit["decision"]));
    bullet(
        &mut out,
        "draft_status",
        display_value(&brief["draft_status"]),
    );
    bullet(
        &mut out,
        "draft_decision",
        display_value(&brief["draft_decision"]),
    );
    if !brief["no_draft_reason"].is_null() {
        bullet(
            &mut out,
            "no_draft_reason",
            display_value(&brief["no_draft_reason"]),
        );
    }
    if fit["ingress"]["status"].is_string() {
        bullet(
            &mut out,
            "ingress_status",
            display_value(&fit["ingress"]["status"]),
        );
        for diagnostic in fit["ingress"]["diagnostics"]
            .as_array()
            .into_iter()
            .flatten()
        {
            bullet(
                &mut out,
                "ingress_diagnostic",
                format!(
                    "{}: {}",
                    display_value(&diagnostic["code"]),
                    display_value(&diagnostic["message"])
                ),
            );
        }
    }
    out.push('\n');

    let minimality = &brief["context"]["minimality"];
    if minimality.is_object() {
        out.push_str("## Minimal Context Receipt\n\n");
        bullet(&mut out, "status", display_value(&minimality["status"]));
        bullet(
            &mut out,
            "context_sha256",
            display_value(&minimality["context_sha256"]),
        );
        bullet(
            &mut out,
            "selected_count",
            display_value(&minimality["selected_count"]),
        );
        bullet(
            &mut out,
            "excluded_count",
            display_value(&minimality["excluded_count"]),
        );
        for diagnostic in minimality["diagnostics"].as_array().into_iter().flatten() {
            bullet(&mut out, "diagnostic", display_value(diagnostic));
        }
        out.push('\n');
    }

    out.push_str("## Evidence Receipts and Signal Decisions\n\n");
    let mut wrote_evidence = false;
    let signal_authority = &fit["signal_authority"];
    bullet(
        &mut out,
        "signal_authority",
        display_value(&signal_authority["authority_class"]),
    );
    bullet(
        &mut out,
        "projection_status",
        display_value(&signal_authority["projection_status"]),
    );
    bullet(
        &mut out,
        "trust_boundary",
        display_value(&signal_authority["trust_boundary"]),
    );
    for field in [
        "source_binding_sha256",
        "source_attempt_request_sha256",
        "collected_attempt_results_sha256",
        "normalized_output_sha256",
    ] {
        if !signal_authority[field].is_null() {
            bullet(&mut out, field, display_value(&signal_authority[field]));
        }
    }
    out.push('\n');
    wrote_evidence |= list_named_items(
        &mut out,
        "Lineage-validated signal contributions",
        &signal_authority["accepted"],
        |item| {
            format!(
                "{}; roles: {}; observations: {}",
                display_value(&item["signal_id"]),
                item["roles"]
                    .as_array()
                    .map(|roles| roles
                        .iter()
                        .map(display_value)
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_else(|| "none".to_string()),
                render_observation_receipts(&item["observation_receipts"])
            )
        },
    );
    wrote_evidence |= list_named_items(
        &mut out,
        "Rejected signal contributions",
        &signal_authority["rejected"],
        |item| {
            format!(
                "{}; roles: {}; reason: {}",
                display_value(&item["signal_id"]),
                item["roles"]
                    .as_array()
                    .map(|roles| roles
                        .iter()
                        .map(display_value)
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_else(|| "none".to_string()),
                display_value(&item["reason"])
            )
        },
    );
    wrote_evidence |= list_named_items(&mut out, "Accepted fit signals", &fit["matches"], |item| {
        titled_body(item, "title", "reason")
    });
    if fit["signal_authority"]["authority_class"] != "lineage-validated" {
        wrote_evidence |= list_named_items(
            &mut out,
            "Legacy prospect signals (unassessed)",
            &prospect["signals"],
            |item| {
                let mut parts = vec![display_value(&item["id"]), display_value(&item["title"])];
                if let Some(confidence) = optional_text(&item["confidence"]) {
                    parts.push(format!("confidence: {confidence}"));
                }
                if let Some(state_as) = optional_text(&item["state_as"]) {
                    parts.push(format!("state_as: {state_as}"));
                }
                parts.join("; ")
            },
        );
    }
    wrote_evidence |= list_context_entries(
        &mut out,
        "Routed evidence entries",
        brief,
        &["claims", "signals", "positioning", "fit-rules"],
    );
    if !wrote_evidence {
        out.push_str("- No accepted evidence or supplied signals were present in the brief.\n");
    }
    out.push('\n');

    out.push_str("## Gaps, Hypotheses, and Current-Role Caveats\n\n");
    let mut wrote_gap = false;
    wrote_gap |= list_named_items(
        &mut out,
        "Missing requirements",
        &fit["context"]["missing_requirements"],
        |item| titled_body(item, "field", "reason"),
    );
    wrote_gap |= list_named_items(
        &mut out,
        "Invalid requirements",
        &fit["context"]["invalid_requirements"],
        |item| titled_body(item, "field", "reason"),
    );
    wrote_gap |= list_context_entries(&mut out, "Routed gap entries", brief, &["gaps"]);
    if let Some(caveat) = current_role_caveat(brief) {
        out.push_str("- Current-role caveat: ");
        out.push_str(&caveat);
        out.push('\n');
        wrote_gap = true;
    }
    if let Some(trigger) = optional_text(&prospect["trigger"]) {
        out.push_str("- Trigger should be treated as supplied context: ");
        out.push_str(&trigger);
        out.push('\n');
        wrote_gap = true;
    }
    if !wrote_gap {
        out.push_str("- No explicit gaps or caveats were present in the brief.\n");
    }
    out.push('\n');

    out.push_str("## Safe Angle\n\n");
    if let Some(trigger) = optional_text(&prospect["trigger"]) {
        out.push_str("- Anchor on supplied trigger: ");
        out.push_str(&trigger);
        out.push('\n');
    }
    if let Some(background) = optional_text(&prospect["background"]) {
        out.push_str("- Background context: ");
        out.push_str(&background);
        out.push('\n');
    }
    if !list_context_entries(
        &mut out,
        "Routed angle entries",
        brief,
        &[
            "hooks",
            "pains",
            "ctas",
            "copy-patterns",
            "channel-policies",
        ],
    ) && optional_text(&prospect["trigger"]).is_none()
        && optional_text(&prospect["background"]).is_none()
    {
        out.push_str("- No safe angle was available from routed context.\n");
    }
    out.push('\n');

    out.push_str("## Avoid-Claims / Guardrails\n\n");
    let wrote_guardrail = list_context_entries(
        &mut out,
        "Routed guardrails",
        brief,
        &["avoid-rules", "output-rules", "fit-rules", "positioning"],
    );
    if !wrote_guardrail {
        out.push_str("- No routed guardrails were present in the brief context.\n");
    }
    out.push('\n');

    out.push_str("## Proposed Outreach Copy\n\n");
    if !render_copy_blockquotes(&mut out, brief) {
        out.push_str("- No proposed outreach copy is included. Draft only after `draft_status: ready` and route/check constraints are reviewed.\n");
    }
    out.push('\n');

    out.push_str("## Discovery Questions / Follow-Up Research\n\n");
    if !list_context_entries(
        &mut out,
        "Routed follow-up entries",
        brief,
        &["gaps", "objections"],
    ) {
        out.push_str(
            "- Confirm the prospect's current role and account context before using draft copy.\n",
        );
        out.push_str(
            "- Add source-backed evidence for any claim not already present in the brief.\n",
        );
    }
    out.push('\n');

    out.push_str("## Validation Status and Source Outputs\n\n");
    bullet(
        &mut out,
        "brief_contract",
        display_value(&brief["contract"]),
    );
    bullet(
        &mut out,
        "context_contract",
        display_value(&brief["context"]["contract"]),
    );
    bullet(
        &mut out,
        "context_status",
        display_value(&brief["context"]["status"]),
    );
    bullet(
        &mut out,
        "source_kind",
        display_value(&brief["prospect_source"]["kind"]),
    );
    bullet(
        &mut out,
        "source_guidance",
        display_value(&brief["prospect_source"]["guidance"]),
    );
    bullet(
        &mut out,
        "input_artifact",
        display_value(&brief["input_artifact"]["path"]),
    );
    bullet(
        &mut out,
        "json_source",
        "Use `mdp --json brief --context` as the machine source of truth.",
    );

    out
}

fn readable_metadata(brief: &Value) -> Vec<(&'static str, String)> {
    let prospect = &brief["prospect"];
    let full_name = display_value(&prospect["name"]);
    let (first_name, last_name) = split_name(&full_name);
    vec![
        ("first_name", first_name),
        ("last_name", last_name),
        ("full_name", full_name),
        ("linkedin_url", display_value(&prospect["linkedin_url"])),
        ("title", display_value(&prospect["title"])),
        ("company_name", display_value(&prospect["company"])),
        ("company_domain", display_value(&prospect["company_domain"])),
        ("company_url", display_value(&prospect["company_url"])),
        ("persona", display_value(&brief["persona"])),
        ("segment", display_value(&prospect["segment"])),
        (
            "source_kind",
            display_value(&brief["prospect_source"]["kind"]),
        ),
        (
            "research_provider",
            prospect["attributes"]["research_provider"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
        ),
        (
            "current_role_caveat",
            current_role_caveat(brief).unwrap_or_else(|| "unknown".to_string()),
        ),
    ]
}

fn render_metadata_frontmatter(out: &mut String, brief: &Value) {
    out.push_str("---\n");
    for (key, value) in readable_metadata(brief) {
        out.push_str(key);
        out.push_str(": ");
        out.push_str(&yaml_scalar(&value));
        out.push('\n');
    }
    out.push_str("tags: [");
    out.push_str(&metadata_tags(brief).join(", "));
    out.push_str("]\n");
    out.push_str("---\n");
}

fn metadata_tags(brief: &Value) -> Vec<String> {
    ["persona", "segment", "source_kind"]
        .iter()
        .filter_map(|key| {
            readable_metadata(brief)
                .into_iter()
                .find(|(metadata_key, _)| metadata_key == key)
                .map(|(_, value)| value)
        })
        .filter(|value| !is_unknownish(value))
        .map(|value| yaml_scalar(&value))
        .collect()
}

fn prospect_brief_title(brief: &Value) -> String {
    let prospect = &brief["prospect"];
    let name = display_value(&prospect["name"]);
    let company = display_value(&prospect["company"]);
    match (is_unknownish(&name), is_unknownish(&company)) {
        (false, false) => format!("{name} at {company}"),
        (false, true) => name,
        (true, false) => company,
        (true, true) => "Unknown prospect".to_string(),
    }
}

fn split_name(full_name: &str) -> (String, String) {
    if is_unknownish(full_name) {
        return ("unknown".to_string(), "unknown".to_string());
    }
    let mut parts = full_name.split_whitespace().collect::<Vec<_>>();
    let first_name = parts.first().copied().unwrap_or("unknown").to_string();
    if !parts.is_empty() {
        parts.remove(0);
    }
    let last_name = if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(" ")
    };
    (first_name, last_name)
}

fn current_role_caveat(brief: &Value) -> Option<String> {
    let prospect = &brief["prospect"];
    if let Some(value) = optional_text(&prospect["attributes"]["current_role_caveat"]) {
        return Some(value);
    }
    let title = display_value(&prospect["title"]);
    if is_unknownish(&title) {
        return Some("No reviewed current role/title was supplied.".to_string());
    }
    Some(
        "Current role is based on supplied prospect data and has not been independently verified."
            .to_string(),
    )
}

fn list_named_items<F>(out: &mut String, heading: &str, value: &Value, render: F) -> bool
where
    F: Fn(&Value) -> String,
{
    let Some(items) = value.as_array() else {
        return false;
    };
    if items.is_empty() {
        return false;
    }
    out.push_str("**");
    out.push_str(heading);
    out.push_str("**\n\n");
    for item in items {
        out.push_str("- ");
        out.push_str(&render(item));
        out.push('\n');
    }
    true
}

fn list_context_entries(out: &mut String, heading: &str, brief: &Value, kinds: &[&str]) -> bool {
    let entries = brief["context"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let selected = entries
        .iter()
        .filter(|entry| {
            entry["card_kind"]
                .as_str()
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return false;
    }
    out.push_str("**");
    out.push_str(heading);
    out.push_str("**\n\n");
    for entry in selected {
        out.push_str("- ");
        out.push_str(&display_value(&entry["title"]));
        out.push_str(" (");
        out.push_str(&display_value(&entry["card_kind"]));
        out.push_str("): ");
        out.push_str(&display_value(&entry["body"]));
        if let Some(avoid) = entry["avoid"].as_array().filter(|items| !items.is_empty()) {
            let terms = avoid
                .iter()
                .map(display_value)
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(" Avoid: ");
            out.push_str(&terms);
            out.push('.');
        }
        out.push('\n');
    }
    true
}

fn render_copy_blockquotes(out: &mut String, brief: &Value) -> bool {
    let mut wrote = false;
    for key in [
        "recommended",
        "message",
        "copy",
        "email",
        "linkedin_message",
    ] {
        if let Some(text) = optional_text(&brief[key]) {
            out.push_str("**");
            out.push_str(key);
            out.push_str("**\n\n");
            for line in text.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            wrote = true;
        }
    }
    wrote
}

fn titled_body(item: &Value, title_key: &str, body_key: &str) -> String {
    let title = display_value(&item[title_key]);
    let body = display_value(&item[body_key]);
    if is_unknownish(&body) {
        title
    } else {
        format!("{title}: {body}")
    }
}

fn bullet(out: &mut String, label: &str, value: impl AsRef<str>) {
    out.push_str("- ");
    out.push_str(label);
    out.push_str(": ");
    out.push_str(value.as_ref());
    out.push('\n');
}

fn render_observation_receipts(receipts: &Value) -> String {
    let Some(receipts) = receipts.as_array() else {
        return "none".to_string();
    };
    if receipts.is_empty() {
        return "none".to_string();
    }
    receipts
        .iter()
        .map(|receipt| {
            format!(
                "{}@{} confidence={} attempts={}",
                display_value(&receipt["id"]),
                display_value(&receipt["observed_at"]),
                display_value(&receipt["confidence"]),
                display_value(&receipt["attempt_ids"])
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(text) if !is_unknownish(text) => text.clone(),
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

fn optional_text(value: &Value) -> Option<String> {
    let text = display_value(value);
    (!is_unknownish(&text)).then_some(text)
}

fn is_unknownish(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty()
        || matches!(
            trimmed.to_ascii_lowercase().as_str(),
            "unknown" | "n/a" | "na" | "none" | "null"
        )
}

fn yaml_scalar(value: &str) -> String {
    if is_unknownish(value) {
        return "unknown".to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub(crate) fn demo_copy(root: &Path, prospect_path: &Path, channel: &str) -> Result<Value> {
    let brief = prospect_brief(
        root,
        prospect_path,
        channel,
        Some("write linkedin outbound copy"),
    )?;
    let prospect: Prospect = serde_json::from_value(brief["prospect"].clone())?;
    if brief["draft_status"] != "ready" {
        return Ok(json!({
            "contract": "mdp.copy-demo.v0",
            "channel": channel,
            "draft_status": "no-draft",
            "fit": brief["fit"].clone(),
            "decision": "Demo copy was not generated because the fit gate did not pass."
        }));
    }
    let persona = brief["persona"].as_str().unwrap_or("finance leader");
    let trigger = prospect
        .trigger
        .as_deref()
        .unwrap_or("scaling finance operations");
    let background = prospect
        .background
        .as_deref()
        .unwrap_or("working on finance systems");
    let first_name = prospect
        .name
        .split_whitespace()
        .next()
        .unwrap_or(&prospect.name);

    let (recommended, shorter, proof_led) = (
        format!(
            "Hey {first_name} - saw you're {background}. If {company} is {trigger}, a Message Decision Pack can keep persona, pain, hooks, CTA rules, and avoid-rules consistent across agents. Worth comparing notes?",
            company = prospect.company
        ),
        format!(
            "Hey {first_name} - noticed {company} is {trigger}. MDP helps teams version their GTM message context so agents draft from the same approved source. Open to a quick compare?",
            company = prospect.company
        ),
        format!(
            "Hey {first_name} - given your {title} role, thought a lightweight message decision layer could be relevant for keeping agent-generated GTM copy consistent.",
            title = prospect.title
        ),
    );

    Ok(json!({
        "contract": "mdp.copy-demo.v0",
        "channel": channel,
        "persona": persona,
        "prospect": {
            "name": prospect.name,
            "title": prospect.title,
            "company": prospect.company,
            "linkedin_url": prospect.linkedin_url
        },
        "recommended": recommended,
        "alternates": [shorter, proof_led],
        "decision_trace": brief["decision_trace"].clone(),
        "cards_used": brief["required_load_order"].clone(),
        "notes": [
            "Deterministic demo copy only; production should pass the brief to a model.",
            "No LinkedIn, Clay, CRM, or sequencer write was performed."
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        root
    }

    fn govern_prospect_input(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["decision_input_contracts"] = serde_yaml::from_str(
            r#"
- id: governed-prospect
  version: v1
  normalization:
    prompt: normalize-prospect-row
    prompt_version: v1
    normalized_schema_ref: mdp.normalized-decision-input.v1
  source_classes: [user_provided]
  attributes: []
"#,
        )
        .expect("decision input contract should parse");
        manifest["input_contracts"][0]["decision_input_contracts"] =
            serde_yaml::from_str("[governed-prospect]")
                .expect("input contract binding should parse");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn add_selected_foundation_gap(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["gaps"] =
            serde_yaml::from_str("- card_id: gaps\n  entry_id: missing-company-proof\n")
                .expect("gap reference should parse");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn set_profile_activation(root: &Path, status: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile_eval"]["activation"]["status"] =
            serde_yaml::Value::String(status.to_string());
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn set_foundation_facet_kind(root: &Path, kind: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["kind"] =
            serde_yaml::Value::String(kind.to_string());
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn remove_canonical_brief_job(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        let jobs = manifest["jobs"]
            .as_sequence_mut()
            .expect("jobs should be a sequence");
        jobs.retain(|job| job["id"].as_str() != Some("outbound-copy-brief"));
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    #[test]
    fn brief_marks_no_draft_when_fit_is_insufficient() {
        let root = temp_pack("brief-no-draft");
        let prospect_path = root.join("examples").join("thin.json");
        std::fs::write(
            &prospect_path,
            r#"{"name":"Taylor Lee","title":"GTM Engineering Lead","company":"ExampleCo"}"#,
        )
        .expect("prospect should be writable");

        let result =
            prospect_brief(&root, &prospect_path, "linkedin", None).expect("brief should succeed");

        assert_eq!(result["fit"]["status"], "insufficient-context");
        assert_eq!(result["draft_status"], "no-draft");
        assert_eq!(result["job"], "outbound-copy-brief");
        assert_eq!(result["product_foundation"]["status"], "ready");
        assert!(
            !result["product_foundation_load_order"]
                .as_array()
                .expect("foundation load order")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn governed_job_detached_brief_cannot_become_ready() {
        let root = temp_pack("governed-detached-brief");
        govern_prospect_input(&root);
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = prospect_brief_with_context(
            &root,
            &prospect_path,
            "linkedin",
            Some("prospect-fit-or-brief"),
            true,
        )
        .expect("governed detached brief should return a deterministic result");

        assert_eq!(result["fit"]["status"], "insufficient-context");
        assert_eq!(result["draft_status"], "no-draft");
        assert_eq!(
            result["fit"]["ingress"]["diagnostics"][0]["code"],
            "governed_job_requires_normalized_input"
        );
        assert!(
            result["agent_instruction"]
                .as_str()
                .expect("agent instruction")
                .starts_with("Stop before drafting")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn default_brief_workflows_use_canonical_outbound_job() {
        let root = temp_pack("canonical-default-brief-job");
        let prospect_path = root.join("examples").join("clay-row.json");

        let prospect = prospect_brief(&root, &prospect_path, "email", None)
            .expect("prospect brief should resolve canonical default");
        let emitted = emit_brief(&root, "PMM", None, None)
            .expect("emit brief should resolve canonical default");

        assert_eq!(prospect["job"], "outbound-copy-brief");
        assert_eq!(emitted["inputs"]["job"], "outbound-copy-brief");
        assert_ne!(prospect["product_foundation"]["status"], "unassessed");
        assert_ne!(emitted["product_foundation"]["status"], "unassessed");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_default_preserves_channel_specific_entry_routing() {
        let root = temp_pack("canonical-default-channel-routing");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = prospect_brief_with_context(&root, &prospect_path, "linkedin", None, true)
            .expect("prospect brief should preserve channel routing");
        let channel_entry_ids = result["context"]["entries"]
            .as_array()
            .expect("context entries should be an array")
            .iter()
            .filter(|entry| entry["card_id"] == "channel-policies")
            .filter_map(|entry| entry["entry_id"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(result["job"], "outbound-copy-brief");
        assert_eq!(
            result["product_foundation"]["job_id"],
            "outbound-copy-brief"
        );
        assert!(channel_entry_ids.contains(&"linkedin-initial-touch"));
        assert!(
            channel_entry_ids
                .iter()
                .all(|entry_id| !entry_id.starts_with("email-"))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_brief_job_wins_over_canonical_default() {
        let root = temp_pack("explicit-brief-job");
        let result = emit_brief(&root, "PMM", None, Some("prospect-fit-or-brief"))
            .expect("explicit job should resolve");

        assert_eq!(result["inputs"]["job"], "prospect-fit-or-brief");
        assert_eq!(result["product_foundation"]["status"], "ready");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_workflows_preserve_legacy_free_text_defaults() {
        let root = temp_pack("legacy-default-brief-job");
        remove_canonical_brief_job(&root);
        let prospect_path = root.join("examples").join("clay-row.json");

        let prospect = prospect_brief(&root, &prospect_path, "linkedin", None)
            .expect("legacy prospect brief should resolve");
        let emitted =
            emit_brief(&root, "PMM", None, None).expect("legacy emit brief should resolve");

        assert_eq!(prospect["job"], "write linkedin outbound message");
        assert_eq!(emitted["inputs"]["job"], "unspecified GTM decision task");
        assert_eq!(prospect["product_foundation"]["status"], "unassessed");
        assert_eq!(emitted["product_foundation"]["status"], "unassessed");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_default_preserves_blocked_foundation() {
        let root = temp_pack("blocked-default-brief-job");
        add_selected_foundation_gap(&root);

        let result = emit_brief(&root, "PMM", None, None)
            .expect("canonical default should resolve blocked foundation");

        assert_eq!(result["inputs"]["job"], "outbound-copy-brief");
        assert_eq!(result["draft_status"], "blocked");
        assert_eq!(result["product_foundation"]["status"], "blocked");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn briefs_block_invalid_foundation_before_drafting() {
        let root = temp_pack("brief-invalid-foundation");
        set_foundation_facet_kind(&root, "unknown-foundation-kind");

        let emitted = emit_brief(&root, "PMM", None, Some("prospect-fit-or-brief"))
            .expect("emit brief should resolve invalid foundation");
        assert_eq!(emitted["draft_status"], "blocked");
        assert_eq!(emitted["context"]["status"], "blocked");
        assert_eq!(emitted["product_foundation"]["status"], "blocked");

        let prospect_path = root.join("examples").join("clay-row.json");
        let prospect = prospect_brief(
            &root,
            &prospect_path,
            "linkedin",
            Some("prospect-fit-or-brief"),
        )
        .expect("prospect brief should resolve invalid foundation");
        assert_eq!(prospect["draft_status"], "no-draft");
        assert_eq!(prospect["product_foundation"]["status"], "blocked");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_account_only_includes_specific_no_draft_reason() {
        let root = temp_pack("brief-account-only-no-draft");
        let prospect_path = root.join("examples").join("account-only.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "N/A",
  "title": "N/A",
  "company": "Northstar Cloud",
  "company_domain": "northstarcloud.com",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing prospect qualification data before routing new campaigns",
  "source_kind": "synthetic-example",
  "synthetic": true,
  "signals": [
    {
      "id": "qualification-data-standardization",
      "title": "Standardizing prospect qualification data",
      "source": "synthetic account-only row"
    }
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = prospect_brief_with_context(&root, &prospect_path, "linkedin", None, true)
            .expect("brief should succeed");

        assert_eq!(result["fit"]["status"], "insufficient-context");
        assert_eq!(result["draft_status"], "no-draft");
        assert_eq!(
            result["no_draft_reason"],
            "No person name or title was present in the prospect row; provide a reviewed contact before drafting."
        );
        assert_eq!(result["context"]["status"], "blocked");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_marks_starter_prospect_as_synthetic_example() {
        let root = temp_pack("brief-synthetic");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result =
            prospect_brief(&root, &prospect_path, "linkedin", None).expect("brief should succeed");

        assert_eq!(result["prospect_source"]["kind"], "synthetic-example");
        assert_eq!(result["prospect_source"]["synthetic"], true);
        assert!(
            result["prospect_source"]["guidance"]
                .as_str()
                .expect("guidance should be a string")
                .contains("Synthetic example fixture")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_context_includes_bounded_entry_bodies_and_guardrails() {
        let root = temp_pack("brief-context-ready");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = prospect_brief_with_context(&root, &prospect_path, "linkedin", None, true)
            .expect("brief should succeed");
        let entries = result["context"]["entries"]
            .as_array()
            .expect("context entries should be an array");
        let titles: Vec<&str> = entries
            .iter()
            .filter_map(|entry| entry["title"].as_str())
            .collect();

        assert_eq!(result["context"]["contract"], "mdp.context.v0");
        assert_eq!(result["context"]["status"], "ready");
        assert_eq!(
            result["runtime_context"],
            result["context"]["runtime_context"]
        );
        assert_eq!(
            result["runtime_context"]["contract"],
            "mdp.runtime-context.v0"
        );
        assert_eq!(result["runtime_context"]["timezone"], "UTC");
        assert!(titles.contains(&"Do not claim execution"));
        assert!(titles.contains(&"No message without context"));
        assert!(titles.contains(&"LinkedIn initial touch"));
        assert!(!titles.contains(&"LinkedIn follow-up"));
        assert!(!titles.contains(&"Email initial touch"));
        assert!(!titles.contains(&"Email follow-up"));
        assert!(!titles.contains(&"Call prep"));
        assert!(
            entries
                .iter()
                .any(|entry| entry["body"].as_str().is_some_and(|body| !body.is_empty()))
        );
        assert!(
            result["context"]["summary"]["guardrail_entry_count"]
                .as_u64()
                .expect("guardrail count")
                > 0
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_context_blocks_entries_when_fit_is_insufficient() {
        let root = temp_pack("brief-context-blocked");
        let prospect_path = root.join("examples").join("thin.json");
        std::fs::write(
            &prospect_path,
            r#"{"name":"Taylor Lee","title":"GTM Engineering Lead","company":"ExampleCo"}"#,
        )
        .expect("prospect should be writable");

        let result = prospect_brief_with_context(&root, &prospect_path, "linkedin", None, true)
            .expect("brief should succeed");

        assert_eq!(result["draft_status"], "no-draft");
        assert_eq!(result["context"]["status"], "blocked");
        assert_eq!(
            result["runtime_context"],
            result["context"]["runtime_context"]
        );
        assert_eq!(
            result["context"]["entries"]
                .as_array()
                .expect("entries array")
                .len(),
            0
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_ready_prospect_stays_no_draft_when_portfolio_context_blocks() {
        let root = temp_pack("brief-portfolio-scope-blocked");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = prospect_brief_with_context(
            &root,
            &prospect_path,
            "linkedin",
            Some("portfolio scope example"),
            true,
        )
        .expect("brief should succeed");

        assert_eq!(result["fit"]["status"], "fit");
        assert_eq!(result["portfolio_sensitive"], true);
        assert_eq!(result["context"]["status"], "blocked");
        assert_eq!(result["draft_status"], "no-draft");
        assert!(
            result["context"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["selection"] == "guardrail")
        );
        assert!(
            result["agent_instruction"]
                .as_str()
                .unwrap()
                .starts_with("Stop before drafting")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn readable_brief_renders_ready_markdown_with_frontmatter() {
        let root = temp_pack("readable-brief-ready");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = prospect_brief_with_context(&root, &prospect_path, "linkedin", None, true)
            .expect("brief should succeed");
        let markdown = render_readable_prospect_brief(&result);

        assert!(markdown.starts_with("---\n"));
        assert!(markdown.contains("first_name: \"Alex\""));
        assert!(markdown.contains("last_name: \"Rivera\""));
        assert!(markdown.contains("company_domain: \"example.com\""));
        assert!(markdown.contains("persona: \"GTM Engineering\""));
        assert!(markdown.contains(
            "tags: [\"GTM Engineering\", \"agent-assisted GTM\", \"synthetic-example\"]"
        ));
        assert!(markdown.contains("---\n\n# Prospect Brief: Alex Rivera at ExampleCo"));
        assert!(!markdown.contains("```yaml"));
        assert!(!markdown.contains("<section"));
        assert!(markdown.contains("## Fit / Draft Readiness"));
        assert!(markdown.contains("- draft_status: ready"));
        assert!(markdown.contains("## Evidence Receipts and Signal Decisions"));
        assert!(markdown.contains("- signal_authority: legacy"));
        assert!(markdown.contains("## Proposed Outreach Copy"));
        assert!(markdown.contains("No proposed outreach copy is included"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn readable_brief_renders_no_draft_reason_without_inventing_contact() {
        let root = temp_pack("readable-brief-no-draft");
        let prospect_path = root.join("examples").join("account-only.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "N/A",
  "title": "N/A",
  "company": "Northstar Cloud",
  "company_domain": "northstarcloud.com",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing prospect qualification data before routing new campaigns",
  "source_kind": "synthetic-example",
  "synthetic": true,
  "signals": [
    {
      "id": "qualification-data-standardization",
      "title": "Standardizing prospect qualification data",
      "source": "synthetic account-only row",
      "state_as": "hypothesis"
    }
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = prospect_brief_with_context(&root, &prospect_path, "linkedin", None, true)
            .expect("brief should succeed");
        let markdown = render_readable_prospect_brief(&result);

        assert!(markdown.starts_with("---\n"));
        assert!(markdown.contains("first_name: unknown"));
        assert!(markdown.contains("last_name: unknown"));
        assert!(markdown.contains("title: unknown"));
        assert!(markdown.contains("tags: ["));
        assert!(markdown.contains("\"agent-assisted GTM\""));
        assert!(markdown.contains("\"synthetic-example\""));
        assert!(!markdown.contains("```yaml"));
        assert!(!markdown.contains("<section"));
        assert!(markdown.contains("- draft_status: no-draft"));
        assert!(markdown.contains("No reviewed current role/title was supplied."));
        assert!(markdown.contains("state_as: hypothesis"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_rejects_unknown_prospect_fields_before_drafting() {
        let root = temp_pack("brief-unknown-prospect-field");
        let prospect_path = root.join("examples").join("unknown-field.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "GTM Engineering Lead",
  "company": "ExampleCo",
  "territory": "enterprise"
}"#,
        )
        .expect("prospect should be writable");

        let err = prospect_brief(&root, &prospect_path, "linkedin", None)
            .expect_err("unknown prospect field should fail");
        let message = err.to_string();

        assert!(message.contains("prospect_unknown_field"));
        assert!(message.contains("attributes.territory"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn emit_brief_resolves_manifest_persona_aliases() {
        let root = temp_pack("emit-brief-persona-alias");

        let result = emit_brief(
            &root,
            "Growth Engineer",
            None,
            Some("agent brief for enriched row"),
        )
        .expect("emit brief should succeed");

        assert_eq!(result["requested_persona"], "Growth Engineer");
        assert_eq!(result["persona"], "GTM Engineering");
        assert_eq!(result["inputs"]["requested_persona"], "Growth Engineer");
        assert_eq!(result["inputs"]["persona"], "GTM Engineering");
        assert_eq!(result["persona_resolution"]["resolved"], true);
        assert!(
            result["required_load_order"]
                .as_array()
                .expect("load order should be an array")
                .iter()
                .any(|path| path == ".mdp/cards/fit-rules.yaml")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_routes_with_manifest_persona_mapping() {
        let root = temp_pack("brief-persona-mapping");
        let prospect_path = root.join("examples").join("demand-gen.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result =
            prospect_brief(&root, &prospect_path, "linkedin", None).expect("brief should succeed");

        assert_eq!(result["persona"], "PMM");
        assert_eq!(result["draft_status"], "ready");
        assert_eq!(
            result["persona_resolution"]["source"],
            "manifest.persona_mappings.title_keywords"
        );
        assert!(
            result["required_load_order"]
                .as_array()
                .expect("load order should be an array")
                .iter()
                .any(|path| path == ".mdp/cards/ctas.yaml")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn emit_brief_preserves_blocked_foundation_without_drafting_entries() {
        let root = temp_pack("emit-brief-foundation-gap");
        add_selected_foundation_gap(&root);

        let result = emit_brief(&root, "PMM", None, Some("prospect-fit-or-brief"))
            .expect("brief should resolve blocked foundation");

        assert_eq!(result["draft_status"], "blocked");
        assert_eq!(result["context"]["status"], "blocked");
        assert_eq!(result["product_foundation"]["status"], "blocked");
        assert_eq!(
            result["context"]["reason"],
            "selected product foundation authority is blocked"
        );
        assert!(
            result["context"]["entries"]
                .as_array()
                .expect("context entries")
                .is_empty()
        );
        assert!(
            result["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics")
                .iter()
                .any(
                    |diagnostic| diagnostic["code"] == "product_foundation_selected_facet_has_gaps"
                )
        );
        assert!(
            result["product_foundation_load_order"]
                .as_array()
                .expect("foundation load order")
                .iter()
                .any(|reference| {
                    reference["reference_kind"] == "gap"
                        && reference["entry_id"] == "missing-company-proof"
                })
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn emit_brief_honors_needs_review_activation_veto() {
        let root = temp_pack("emit-brief-needs-review");
        set_profile_activation(&root, "needs-review");

        let result = emit_brief(&root, "PMM", None, Some("prospect-fit-or-brief"))
            .expect("brief should honor activation veto");

        assert_eq!(result["draft_status"], "blocked");
        assert_eq!(result["context"]["status"], "blocked");
        assert_eq!(result["product_foundation"]["status"], "ready");
        assert_eq!(
            result["context"]["reason"],
            "profile activation requires review or is blocked"
        );
        assert!(
            result["context"]["entries"]
                .as_array()
                .expect("context entries")
                .is_empty()
        );
        assert!(
            !result["product_foundation_load_order"]
                .as_array()
                .expect("foundation load order")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
