use crate::constants::DEFAULT_DIR;
use crate::models::{Manifest, SourceTemporal};
use crate::pack_io::read_manifest;
use crate::time::{parse_day_cadence, parse_utc_seconds};
use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) const CONTRACT: &str = "mdp.temporal-health.v1";

fn diagnostic(code: &str, path: impl Into<String>, message: &str) -> Value {
    json!({"code": code, "path": path.into(), "message": message})
}
#[derive(Debug, Deserialize, Default)]
struct Ledger {
    #[serde(default)]
    sources: Vec<LedgerSource>,
}
#[derive(Debug, Deserialize, Default)]
struct LedgerSource {
    id: String,
    #[serde(default)]
    locator: Option<String>,
    #[serde(default)]
    temporal: Option<SourceTemporal>,
}

fn timestamp(
    value: Option<&String>,
    path: &str,
    as_of: i64,
    diagnostics: &mut Vec<Value>,
) -> Option<i64> {
    let Some(value) = value else { return None };
    match parse_utc_seconds(value) {
        Some(seconds) if seconds <= as_of => Some(seconds),
        _ => {
            diagnostics.push(diagnostic(
                "temporal_timestamp_invalid_or_future",
                path,
                "timestamp must be strict UTC and no later than evaluation time",
            ));
            None
        }
    }
}
fn policy_days(
    policy: Option<&crate::models::ReviewPolicy>,
    diagnostics: &mut Vec<Value>,
    path: &str,
) -> (Option<u32>, Option<u32>) {
    let Some(p) = policy else { return (None, None) };
    let cadence = p.cadence.as_deref().and_then(parse_day_cadence);
    if p.cadence.is_some() && cadence.is_none() {
        diagnostics.push(diagnostic(
            "temporal_cadence_invalid",
            format!("{path}/cadence"),
            "cadence must be a positive P<n>D",
        ));
    }
    if p.aging_after_days == Some(0)
        || p.stale_after_days == Some(0)
        || matches!((p.aging_after_days,p.stale_after_days),(Some(a),Some(s)) if s<a)
    {
        diagnostics.push(diagnostic(
            "temporal_threshold_invalid",
            path,
            "thresholds must be positive and non-contradictory",
        ));
    }
    (
        p.aging_after_days.or(cadence),
        p.stale_after_days.or(cadence.map(|d| d.saturating_mul(2))),
    )
}

fn check_transition(
    value: Option<&String>,
    origin: Option<i64>,
    path: &str,
    diagnostics: &mut Vec<Value>,
) {
    if let (Some(value), Some(origin)) = (value, origin)
        && parse_utc_seconds(value).is_some_and(|transition| transition < origin)
    {
        diagnostics.push(diagnostic(
            "temporal_transition_before_origin",
            path,
            "transition timestamp cannot precede its source or decision timestamp",
        ));
    }
}
fn source_state(
    source: &LedgerSource,
    root: &Path,
    as_of: i64,
    diagnostics: &mut Vec<Value>,
    index: usize,
) -> (&'static str, Option<i64>, Option<bool>) {
    let t = source.temporal.as_ref();
    let life = t.and_then(|x| x.lifecycle.as_deref()).unwrap_or("current");
    // Imported-at describes local ingestion, not source age.  Prefer the
    // source's observation time, with publication time as the other declared
    // source-origin clock; never substitute imported_at.
    let observed = t.and_then(|x| x.observed_at.as_ref());
    let published = t.and_then(|x| x.published_at.as_ref());
    let instant = timestamp(
        observed.or(published),
        &format!(".mdp/sources.yaml#/sources/{index}/temporal"),
        as_of,
        diagnostics,
    );
    let hash_match = source_hash_match(source, root);
    if hash_match == Some(false) {
        diagnostics.push(diagnostic(
            "source_hash_mismatch",
            format!(".mdp/sources.yaml#/sources/{index}/temporal/sha256"),
            "pack-local source bytes do not match the declared digest",
        ));
    }
    if life == "revoked" {
        return ("revoked", instant, hash_match);
    }
    if life == "superseded" {
        return ("superseded", instant, hash_match);
    }
    let Some(at) = instant else {
        return ("unknown", None, hash_match);
    };
    let (aging, stale) = policy_days(
        t.and_then(|x| x.review_policy.as_ref()),
        diagnostics,
        &format!("source/{}/review_policy", source.id),
    );
    let age = (as_of - at) / 86400;
    if stale.is_some_and(|n| age >= i64::from(n)) {
        ("stale", Some(at), hash_match)
    } else if aging.is_some_and(|n| age >= i64::from(n)) {
        ("aging", Some(at), hash_match)
    } else {
        ("current", Some(at), hash_match)
    }
}

