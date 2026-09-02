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
    let aging = p.aging_after_days.or(cadence);
    let stale = match p
        .stale_after_days
        .or_else(|| cadence.and_then(|d| d.checked_mul(2)))
    {
        Some(value) => Some(value),
        None if cadence.is_some() => {
            diagnostics.push(diagnostic(
                "temporal_threshold_invalid",
                path,
                "derived stale threshold overflows",
            ));
            None
        }
        None => None,
    };
    if aging == Some(0) || stale == Some(0) || matches!((aging, stale), (Some(a), Some(s)) if s < a)
    {
        diagnostics.push(diagnostic(
            "temporal_threshold_invalid",
            path,
            "thresholds must be positive and non-contradictory",
        ));
    }
    (aging, stale)
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
    let observed_at = timestamp(
        observed,
        &format!(".mdp/sources.yaml#/sources/{index}/temporal/observed_at"),
        as_of,
        diagnostics,
    );
    let published_at = timestamp(
        published,
        &format!(".mdp/sources.yaml#/sources/{index}/temporal/published_at"),
        as_of,
        diagnostics,
    );
    let instant = observed_at.or(published_at);
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
    if !valid_sha256(expected) {
        // A malformed declaration is not revision evidence.  Validation
        // reports it, but the evaluator must not compare against it.
        return None;
    }
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
    // Source locators are pack-root-relative (existing ledgers use both
    // `.mdp/...` and `examples/...`), unlike manifest card paths which are
    // `.mdp`-relative. Canonicalize both ends so an intermediate symlink
    // cannot escape the pack, and reject a symlink as the final locator too.
    let resolved = root.join(path);
    let pack_root = root.canonicalize().ok()?;
    let real_path = resolved.canonicalize().ok()?;
    if !real_path.starts_with(&pack_root)
        || fs::symlink_metadata(&resolved)
            .ok()?
            .file_type()
            .is_symlink()
        || !real_path.is_file()
    {
        return None;
    }
    let bytes = fs::read(real_path).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)) == *expected)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn timestamp_in_wire_range(seconds: i64) -> bool {
    let minimum = crate::time::days_from_civil(0, 1, 1) * 86_400;
    let maximum = (crate::time::days_from_civil(9_999, 12, 31) + 1) * 86_400 - 1;
    (minimum..=maximum).contains(&seconds)
}

