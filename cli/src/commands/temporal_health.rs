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

fn deduplicate_diagnostics(diagnostics: &mut Vec<Value>) {
    let mut seen = BTreeSet::new();
    diagnostics.retain(|diagnostic| {
        seen.insert((
            diagnostic["code"].as_str().unwrap_or_default().to_owned(),
            diagnostic["path"].as_str().unwrap_or_default().to_owned(),
            diagnostic["message"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ))
    });
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Ledger {
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    rules: Vec<String>,
    #[serde(default)]
    sources: Vec<LedgerSource>,
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct LedgerSource {
    id: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    locator: Option<String>,
    #[serde(default)]
    freshness: Option<String>,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    direct_claims: Vec<String>,
    #[serde(default)]
    interpretations: Vec<String>,
    #[serde(default)]
    gaps: Vec<String>,
    #[serde(default)]
    temporal: Option<SourceTemporal>,
}

struct LedgerLoad {
    ledger: Ledger,
    usable: bool,
    diagnostics: Vec<Value>,
}

fn load_ledger(root: &Path) -> LedgerLoad {
    let path = root.join(DEFAULT_DIR).join("sources.yaml");
    let malformed = || LedgerLoad {
        ledger: Ledger::default(),
        usable: false,
        diagnostics: vec![diagnostic(
            "temporal_source_ledger_malformed",
            ".mdp/sources.yaml",
            "source ledger temporal fields must match the typed governance shape",
        )],
    };
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LedgerLoad {
                ledger: Ledger::default(),
                usable: true,
                diagnostics: Vec::new(),
            };
        }
        Err(_) => return malformed(),
    };
    if !metadata.file_type().is_file() {
        return malformed();
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return malformed();
    };
    match serde_yaml::from_str::<Ledger>(&raw) {
        Ok(ledger) => LedgerLoad {
            ledger,
            usable: true,
            diagnostics: Vec::new(),
        },
        Err(_) => malformed(),
    }
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
    let mut projection_diagnostics = Vec::new();
    let observed_at = timestamp(
        observed,
        &format!(".mdp/sources.yaml#/sources/{index}/temporal/observed_at"),
        as_of,
        &mut projection_diagnostics,
    );
    let published_at = timestamp(
        published,
        &format!(".mdp/sources.yaml#/sources/{index}/temporal/published_at"),
        as_of,
        &mut projection_diagnostics,
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
    if !matches!(life, "current" | "revoked" | "superseded") {
        // Invalid lifecycle declarations are not evidence of a current
        // source. Keep the row visible, but fail closed in the projection.
        return ("unknown", instant, hash_match);
    }
    let transition = match life {
        "revoked" => t.and_then(|x| x.revoked_at.as_ref()),
        "superseded" => t.and_then(|x| x.superseded_at.as_ref()),
        _ => None,
    };
    let shape_valid = (life == "revoked") == t.is_some_and(|x| x.revoked_at.is_some())
        && (life == "superseded") == t.is_some_and(|x| x.superseded_at.is_some());
    let origin = observed_at.max(published_at);
    let origin_invalid = t.is_some_and(|x| {
        [x.observed_at.as_ref(), x.published_at.as_ref()]
            .into_iter()
            .flatten()
            .any(|value| parse_utc_seconds(value).is_none())
    });
    if !shape_valid
        || origin_invalid
        || (life != "current"
            && !transition.is_some_and(|value| {
                parse_utc_seconds(value)
                    .is_some_and(|at| at <= as_of && origin.is_none_or(|o| at >= o))
            }))
    {
        return ("unknown", instant, hash_match);
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
        &mut projection_diagnostics,
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

#[derive(Debug)]
struct SourceRevisionAuthority {
    sha256: Option<String>,
    transition_at: Option<i64>,
    transition_valid: bool,
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
    let loaded = load_ledger(root);
    let mut diagnostics = loaded.diagnostics;
    diagnostics.extend(validate_governance_with_ledger(
        root,
        manifest,
        as_of,
        &loaded.ledger,
        loaded.usable,
    ));
    diagnostics
}

fn validate_governance_with_ledger(
    root: &Path,
    manifest: &Manifest,
    as_of: i64,
    ledger: &Ledger,
    ledger_usable: bool,
) -> Vec<Value> {
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
        if g.id.trim().is_empty() {
            d.push(diagnostic(
                "decision_group_id_empty",
                format!("#/decision_groups/{i}/id"),
                "decision group ID must not be empty or whitespace-only",
            ));
        }
        if g.label.trim().is_empty() {
            d.push(diagnostic(
                "decision_group_label_empty",
                format!("#/decision_groups/{i}/label"),
                "decision group label must not be empty or whitespace-only",
            ));
        }
        if g.owner
            .as_deref()
            .is_some_and(|owner| owner.trim().is_empty())
        {
            d.push(diagnostic(
                "decision_group_owner_empty",
                format!("#/decision_groups/{i}/owner"),
                "decision group owner must not be empty or whitespace-only",
            ));
        }
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
        if let Some(p) = &g.review_policy {
            policy_days(
                Some(p),
                &mut d,
                &format!("#/decision_groups/{i}/review_policy"),
            );
        }
        if let Some(t) = &g.temporal {
            let changed = timestamp(
                t.changed_at.as_ref(),
                &format!("#/decision_groups/{i}/temporal/changed_at"),
                as_of,
                &mut d,
            );
            let reviewed = timestamp(
                t.reviewed_at.as_ref(),
                &format!("#/decision_groups/{i}/temporal/reviewed_at"),
                as_of,
                &mut d,
            );
            let revoked_at = timestamp(
                t.revoked_at.as_ref(),
                &format!("#/decision_groups/{i}/temporal/revoked_at"),
                as_of,
                &mut d,
            );
            let superseded_at = timestamp(
                t.superseded_at.as_ref(),
                &format!("#/decision_groups/{i}/temporal/superseded_at"),
                as_of,
                &mut d,
            );
            if reviewed.zip(changed).is_some_and(|(r, c)| r < c) {
                d.push(diagnostic(
                    "decision_reviewed_before_changed",
                    format!("#/decision_groups/{i}/temporal/reviewed_at"),
                    "reviewed_at cannot precede changed_at",
                ));
            }
            if !matches!(t.lifecycle.as_str(), "current" | "revoked" | "superseded") {
                d.push(diagnostic(
                    "decision_lifecycle_invalid",
                    format!("#/decision_groups/{i}/temporal/lifecycle"),
                    "lifecycle must be current, revoked, or superseded",
                ));
            }
            if t.lifecycle == "revoked" && t.revoked_at.is_none()
                || t.lifecycle != "revoked" && t.revoked_at.is_some()
            {
                d.push(diagnostic(
                    "decision_revocation_transition_invalid",
                    format!("#/decision_groups/{i}/temporal/revoked_at"),
                    "revoked_at must be present only for revoked decisions",
                ));
            }
            if t.lifecycle == "superseded" && t.superseded_at.is_none()
                || t.lifecycle != "superseded" && t.superseded_at.is_some()
            {
                d.push(diagnostic(
                    "decision_supersession_transition_invalid",
                    format!("#/decision_groups/{i}/temporal/superseded_at"),
                    "superseded_at must be present only for superseded decisions",
                ));
            }
            check_transition(
                t.revoked_at.as_ref(),
                changed,
                &format!("#/decision_groups/{i}/temporal/revoked_at"),
                &mut d,
            );
            check_transition(
                t.superseded_at.as_ref(),
                changed,
                &format!("#/decision_groups/{i}/temporal/superseded_at"),
                &mut d,
            );
            check_transition(
                t.revoked_at.as_ref(),
                reviewed,
                &format!("#/decision_groups/{i}/temporal/revoked_at"),
                &mut d,
            );
            check_transition(
                t.superseded_at.as_ref(),
                reviewed,
                &format!("#/decision_groups/{i}/temporal/superseded_at"),
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
                        format!("#/decision_groups/{i}/temporal/source_revisions"),
                        "source revision must be exactly 64 lowercase hexadecimal characters",
                    ));
                }
            }
        }
    }
    if ledger_usable {
        let source_ids: BTreeSet<_> = ledger
            .sources
            .iter()
            .filter(|s| !s.id.trim().is_empty())
            .map(|s| s.id.as_str())
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
            }
        }
        for (i, source) in ledger.sources.iter().enumerate() {
            // A blank ID is invalid identity, not a usable lookup key. Keep it
            // out of both the duplicate/authoritative source index and the
            // revision comparison path below; otherwise two invalid blanks
            // could appear to match and incorrectly clear mismatch state.
            let source_id_valid = !source.id.trim().is_empty();
            if !source_id_valid {
                d.push(diagnostic(
                    "source_id_empty",
                    format!(".mdp/sources.yaml#/sources/{i}/id"),
                    "source ID must not be empty or whitespace-only",
                ));
            }
            if source_id_valid && !source_ids_seen.insert(source.id.as_str()) {
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
                let _imported = timestamp(
                    t.imported_at.as_ref(),
                    &format!(".mdp/sources.yaml#/sources/{i}/temporal/imported_at"),
                    as_of,
                    &mut d,
                );
                check_transition(
                    t.revoked_at.as_ref(),
                    observed.max(published),
                    &format!(".mdp/sources.yaml#/sources/{i}/temporal/revoked_at"),
                    &mut d,
                );
                check_transition(
                    t.superseded_at.as_ref(),
                    observed.max(published),
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
                    if name != "observed_at" && name != "published_at" && name != "imported_at" {
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
                    || t.lifecycle.as_deref() != Some("superseded") && t.superseded_at.is_some()
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
                            format!(".mdp/sources.yaml#/sources/{i}/temporal/superseded_by"),
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
    for (i, group) in manifest.decision_groups.iter().enumerate() {
        if let Some(replacement) = group
            .temporal
            .as_ref()
            .and_then(|t| t.replacement_group.as_ref())
        {
            if replacement == &group.id
                || !manifest
                    .decision_groups
                    .iter()
                    .any(|g| g.id == *replacement)
            {
                d.push(diagnostic(
                    "decision_replacement_group_invalid",
                    format!("#/decision_groups/{i}/temporal/replacement_group"),
                    "replacement_group must reference a distinct existing decision group",
                ));
            }
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
    let loaded = load_ledger(root);
    let mut diagnostics = loaded.diagnostics;
    diagnostics.extend(validate_governance_with_ledger(
        root,
        &manifest,
        as_of,
        &loaded.ledger,
        loaded.usable,
    ));
    let ledger = loaded.ledger;
    let mut sources = Vec::new();
    let mut source_map: BTreeMap<String, SourceRevisionAuthority> = BTreeMap::new();
    let mut source_local_mismatch: BTreeSet<String> = BTreeSet::new();
    // Only an exact, unique nonblank identity is authoritative. Duplicate
    // rows remain visible in the projection, but cannot silently become a
    // last-wins revision lookup entry.
    let source_id_counts = ledger
        .sources
        .iter()
        .filter(|s| !s.id.trim().is_empty())
        .fold(BTreeMap::<&str, usize>::new(), |mut counts, source| {
            *counts.entry(source.id.as_str()).or_default() += 1;
            counts
        });
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
        // Blank source IDs are invalid and must never become authoritative
        // revision lookup keys. Their decision bindings remain unverifiable.
        if source_id_counts.get(s.id.as_str()) == Some(&1) {
            let temporal = s.temporal.as_ref();
            let lifecycle = temporal
                .and_then(|t| t.lifecycle.as_deref())
                .unwrap_or("current");
            let transition_text = match lifecycle {
                "revoked" => temporal.and_then(|t| t.revoked_at.as_ref()),
                "superseded" => temporal.and_then(|t| t.superseded_at.as_ref()),
                _ => None,
            };
            let transition_at = transition_text.and_then(|value| parse_utc_seconds(value));
            let observed_origin = temporal
                .and_then(|t| t.observed_at.as_ref())
                .and_then(|value| parse_utc_seconds(value));
            let published_origin = temporal
                .and_then(|t| t.published_at.as_ref())
                .and_then(|value| parse_utc_seconds(value));
            let origin = observed_origin.max(published_origin);
            let origin_invalid = temporal.is_some_and(|t| {
                [t.observed_at.as_ref(), t.published_at.as_ref()]
                    .into_iter()
                    .flatten()
                    .any(|value| parse_utc_seconds(value).is_none())
            });
            let lifecycle_valid = matches!(lifecycle, "current" | "revoked" | "superseded");
            let lifecycle_shape_valid = lifecycle_valid
                && ((lifecycle == "revoked") == temporal.is_some_and(|t| t.revoked_at.is_some()))
                && ((lifecycle == "superseded")
                    == temporal.is_some_and(|t| t.superseded_at.is_some()));
            let transition_valid = lifecycle_valid
                && lifecycle_shape_valid
                && !origin_invalid
                && (!matches!(lifecycle, "revoked" | "superseded")
                    || transition_at.is_some_and(|transition| {
                        transition <= as_of && origin.is_none_or(|start| transition >= start)
                    }));
            source_map.insert(
                s.id.clone(),
                SourceRevisionAuthority {
                    sha256: declared_sha,
                    transition_at,
                    transition_valid,
                },
            );
        }
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
    for (group_index, g) in manifest.decision_groups.iter().enumerate() {
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
        let decision_transition_valid = t.is_none_or(|temporal| {
            let transition = match temporal.lifecycle.as_str() {
                "revoked" => temporal.revoked_at.as_ref(),
                "superseded" => temporal.superseded_at.as_ref(),
                _ => None,
            };
            let shape = (temporal.lifecycle == "revoked") == temporal.revoked_at.is_some()
                && (temporal.lifecycle == "superseded") == temporal.superseded_at.is_some();
            shape
                && transition.is_none_or(|value| {
                    parse_utc_seconds(value)
                        .is_some_and(|at| at <= as_of && changed.is_none_or(|origin| at >= origin))
                })
        });
        let mismatch = t.is_some_and(|x| {
            x.source_revisions
                .iter()
                .any(|r| match source_map.get(&r.source_id) {
                    Some(authority)
                        if authority.transition_valid
                            && authority.sha256.is_some()
                            && valid_sha256(&r.sha256) =>
                    {
                        authority.sha256.as_ref() != Some(&r.sha256)
                            || source_local_mismatch.contains(&r.source_id)
                            || authority.transition_at.is_some_and(|transition| {
                                reviewed.is_some_and(|review| transition > review)
                            })
                    }
                    _ => true,
                })
        });
        if let Some(t) = t {
            for r in &t.source_revisions {
                if source_map.get(&r.source_id).is_none_or(|authority| {
                    authority.sha256.is_none() || !authority.transition_valid
                }) {
                    diagnostics.push(diagnostic("source_revision_unverifiable", format!("#/decision_groups/{group_index}/temporal/source_revisions"), "source revision cannot be compared because identity, digest, or lifecycle evidence is invalid or unavailable"));
                }
            }
        }
        let state = if !decision_transition_valid {
            "review-due"
        } else if lifecycle == "revoked" {
            "revoked"
        } else if lifecycle == "superseded" {
            "superseded"
        } else if !matches!(lifecycle, "current" | "revoked" | "superseded") {
            // Do not let an invalid lifecycle claim a current review state.
            "review-due"
        } else if changed_invalid {
            // An invalid change clock cannot establish that a review is
            // current.  Keep the decision authority untouched, but require
            // a new review rather than failing open.
            "review-due"
        } else if reviewed.is_none() {
            "never-reviewed"
        } else if reviewed.is_some() {
            let mut projection_policy_diagnostics = Vec::new();
            let (aging, stale) = policy_days(
                g.review_policy.as_ref(),
                &mut projection_policy_diagnostics,
                &format!("#/decision_groups/{group_index}/review_policy"),
            );
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
    deduplicate_diagnostics(&mut diagnostics);
    let publication = json!({"state":publication_time.map(|_|"known").unwrap_or("unknown"),"published_at":publication_temporal.and_then(|x|x.published_at.clone()),"receipt_ref":publication_temporal.and_then(|x|x.receipt_ref.clone()),"receipt_sha256":publication_temporal.and_then(|x|x.receipt_sha256.clone()),"authority":if complete_binding {"receipt-bound"} else if publication_time.is_some() {"declared-unverified"} else {"unknown"}});
    let recommendation = if !diagnostics.is_empty() {
        "Review the listed temporal diagnostics and unknown evidence."
    } else if decisions
        .iter()
        .any(|value| value["state"] == "review-overdue")
    {
        "Review overdue decision groups before relying on them."
    } else if decisions.iter().any(|value| value["state"] == "review-due") {
        "Review decision groups that are due for review."
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
        json!({"contract":CONTRACT,"evaluation":{"as_of":format_timestamp(as_of),"timezone":"UTC"},"sources":sources,"decision_review":decisions,"pack_publication":publication,"diagnostics":diagnostics,"recommendation":recommendation,"status":if diagnostics.is_empty() {"available"} else {"available-with-diagnostics"}}),
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
            ..LedgerSource::default()
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
    fn blank_source_identity_is_unverifiable_and_strict_validation_error() {
        let root = governed_pack("blank-source-id");
        let sources_path = root.join(DEFAULT_DIR).join("sources.yaml");
        let sources = fs::read_to_string(&sources_path)
            .unwrap()
            .replace("- id: source", "- id: '   '")
            .replace("lifecycle: current", "lifecycle: invalid");
        fs::write(sources_path, sources).unwrap();
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest_text = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("source_id: source", "source_id: '   '");
        fs::write(manifest_path, manifest_text).unwrap();

        let health = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        let diagnostics = health["diagnostics"].as_array().unwrap();
        assert!(diagnostics.iter().any(|d| {
            d["code"] == "source_id_empty" && d["path"] == ".mdp/sources.yaml#/sources/0/id"
        }));
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "source_lifecycle_invalid")
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "source_revision_unverifiable")
        );
        assert_eq!(
            health["decision_review"][0]["source_revision_mismatch"],
            true
        );
        assert_eq!(health["sources"][0]["state"], "unknown");

        let validation = crate::commands::health::validate_pack(&root).unwrap();
        assert_eq!(validation["valid"], false);
        assert!(
            validation["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "source_id_empty" && issue["severity"] == "error"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn due_review_without_diagnostics_remains_available_and_is_cause_neutral() {
        let root = governed_pack("cadence-due");
        let path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&path).unwrap().replace(
            "reviewed_at: 2026-09-01T00:00:00Z",
            "reviewed_at: 2026-08-20T00:00:00Z",
        );
        fs::write(path, manifest).unwrap();
        let output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(output["decision_review"][0]["state"], "review-due");
        assert!(output["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(output["status"], "available");
        assert_eq!(
            output["recommendation"],
            "Review decision groups that are due for review."
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_transition_before_or_after_review_controls_mismatch_only() {
        for lifecycle in ["revoked", "superseded"] {
            let root = governed_pack("source-transition");
            let source_path = root.join(DEFAULT_DIR).join("sources.yaml");
            let source = fs::read_to_string(&source_path)
            .unwrap()
            .replace(
                "observed_at: 2026-01-01T00:00:00Z",
                &format!("observed_at: 2026-01-01T00:00:00Z\n    {lifecycle}_at: 2026-02-01T00:00:00Z"),
            )
            .replace("lifecycle: current", &format!("lifecycle: {lifecycle}"))
            .replace(
                "imported_at: 2026-09-01T00:00:00Z",
                "imported_at: 2026-03-01T00:00:00Z",
            );
            fs::write(source_path, source).unwrap();
            let after = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
            assert_eq!(after["decision_review"][0]["state"], "review-current");
            assert!(
                !after["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "temporal_transition_before_origin")
            );
            assert_eq!(
                after["decision_review"][0]["source_revision_mismatch"],
                false
            );
            let path = root.join(DEFAULT_DIR).join("manifest.yaml");
            let manifest = fs::read_to_string(&path)
                .unwrap()
                .replace(
                    "reviewed_at: 2026-09-01T00:00:00Z",
                    "reviewed_at: 2026-01-15T00:00:00Z",
                )
                .replace(
                    "changed_at: 2026-08-01T00:00:00Z",
                    "changed_at: 2026-01-01T00:00:00Z",
                )
                .replace("cadence: P10D", "cadence: P1000D")
                .replace("aging_after_days: 10", "aging_after_days: 1000")
                .replace("stale_after_days: 20", "stale_after_days: 2000");
            fs::write(path, manifest).unwrap();
            let before = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
            assert_eq!(before["decision_review"][0]["state"], "review-due");
            assert_eq!(
                before["decision_review"][0]["source_revision_mismatch"],
                true
            );
            assert_eq!(before["sources"][0]["state"], lifecycle);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn source_transition_invalid_evidence_fails_closed_for_both_lifecycles() {
        for lifecycle in ["revoked", "superseded"] {
            for transition in [
                "",
                "not-a-time",
                "2027-02-01T00:00:00Z",
                "2025-12-01T00:00:00Z",
            ] {
                let root = governed_pack("invalid-source-transition");
                let source_path = root.join(DEFAULT_DIR).join("sources.yaml");
                let mut source = fs::read_to_string(&source_path).unwrap();
                source = source.replace("lifecycle: current", &format!("lifecycle: {lifecycle}"));
                source = source.replace(
                    &format!("    lifecycle: {lifecycle}"),
                    &format!("    lifecycle: {lifecycle}\n    {lifecycle}_at: {transition}"),
                );
                fs::write(source_path, source).unwrap();
                let output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
                assert_eq!(output["sources"][0]["state"], "unknown");
                assert_eq!(
                    output["decision_review"][0]["source_revision_mismatch"],
                    true
                );
                assert!(
                    output["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"] == "source_revision_unverifiable")
                );
                let _ = fs::remove_dir_all(root);
            }
        }
    }

    #[test]
    fn replacement_group_validation_does_not_require_source_ledger() {
        for replacement in ["positioning-decision", "missing-group"] {
            let root = governed_pack("replacement-without-ledger");
            fs::remove_file(root.join(DEFAULT_DIR).join("sources.yaml")).unwrap();
            let path = root.join(DEFAULT_DIR).join("manifest.yaml");
            let manifest = fs::read_to_string(&path).unwrap().replace(
                "    reviewed_at: 2026-09-01T00:00:00Z",
                &format!(
                    "    reviewed_at: 2026-09-01T00:00:00Z\n    replacement_group: {replacement}"
                ),
            );
            fs::write(path, manifest).unwrap();
            let health = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
            assert!(
                health["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "decision_replacement_group_invalid")
            );
            let validation = crate::commands::health::validate_pack(&root).unwrap();
            assert_eq!(validation["valid"], false);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn invalid_decision_lifecycle_fails_closed_even_with_current_review() {
        let root = governed_pack("invalid-decision-lifecycle");
        let path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&path).unwrap().replace(
            "lifecycle: current\n    changed_at:",
            "lifecycle: invalid\n    changed_at:",
        );
        fs::write(path, manifest).unwrap();
        let output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(output["decision_review"][0]["state"], "review-due");
        assert!(
            output["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "decision_lifecycle_invalid")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_source_identity_is_unverifiable_and_strict_validation_error() {
        let root = governed_pack("duplicate-source-id");
        let sources_path = root.join(DEFAULT_DIR).join("sources.yaml");
        let original = fs::read_to_string(&sources_path).unwrap();
        let entries = original.split_once("sources:\n").unwrap().1;
        fs::write(
            sources_path,
            format!("format: mdp.sources.v0\nsources:\n{entries}{entries}"),
        )
        .unwrap();

        let health = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        let diagnostics = health["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "source_duplicate_id")
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "source_revision_unverifiable")
        );
        assert_eq!(
            health["decision_review"][0]["source_revision_mismatch"],
            true
        );

        let validation = crate::commands::health::validate_pack(&root).unwrap();
        assert_eq!(validation["valid"], false);
        assert!(
            validation["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "source_duplicate_id" && issue["severity"] == "error"
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
        for lifecycle in ["revoked", "superseded"] {
            for (value, diagnostic) in [
                (None, "transition_missing"),
                (Some("not-a-time"), "temporal_timestamp_invalid_or_future"),
                (
                    Some("2027-01-01T00:00:00Z"),
                    "temporal_timestamp_invalid_or_future",
                ),
                (
                    Some("2026-01-01T00:00:00Z"),
                    "temporal_transition_before_origin",
                ),
            ] {
                let root = governed_pack("invalid-transition");
                let path = root.join(DEFAULT_DIR).join("manifest.yaml");
                let transition =
                    value.map_or(String::new(), |v| format!("{lifecycle}_at: {v}\n    "));
                let manifest = fs::read_to_string(&path).unwrap().replace(
                    "lifecycle: current\n    changed_at:",
                    &format!("lifecycle: {lifecycle}\n    {transition}changed_at:"),
                );
                fs::write(&path, manifest).unwrap();
                let health = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
                assert_eq!(health["decision_review"][0]["state"], "review-due");
                assert!(
                    health["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|d| d["code"]
                            == if diagnostic == "transition_missing" && lifecycle == "revoked" {
                                "decision_revocation_transition_invalid"
                            } else if diagnostic == "transition_missing" {
                                "decision_supersession_transition_invalid"
                            } else {
                                diagnostic
                            })
                );
                if value == Some("not-a-time") && lifecycle == "revoked" {
                    let validation = crate::commands::health::validate_pack(&root).unwrap();
                    assert_eq!(validation["valid"], false);
                    assert!(crate::commands::health::doctor(&root)["status"] != "ready");
                }
                let _ = fs::remove_dir_all(root);
            }
        }
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
    fn malformed_typed_source_ledger_keeps_temporal_projection_available() {
        let root = governed_pack("malformed-ledger");
        fs::write(
            root.join(DEFAULT_DIR).join("sources.yaml"),
            "format: mdp.sources.v0\nsources:\n- id: source\n  temporal:\n    observed_at: [not-a-timestamp]\n",
        )
        .unwrap();
        let output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(output["contract"], CONTRACT);
        assert!(output["sources"].as_array().unwrap().is_empty());
        assert!(
            output["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| { d["code"] == "temporal_source_ledger_malformed" })
        );
        jsonschema::draft202012::validate(
            &crate::commands::schema(crate::cli::SchemaTarget::TemporalHealthV1),
            &output,
        )
        .unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typed_source_ledger_root_and_source_shapes_are_closed() {
        let root = governed_pack("closed-ledger");
        let path = root.join(DEFAULT_DIR).join("sources.yaml");
        fs::write(
            &path,
            "format: mdp.sources.v0\nunknown_root: true\nsources: []\n",
        )
        .unwrap();
        let root_unknown = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert!(
            root_unknown["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| { d["code"] == "temporal_source_ledger_malformed" })
        );

        fs::write(
            &path,
            "format: mdp.sources.v0\nsources:\n- id: source\n  temporal_typo: {}\n",
        )
        .unwrap();
        let source_unknown = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(source_unknown["contract"], CONTRACT);
        assert!(
            source_unknown["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| { d["code"] == "temporal_source_ledger_malformed" })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn decision_temporal_diagnostics_are_indexed_and_unique() {
        let root = governed_pack("indexed-diagnostics");
        let path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let original = fs::read_to_string(&path).unwrap();
        let manifest = original
            .clone()
            .replace(
                "reviewed_at: 2026-09-01T00:00:00Z",
                "reviewed_at: not-a-timestamp",
            )
            .replace("cadence: P10D", "cadence: P0D");
        fs::write(&path, manifest).unwrap();
        let output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        let diagnostics = output["diagnostics"].as_array().unwrap();
        let keys = diagnostics
            .iter()
            .map(|d| {
                (
                    d["code"].as_str().unwrap_or_default().to_owned(),
                    d["path"].as_str().unwrap_or_default().to_owned(),
                    d["message"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), diagnostics.len());
        assert!(diagnostics.iter().any(|d| {
            d["code"] == "temporal_timestamp_invalid_or_future"
                && d["path"] == "#/decision_groups/0/temporal/reviewed_at"
        }));
        assert!(diagnostics.iter().any(|d| {
            d["code"] == "temporal_cadence_invalid"
                && d["path"] == "#/decision_groups/0/review_policy/cadence"
        }));

        for (lifecycle, transition) in [("revoked", "revoked_at"), ("superseded", "superseded_at")]
        {
            let transition_manifest = original
                .replace(
                    "lifecycle: current\n    changed_at:",
                    &format!(
                        "lifecycle: {lifecycle}\n    {transition}: 2026-07-01T00:00:00Z\n    changed_at:"
                    ),
                );
            fs::write(&path, transition_manifest).unwrap();
            let transition_output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
            let matching = transition_output["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| {
                    d["code"] == "temporal_transition_before_origin"
                        && d["path"] == format!("#/decision_groups/0/temporal/{transition}")
                })
                .count();
            assert_eq!(matching, 1, "{lifecycle} transition should be unique");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn decision_group_identity_fields_require_meaningful_text() {
        let root = governed_pack("group-text-fields");
        let path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let original = fs::read_to_string(&path).unwrap();
        for (needle, replacement, code, field) in [
            (
                "id: positioning-decision",
                "id: '   '",
                "decision_group_id_empty",
                "id",
            ),
            (
                "label: Positioning decision",
                "label: '   '",
                "decision_group_label_empty",
                "label",
            ),
            (
                "jobs:\n  - prospect-fit-or-brief",
                "jobs:\n  - prospect-fit-or-brief\n  owner: '   '",
                "decision_group_owner_empty",
                "owner",
            ),
        ] {
            fs::write(&path, original.replace(needle, replacement)).unwrap();
            let output = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
            assert!(output["diagnostics"].as_array().unwrap().iter().any(|d| {
                d["code"] == code && d["path"] == format!("#/decision_groups/0/{field}")
            }));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn existing_nonregular_ledger_is_diagnostic_but_absent_optional_ledger_is_not() {
        let root = governed_pack("ledger-regularity");
        let ledger_path = root.join(DEFAULT_DIR).join("sources.yaml");
        fs::remove_file(&ledger_path).unwrap();
        fs::create_dir(&ledger_path).unwrap();
        let nonregular = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert_eq!(nonregular["contract"], CONTRACT);
        assert_eq!(
            nonregular["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["code"] == "temporal_source_ledger_malformed")
                .count(),
            1
        );

        fs::remove_dir(&ledger_path).unwrap();
        let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
        let manifest = fs::read_to_string(&manifest_path).unwrap();
        let without_groups = manifest.split("decision_groups:").next().unwrap();
        fs::write(manifest_path, without_groups).unwrap();
        let absent = temporal_health(&root, Some("2026-09-02T00:00:00Z")).unwrap();
        assert!(
            absent["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .all(|d| d["code"] != "temporal_source_ledger_malformed")
        );
        let _ = fs::remove_dir_all(root);
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