fn source_hash_match(source: &LedgerSource, root: &Path) -> Option<bool> {
    let expected = source.temporal.as_ref()?.sha256.as_ref()?;
    let locator = source.locator.as_deref()?;
    let path = Path::new(locator);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
    {
        return None;
    }
    let resolved = crate::pack_io::resolve_pack_path(root, locator).ok()?;
    if !fs::symlink_metadata(&resolved).ok()?.file_type().is_file() {
        return None;
    }
    let bytes = fs::read(resolved).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)) == *expected)
}
pub(crate) fn validate_governance(root: &Path, manifest: &Manifest, as_of: i64) -> Vec<Value> {
    let mut d = Vec::new();
    let mut ids = BTreeSet::new();
    let mut source_ids_seen = BTreeSet::new();
    let mut card_entries = BTreeMap::new();
    for c in &manifest.cards {
        if let Ok(card) = crate::pack_io::read_card(
            &crate::pack_io::resolve_pack_path(root, &c.path).unwrap_or_default(),
        ) {
            for e in card.entries {
                card_entries.insert((c.id.clone(), e.id), true);
            }
        }
    }
    for (i, g) in manifest.decision_groups.iter().enumerate() {
        if !ids.insert(&g.id) {
            d.push(diagnostic(
                "decision_group_duplicate_id",
                format!("#/decision_groups/{i}/id"),
                "decision group IDs must be unique",
            ));
        }
        if g.entries.is_empty() {
            d.push(diagnostic(
                "decision_group_entries_empty",
                format!("#/decision_groups/{i}/entries"),
                "present decision groups require at least one exact entry reference",
            ));
        }
        if g.jobs.is_empty() {
            d.push(diagnostic(
                "decision_group_jobs_empty",
                format!("#/decision_groups/{i}/jobs"),
                "present decision groups require at least one canonical job reference",
            ));
        }
        for e in &g.entries {
            if !card_entries.contains_key(&(e.card_id.clone(), e.entry_id.clone())) {
                d.push(diagnostic(
                    "decision_group_entry_unknown",
                    format!("#/decision_groups/{i}/entries"),
                    "entry reference must resolve exactly",
                ));
            }
        }
        for j in &g.jobs {
            if !manifest.jobs.iter().any(|x| x.id == *j) {
                d.push(diagnostic(
                    "decision_group_job_unknown",
                    format!("#/decision_groups/{i}/jobs"),
                    "job reference must resolve to a canonical manifest job",
                ));
            }
        }
        if let Some(t) = &g.temporal {
            let changed = timestamp(
                t.changed_at.as_ref(),
                "#/decision_groups/temporal/changed_at",
                as_of,
                &mut d,
            );
            let reviewed = timestamp(
                t.reviewed_at.as_ref(),
                "#/decision_groups/temporal/reviewed_at",
                as_of,
                &mut d,
            );
            if reviewed.zip(changed).is_some_and(|(r, c)| r < c) {
                d.push(diagnostic(
                    "decision_reviewed_before_changed",
                    "#/decision_groups/temporal/reviewed_at",
                    "reviewed_at cannot precede changed_at",
                ));
            }
            if !matches!(t.lifecycle.as_str(), "current" | "revoked" | "superseded") {
                d.push(diagnostic(
                    "decision_lifecycle_invalid",
                    "#/decision_groups/temporal/lifecycle",
                    "lifecycle must be current, revoked, or superseded",
                ));
            }
            if t.lifecycle == "revoked" && t.revoked_at.is_none()
                || t.lifecycle != "revoked" && t.revoked_at.is_some()
            {
                d.push(diagnostic(
                    "decision_revocation_transition_invalid",
                    "#/decision_groups/temporal/revoked_at",
                    "revoked_at must be present only for revoked decisions",
                ));
            }
            if t.lifecycle == "superseded" && t.superseded_at.is_none()
                || t.lifecycle != "superseded" && t.superseded_at.is_some()
            {
                d.push(diagnostic(
                    "decision_supersession_transition_invalid",
                    "#/decision_groups/temporal/superseded_at",
                    "superseded_at must be present only for superseded decisions",
                ));
            }
            check_transition(
                t.revoked_at.as_ref(),
                changed,
                "#/decision_groups/temporal/revoked_at",
                &mut d,
            );
            check_transition(
                t.superseded_at.as_ref(),
                changed,
                "#/decision_groups/temporal/superseded_at",
                &mut d,
            );
            check_transition(
                t.revoked_at.as_ref(),
                reviewed,
                "#/decision_groups/temporal/revoked_at",
                &mut d,
            );
            check_transition(
                t.superseded_at.as_ref(),
                reviewed,
                "#/decision_groups/temporal/superseded_at",
                &mut d,
            );
            for r in &t.source_revisions {
                if r.sha256.len() != 64
                    || !r
                        .sha256
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() && (!c.is_ascii_uppercase()))
                {
                    d.push(diagnostic(
                        "source_revision_hash_invalid",
                        "#/decision_groups/temporal/source_revisions",
                        "source revision must be exactly 64 lowercase hexadecimal characters",
                    ));
                }
            }
            if let Some(p) = &g.review_policy {
                policy_days(Some(p), &mut d, "#/decision_groups/review_policy");
            }
        }
    }
    let ledger_path = root.join(DEFAULT_DIR).join("sources.yaml");
    if let Ok(raw) = fs::read_to_string(&ledger_path) {
        match serde_yaml::from_str::<Ledger>(&raw) {
            Ok(ledger) => {
                let source_ids: BTreeSet<_> =
                    ledger.sources.iter().map(|s| s.id.as_str()).collect();
                let decision_ids: BTreeSet<_> = manifest
                    .decision_groups
                    .iter()
                    .map(|g| g.id.as_str())
                    .collect();
                for (i, group) in manifest.decision_groups.iter().enumerate() {
                    if let Some(temporal) = &group.temporal {
                        for revision in &temporal.source_revisions {
                            if !source_ids.contains(revision.source_id.as_str()) {
                                d.push(diagnostic(
                                    "source_revision_source_unknown",
                                    format!("#/decision_groups/{i}/temporal/source_revisions"),
                                    "source revision must reference an existing source",
                                ));
                            }
                        }
                        if let Some(replacement) = &temporal.replacement_group {
                            if replacement == &group.id
                                || !decision_ids.contains(replacement.as_str())
                            {
                                d.push(diagnostic(
                                    "decision_replacement_group_invalid",
                                    format!("#/decision_groups/{i}/temporal/replacement_group"),
                                    "replacement_group must reference a distinct existing decision group",
                                ));
                            }
                        }
                    }
                }
                for (i, source) in ledger.sources.iter().enumerate() {
                    if !source_ids_seen.insert(source.id.as_str()) {
                        d.push(diagnostic(
                            "source_duplicate_id",
                            format!(".mdp/sources.yaml#/sources/{i}/id"),
                            "source IDs must be unique",
                        ));
                    }
                    if let Some(t) = &source.temporal {
                        if !matches!(
                            t.lifecycle.as_deref().unwrap_or("current"),
                            "current" | "revoked" | "superseded"
                        ) {
                            d.push(diagnostic(
                                "source_lifecycle_invalid",
                                format!(".mdp/sources.yaml#/sources/{i}/temporal/lifecycle"),
                                "lifecycle must be current, revoked, or superseded",
                            ));
                        }
                        let observed = timestamp(
                            t.observed_at.as_ref(),
                            &format!(".mdp/sources.yaml#/sources/{i}/temporal/observed_at"),
                            as_of,
                            &mut d,
                        );
                        let published = timestamp(
                            t.published_at.as_ref(),
                            &format!(".mdp/sources.yaml#/sources/{i}/temporal/published_at"),
                            as_of,
                            &mut d,
                        );
                        let imported = timestamp(
                            t.imported_at.as_ref(),
                            &format!(".mdp/sources.yaml#/sources/{i}/temporal/imported_at"),
                            as_of,
                            &mut d,
                        );
                        check_transition(
                            t.revoked_at.as_ref(),
                            observed.max(published).max(imported),
                            &format!(".mdp/sources.yaml#/sources/{i}/temporal/revoked_at"),
                            &mut d,
                        );
                        check_transition(
                            t.superseded_at.as_ref(),
                            observed.max(published).max(imported),
                            &format!(".mdp/sources.yaml#/sources/{i}/temporal/superseded_at"),
                            &mut d,
                        );
                        for (name, value) in [
                            ("observed_at", &t.observed_at),
                            ("published_at", &t.published_at),
                            ("imported_at", &t.imported_at),
                            ("revoked_at", &t.revoked_at),
                            ("superseded_at", &t.superseded_at),
                        ] {
                            if name != "observed_at"
                                && name != "published_at"
                                && name != "imported_at"
                            {
                                timestamp(
                                    value.as_ref(),
                                    &format!(".mdp/sources.yaml#/sources/{i}/temporal/{name}"),
                                    as_of,
                                    &mut d,
                                );
                            }
                        }
                        if t.lifecycle.as_deref() == Some("revoked") && t.revoked_at.is_none()
                            || t.lifecycle.as_deref() != Some("revoked") && t.revoked_at.is_some()
                        {
                            d.push(diagnostic(
                                "source_revocation_transition_invalid",
                                format!(".mdp/sources.yaml#/sources/{i}/temporal/revoked_at"),
                                "revoked_at must be present only for revoked sources",
                            ));
                        }
                        if t.lifecycle.as_deref() == Some("superseded") && t.superseded_at.is_none()
                            || t.lifecycle.as_deref() != Some("superseded")
                                && t.superseded_at.is_some()
                        {
                            d.push(diagnostic(
                                "source_supersession_transition_invalid",
                                format!(".mdp/sources.yaml#/sources/{i}/temporal/superseded_at"),
                                "superseded_at must be present only for superseded sources",
                            ));
                        }
                        if let Some(id) = &t.superseded_by {
                            if id == &source.id || !source_ids.contains(id.as_str()) {
                                d.push(diagnostic(
                                    "source_superseded_by_invalid",
                                    format!(
                                        ".mdp/sources.yaml#/sources/{i}/temporal/superseded_by"
                                    ),
                                    "superseded_by must reference a distinct existing source",
                                ));
                            }
                        }
                        if let Some(hash) = &t.sha256 {
                            if hash.len() != 64
                                || !hash
                                    .chars()
                                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                            {
                                d.push(diagnostic(
                                    "source_hash_invalid",
                                    format!(".mdp/sources.yaml#/sources/{i}/temporal/sha256"),
                                    "sha256 must be exactly 64 lowercase hexadecimal characters",
                                ));
                            }
                        }
                        policy_days(
                            t.review_policy.as_ref(),
                            &mut d,
                            &format!("source/{}/review_policy", source.id),
                        );
                    }
                }
            }
            Err(_) => d.push(diagnostic(
                "temporal_source_ledger_malformed",
                ".mdp/sources.yaml",
                "source ledger temporal fields must match the typed governance shape",
            )),
        }
    }
    if let Some(publication) = &manifest.provenance.temporal {
        timestamp(
            publication.published_at.as_ref(),
            "#/provenance/temporal/published_at",
            as_of,
            &mut d,
        );
        if publication.receipt_sha256.as_ref().is_some_and(|h| {
            h.len() != 64
                || !h
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        }) {
            d.push(diagnostic(
                "publication_receipt_hash_invalid",
                "#/provenance/temporal/receipt_sha256",
                "receipt_sha256 must be exactly 64 lowercase hexadecimal characters",
            ));
        }
    }
    d
}
pub(crate) fn temporal_health(root: &Path, as_of_text: Option<&str>) -> Result<Value> {
    let as_of = match as_of_text {
        Some(v) => {
            parse_utc_seconds(v).ok_or_else(|| anyhow!("--as-of must be strict UTC timestamp"))?
        }
        None => {
            let c = crate::runtime_context::current_runtime_context()?;
            parse_utc_seconds(c["now_utc"].as_str().unwrap()).unwrap()
        }
    };
    let manifest = read_manifest(root)?;
    let mut diagnostics = validate_governance(root, &manifest, as_of);
    let ledger_path = root.join(DEFAULT_DIR).join("sources.yaml");
    let ledger = if ledger_path.exists() {
        serde_yaml::from_str::<Ledger>(&fs::read_to_string(&ledger_path)?)?
    } else {
        Ledger::default()
    };
    let mut sources = Vec::new();
    let mut source_map: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (i, s) in ledger.sources.iter().enumerate() {
        let (state, at, hash_match) = source_state(s, root, as_of, &mut diagnostics, i);
        // Revision comparison is against the source's declared digest. Local
        // byte verification is a separate, optional fact.
        source_map.insert(
            s.id.clone(),
            s.temporal.as_ref().and_then(|t| t.sha256.clone()),
        );
        let t = s.temporal.as_ref();
        let origin = at.map(format_timestamp);
        let next_review_at = t.and_then(|t| t.review_policy.as_ref()).and_then(|p| {
            p.cadence
                .as_deref()
                .and_then(parse_day_cadence)
                .and_then(|days| {
                    at.and_then(|origin| crate::time::checked_add_days(origin, days))
                        .map(format_timestamp)
                })
        });
        sources.push(json!({"id":s.id,"state":state,"observed_at":t.and_then(|x|x.observed_at.clone()),"published_at":t.and_then(|x|x.published_at.clone()),"imported_at":t.and_then(|x|x.imported_at.clone()),"age_origin_at":origin,"next_review_at":next_review_at,"hash_match":hash_match}));
    }
    let mut decisions = Vec::new();
    for g in &manifest.decision_groups {
        let t = g.temporal.as_ref();
        let lifecycle = t.map(|x| x.lifecycle.as_str()).unwrap_or("unknown");
        let reviewed = t
            .and_then(|x| x.reviewed_at.as_ref())
            .and_then(|x| parse_utc_seconds(x));
        let changed = t
            .and_then(|x| x.changed_at.as_ref())
            .and_then(|x| parse_utc_seconds(x));
        let mismatch = t.is_some_and(|x| {
            x.source_revisions
                .iter()
                .any(|r| match source_map.get(&r.source_id) {
                    Some(Some(hash)) => hash != &r.sha256,
                    _ => true,
                })
        });
        if let Some(t) = t {
            for r in &t.source_revisions {
                if source_map
                    .get(&r.source_id)
                    .and_then(|v| v.as_ref())
                    .is_none()
                {
                    diagnostics.push(diagnostic("source_revision_unverifiable", "#/decision_groups/temporal/source_revisions", "source revision cannot be compared because the source has no declared digest"));
                }
            }
        }
        let state = if lifecycle == "revoked" {
            "revoked"
        } else if lifecycle == "superseded" {
            "superseded"
        } else if reviewed.is_none() {
            "never-reviewed"
        } else if mismatch {
            "review-due"
        } else if reviewed.is_some() {
            let (aging, stale) =
                policy_days(g.review_policy.as_ref(), &mut diagnostics, "decision");
            if stale.is_some_and(|n| as_of - reviewed.unwrap() >= i64::from(n) * 86400)
                || changed.is_some_and(|changed| reviewed.unwrap() < changed)
            {
                "review-overdue"
            } else if aging.is_some_and(|n| as_of - reviewed.unwrap() >= i64::from(n) * 86400) {
                "review-due"
            } else {
                "review-current"
            }
        } else {
            "review-current"
        };
        let next_review_at = t
            .and_then(|x| x.reviewed_at.as_ref())
            .and_then(|value| parse_utc_seconds(value))
            .and_then(|reviewed| {
                g.review_policy
                    .as_ref()
                    .and_then(|p| p.cadence.as_deref())
                    .and_then(parse_day_cadence)
                    .and_then(|days| crate::time::checked_add_days(reviewed, days))
                    .map(format_timestamp)
            });
        decisions.push(json!({"id":g.id,"label":g.label,"state":state,"reviewed_at":t.and_then(|x|x.reviewed_at.clone()),"changed_at":t.and_then(|x|x.changed_at.clone()),"source_revision_mismatch":mismatch,"next_review_at":next_review_at}));
    }
    let publication_temporal = manifest.provenance.temporal.as_ref();
    let receipt_hash_valid = publication_temporal
        .and_then(|x| x.receipt_sha256.as_ref())
        .is_some_and(|h| {
            h.len() == 64
                && h.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        });
    let complete_binding =
        publication_temporal.is_some_and(|x| x.receipt_ref.is_some() && receipt_hash_valid);
    if publication_temporal.is_some_and(|x| x.receipt_ref.is_some() ^ x.receipt_sha256.is_some()) {
        diagnostics.push(diagnostic(
            "publication_binding_partial",
            "#/provenance/temporal",
            "receipt_ref and a valid receipt_sha256 are both required for receipt-bound authority",
        ));
    }
    let publication = json!({"state":publication_temporal.and_then(|x|x.published_at.as_ref()).map(|_|"known").unwrap_or("unknown"),"published_at":publication_temporal.and_then(|x|x.published_at.clone()),"receipt_ref":publication_temporal.and_then(|x|x.receipt_ref.clone()),"receipt_sha256":publication_temporal.and_then(|x|x.receipt_sha256.clone()),"authority":if complete_binding {"receipt-bound"} else if publication_temporal.is_some() {"declared-unverified"} else {"unknown"}});
    let recommendation = if !diagnostics.is_empty() {
        "Review the listed temporal diagnostics and unknown evidence."
    } else if decisions
        .iter()
        .any(|value| value["state"] == "review-overdue")
    {
        "Review overdue decision groups before relying on them."
    } else if decisions.iter().any(|value| value["state"] == "review-due") {
        "Review decision groups whose source revisions changed."
    } else if sources.iter().any(|value| value["state"] == "stale") {
        "Review stale source evidence before relying on it."
    } else if sources.iter().any(|value| value["state"] == "unknown") {
        "Declare source observation evidence for unknown sources."
    } else if decisions
        .iter()
        .any(|value| value["state"] == "never-reviewed")
    {
        "Review decision groups that have never been reviewed."
    } else {
        "No action required; continue periodic review."
    };
    Ok(
        json!({"contract":CONTRACT,"evaluation":{"as_of":format_timestamp(as_of),"timezone":"UTC"},"sources":sources,"decision_review":decisions,"pack_publication":publication,"diagnostics":diagnostics,"recommendation":recommendation,"status":if recommendation.starts_with("No action") {"available"} else {"available-with-diagnostics"}}),
    )
}
fn format_timestamp(s: i64) -> String {
    let days = s.div_euclid(86_400);
    let day_seconds = s.rem_euclid(86_400);
    let (year, month, day) = crate::time::civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60,
        day_seconds % 60
    )
}