fn verify_publication_receipt(
    root: &Path,
    receipt_ref: &str,
    expected: &str,
    diagnostics: &mut Vec<Value>,
) -> bool {
    if !valid_sha256(expected) {
        return false;
    }
    let resolved = match crate::pack_io::resolve_pack_path(root, receipt_ref) {
        Ok(path) => path,
        Err(_) => {
            diagnostics.push(diagnostic(
                "publication_receipt_unverifiable",
                "#/provenance/temporal/receipt_ref",
                "publication receipt reference is unsafe or outside the pack",
            ));
            return false;
        }
    };
    if !fs::symlink_metadata(&resolved)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_file())
    {
        diagnostics.push(diagnostic(
            "publication_receipt_unverifiable",
            "#/provenance/temporal/receipt_ref",
            "publication receipt must resolve to a readable regular file",
        ));
        return false;
    }
    let bytes = match fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(_) => {
            diagnostics.push(diagnostic(
                "publication_receipt_unverifiable",
                "#/provenance/temporal/receipt_ref",
                "publication receipt bytes are unavailable",
            ));
            return false;
        }
    };
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        diagnostics.push(diagnostic(
            "publication_receipt_mismatch",
            "#/provenance/temporal/receipt_sha256",
            "publication receipt bytes do not match the declared digest",
        ));
        return false;
    }
    true
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
            let revoked_at = timestamp(
                t.revoked_at.as_ref(),
                "#/decision_groups/temporal/revoked_at",
                as_of,
                &mut d,
            );
            let superseded_at = timestamp(
                t.superseded_at.as_ref(),
                "#/decision_groups/temporal/superseded_at",
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
            let _ = (revoked_at, superseded_at);
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
    let mut source_local_mismatch: BTreeSet<String> = BTreeSet::new();
    for (i, s) in ledger.sources.iter().enumerate() {
        let (state, at, hash_match) = source_state(s, root, as_of, &mut diagnostics, i);
        // Revision comparison is against the source's declared digest. Local
        // byte verification is a separate, optional fact.
        let declared_sha = s
            .temporal
            .as_ref()
            .and_then(|t| t.sha256.as_deref())
            .filter(|sha| valid_sha256(sha))
            .map(str::to_owned);
        if hash_match == Some(false) {
            source_local_mismatch.insert(s.id.clone());
        }
        source_map.insert(s.id.clone(), declared_sha);
        let t = s.temporal.as_ref();
        let origin = at.map(format_timestamp);
        let next_review_at = t.and_then(|t| t.review_policy.as_ref()).and_then(|p| {
            p.cadence
                .as_deref()
                .and_then(parse_day_cadence)
                .and_then(|days| {
                    at.and_then(|origin| crate::time::checked_add_days(origin, days))
                        .filter(|seconds| timestamp_in_wire_range(*seconds))
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
            .and_then(|x| parse_utc_seconds(x))
            .filter(|seconds| *seconds <= as_of);
        let changed = t
            .and_then(|x| x.changed_at.as_ref())
            .and_then(|x| parse_utc_seconds(x))
            .filter(|seconds| *seconds <= as_of);
        let changed_invalid = t
            .and_then(|x| x.changed_at.as_ref())
            .is_some_and(|_| changed.is_none());
        let mismatch = t.is_some_and(|x| {
            x.source_revisions
                .iter()
                .any(|r| match source_map.get(&r.source_id) {
                    Some(Some(hash)) if valid_sha256(&r.sha256) => {
                        hash != &r.sha256 || source_local_mismatch.contains(&r.source_id)
                    }
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
        } else if changed_invalid {
            // An invalid change clock cannot establish that a review is
            // current.  Keep the decision authority untouched, but require
            // a new review rather than failing open.
            "review-due"
        } else if reviewed.is_none() {
            "never-reviewed"
        } else if reviewed.is_some() {
            let (aging, stale) =
                policy_days(g.review_policy.as_ref(), &mut diagnostics, "decision");
            if stale.is_some_and(|n| as_of - reviewed.unwrap() >= i64::from(n) * 86400)
                || changed.is_some_and(|changed| reviewed.unwrap() < changed)
            {
                "review-overdue"
            } else if mismatch {
                "review-due"
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
            .filter(|reviewed| *reviewed <= as_of)
            .filter(|_| !changed_invalid)
            .and_then(|reviewed| {
                g.review_policy
                    .as_ref()
                    .and_then(|p| p.cadence.as_deref())
                    .and_then(parse_day_cadence)
                    .and_then(|days| crate::time::checked_add_days(reviewed, days))
                    .filter(|seconds| timestamp_in_wire_range(*seconds))
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
    let publication_time = publication_temporal
        .and_then(|x| x.published_at.as_ref())
        .and_then(|value| parse_utc_seconds(value))
        .filter(|published| *published <= as_of);
    let receipt_verified = publication_temporal
        .and_then(|x| x.receipt_ref.as_deref().zip(x.receipt_sha256.as_deref()))
        .is_some_and(|(receipt_ref, receipt_sha)| {
            verify_publication_receipt(root, receipt_ref, receipt_sha, &mut diagnostics)
        });
    let complete_binding = publication_time.is_some_and(|_| receipt_verified && receipt_hash_valid);
    if publication_temporal.is_some_and(|x| x.receipt_ref.is_some() ^ x.receipt_sha256.is_some()) {
        diagnostics.push(diagnostic(
            "publication_binding_partial",
            "#/provenance/temporal",
            "receipt_ref and a valid receipt_sha256 are both required for receipt-bound authority",
        ));
    }
    let publication = json!({"state":publication_time.map(|_|"known").unwrap_or("unknown"),"published_at":publication_temporal.and_then(|x|x.published_at.clone()),"receipt_ref":publication_temporal.and_then(|x|x.receipt_ref.clone()),"receipt_sha256":publication_temporal.and_then(|x|x.receipt_sha256.clone()),"authority":if complete_binding {"receipt-bound"} else if publication_time.is_some() {"declared-unverified"} else {"unknown"}});
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ReviewPolicy;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn persisted_timestamps_fail_closed_at_evaluation_boundary() {
        let mut diagnostics = Vec::new();
        let as_of = parse_utc_seconds("2026-09-02T00:00:00Z").unwrap();
        assert_eq!(
            timestamp(
                Some(&"2026-09-02T00:00:00Z".to_owned()),
                "/reviewed_at",
                as_of,
                &mut diagnostics
            ),
            Some(as_of)
        );
        assert!(
            timestamp(
                Some(&"2026-09-02T00:00:01Z".to_owned()),
                "/future",
                as_of,
                &mut diagnostics
            )
            .is_none()
        );
        assert!(
            timestamp(
                Some(&"2026-09-02T00:00:00+00:00".to_owned()),
                "/malformed",
                as_of,
                &mut diagnostics
            )
            .is_none()
        );
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn malformed_hash_is_not_revision_evidence() {
        assert!(valid_sha256(&"a".repeat(64)));
        assert!(!valid_sha256(&"A".repeat(64)));
        assert!(!valid_sha256("not-a-sha"));
    }

    #[test]
    fn pack_local_hash_mismatch_is_stable_review_evidence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-temporal-health-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join(DEFAULT_DIR)).unwrap();
        fs::write(root.join(DEFAULT_DIR).join("source.txt"), b"actual bytes").unwrap();
        let source = LedgerSource {
            id: "source".into(),
            locator: Some(".mdp/source.txt".into()),
            temporal: Some(SourceTemporal {
                sha256: Some("0".repeat(64)),
                observed_at: Some("2026-09-01T00:00:00Z".into()),
                ..SourceTemporal::default()
            }),
        };
        let mut diagnostics = Vec::new();
        let (_, _, hash_match) = source_state(
            &source,
            &root,
            parse_utc_seconds("2026-09-02T00:00:00Z").unwrap(),
            &mut diagnostics,
            0,
        );
        assert_eq!(hash_match, Some(false));
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "source_hash_mismatch")
        );
        let _ = fs::remove_dir_all(root);
    }

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn governed_pack(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-temporal-{label}-{nonce}"));
        let template =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/basic/.mdp");
        copy_tree(&template, &root.join(DEFAULT_DIR));
        let card = root.join(DEFAULT_DIR).join("cards/positioning.yaml");
        let sha = format!("{:x}", Sha256::digest(fs::read(&card).unwrap()));
        let receipt = b"synthetic publication receipt\n";
        let receipt_sha = format!("{:x}", Sha256::digest(receipt));
        fs::write(root.join(DEFAULT_DIR).join("receipt.json"), receipt).unwrap();
        fs::write(
            root.join(DEFAULT_DIR).join("sources.yaml"),
            format!(
                "format: mdp.sources.v0\nsources:\n- id: source\n  locator: .mdp/cards/positioning.yaml\n  temporal:\n    observed_at: 2026-01-01T00:00:00Z\n    imported_at: 2026-09-01T00:00:00Z\n    sha256: {sha}\n    lifecycle: current\n    review_policy:\n      cadence: P30D\n      aging_after_days: 30\n      stale_after_days: 60\n"
            ),
        )
        .unwrap();
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let mut manifest = fs::read_to_string(&manifest_path).unwrap();
        manifest.push_str(&format!(
            "  temporal:\n    published_at: 2026-09-01T00:00:00Z\n    receipt_ref: receipt.json\n    receipt_sha256: {receipt_sha}\ndecision_groups:\n- id: positioning-decision\n  label: Positioning decision\n  entries:\n  - card_id: positioning\n    entry_id: decision-layer\n  jobs:\n  - prospect-fit-or-brief\n  review_policy:\n    cadence: P10D\n    aging_after_days: 10\n    stale_after_days: 20\n  temporal:\n    lifecycle: current\n    changed_at: 2026-08-01T00:00:00Z\n    reviewed_at: 2026-09-01T00:00:00Z\n    source_revisions:\n    - source_id: source\n      sha256: {sha}\n"
        ));
        fs::write(manifest_path, manifest).unwrap();
        root
    }

    #[test]
    fn temporal_health_matrix_separates_source_review_and_binds_integrity() {
        let root = governed_pack("matrix");
        let as_of = "2026-09-02T00:00:00Z";
        let baseline = temporal_health(&root, Some(as_of)).unwrap();
        assert_eq!(baseline["sources"][0]["state"], "stale");
        assert_eq!(baseline["sources"][0]["hash_match"], true);
        assert_eq!(baseline["decision_review"][0]["state"], "review-current");
        assert_eq!(baseline["pack_publication"]["authority"], "receipt-bound");
        jsonschema::draft202012::validate(
            &crate::commands::schema(crate::cli::SchemaTarget::TemporalHealthV1),
            &baseline,
        )
        .unwrap();

        let hash_before = crate::artifact_hash::pack_content_sha256(&root).unwrap();
        let sources_path = root.join(DEFAULT_DIR).join("sources.yaml");
        let same_bytes = fs::read(&sources_path).unwrap();
        fs::write(&sources_path, &same_bytes).unwrap();
        assert_eq!(
            hash_before,
            crate::artifact_hash::pack_content_sha256(&root).unwrap(),
            "mtime/write time must not affect semantic identity"
        );
        let changed_sources = String::from_utf8(same_bytes)
            .unwrap()
            .replace("2026-09-01T00:00:00Z", "2026-08-31T00:00:00Z");
        fs::write(&sources_path, changed_sources).unwrap();
        assert_ne!(
            hash_before,
            crate::artifact_hash::pack_content_sha256(&root).unwrap(),
            "authority-bearing temporal content must affect semantic identity"
        );

        let card = root.join(DEFAULT_DIR).join("cards/positioning.yaml");
        let mut bytes = fs::read(&card).unwrap();
        bytes.push(b'\n');
        fs::write(card, bytes).unwrap();
        let mismatched = temporal_health(&root, Some(as_of)).unwrap();
        assert_eq!(mismatched["sources"][0]["hash_match"], false);
        assert_eq!(mismatched["decision_review"][0]["state"], "review-due");
        assert!(
            mismatched["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|d| {
                    d.get("code").is_some() && d.get("path").is_some() && d.get("message").is_some()
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn temporal_health_matrix_is_fail_closed_at_boundaries() {
        let root = governed_pack("boundaries");
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path).unwrap().replace(
            "reviewed_at: 2026-09-01T00:00:00Z",
            "reviewed_at: 2026-08-23T00:00:00Z",
        );
        fs::write(&manifest_path, &manifest).unwrap();
        let due = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(due["decision_review"][0]["state"], "review-due");
        let overdue = temporal_health(&root, Some("2026-09-12T00:00:00Z")).unwrap();
        assert_eq!(overdue["decision_review"][0]["state"], "review-overdue");

        let invalid = manifest
            .replace(
                "reviewed_at: 2026-08-23T00:00:00Z",
                "reviewed_at: 2027-01-01T00:00:00Z",
            )
            .replace(
                "published_at: 2026-09-01T00:00:00Z",
                "published_at: not-a-time",
            )
            .replace(&"a".repeat(64), "bad-hash");
        fs::write(&manifest_path, invalid).unwrap();
        let failed_closed = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(
            failed_closed["decision_review"][0]["state"],
            "never-reviewed"
        );
        assert_eq!(
            failed_closed["decision_review"][0]["next_review_at"],
            Value::Null
        );
        assert_eq!(failed_closed["pack_publication"]["state"], "unknown");
        assert_eq!(failed_closed["pack_publication"]["authority"], "unknown");
        assert!(
            failed_closed["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "temporal_timestamp_invalid_or_future")
        );

        for template in ["basic", "proposal"] {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../plugin/assets/templates")
                .join(template);
            assert!(temporal_health(&root, Some("2026-09-02T00:00:00Z")).is_ok());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_decision_transition_fails_validate_and_doctor_closed() {
        let root = governed_pack("invalid-transition");
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path).unwrap().replace(
            "lifecycle: current\n    changed_at:",
            "lifecycle: revoked\n    revoked_at: not-a-time\n    changed_at:",
        );
        fs::write(&manifest_path, manifest).unwrap();
        let validation = crate::commands::health::validate_pack(&root).unwrap();
        assert_eq!(validation["valid"], false);
        assert!(
            validation["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "temporal_timestamp_invalid_or_future"
                        && issue["severity"] == "error"
                })
        );
        let doctor = crate::commands::health::doctor(&root);
        assert_ne!(doctor["status"], "ready");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn overdue_review_precedes_source_revision_mismatch() {
        let root = governed_pack("overdue-mismatch");
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path).unwrap().replace(
            "reviewed_at: 2026-09-01T00:00:00Z",
            "reviewed_at: 2026-08-01T00:00:00Z",
        );
        fs::write(&manifest_path, manifest).unwrap();
        let card = root.join(DEFAULT_DIR).join("cards/positioning.yaml");
        let mut bytes = fs::read(&card).unwrap();
        bytes.push(b'\n');
        fs::write(card, bytes).unwrap();
        let health = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(health["decision_review"][0]["state"], "review-overdue");
        assert_eq!(
            health["decision_review"][0]["source_revision_mismatch"],
            true
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn publication_receipt_must_be_present_safe_and_byte_matching() {
        let root = governed_pack("receipt");
        let receipt = root.join(DEFAULT_DIR).join("receipt.json");
        fs::write(&receipt, b"tampered receipt\n").unwrap();
        let tampered = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(
            tampered["pack_publication"]["authority"],
            "declared-unverified"
        );
        assert!(
            tampered["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "publication_receipt_mismatch")
        );

        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("receipt_ref: receipt.json", "receipt_ref: ../receipt.json");
        fs::write(manifest_path, manifest).unwrap();
        let unsafe_ref = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(
            unsafe_ref["pack_publication"]["authority"],
            "declared-unverified"
        );
        assert!(
            unsafe_ref["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "publication_receipt_unverifiable")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cadence_defaults_are_checked_after_resolution() {
        let mut diagnostics = Vec::new();
        let policy = ReviewPolicy {
            cadence: Some("P90D".into()),
            aging_after_days: None,
            stale_after_days: Some(30),
        };
        let (aging, stale) = policy_days(Some(&policy), &mut diagnostics, "source/review_policy");
        assert_eq!((aging, stale), (Some(90), Some(30)));
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "temporal_threshold_invalid")
        );
        let _ = policy_days(
            Some(&ReviewPolicy {
                cadence: Some("P4294967295D".into()),
                ..ReviewPolicy::default()
            }),
            &mut diagnostics,
            "source/review_policy",
        );
        assert!(diagnostics.len() >= 2);
    }

    #[test]
    fn next_review_does_not_emit_outside_four_digit_utc_range() {
        let root = governed_pack("wire-range");
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("cadence: P10D", "cadence: P1D")
            .replace(
                "reviewed_at: 2026-09-01T00:00:00Z",
                "reviewed_at: 9999-12-31T00:00:00Z",
            );
        fs::write(&manifest_path, manifest).unwrap();
        let output = temporal_health(&root, Some("9999-12-31T00:00:00Z")).unwrap();
        assert_eq!(output["decision_review"][0]["next_review_at"], Value::Null);
        jsonschema::draft202012::validate(
            &crate::commands::schema(crate::cli::SchemaTarget::TemporalHealthV1),
            &output,
        )
        .unwrap();
        let _ = fs::remove_dir_all(root);
    }
}
