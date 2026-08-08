use crate::models::{
    Card, CardKind, Entry, EntryConstraints, Manifest, ProductFoundationCondition,
    ProductFoundationConditionFact, ProductFoundationEntryRef, ProductFoundationFacet,
    ProductFoundationFacetKind,
};
use crate::pack_io::{read_card, resolve_pack_path};
use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProductFoundationStatus {
    Unassessed,
    Ready,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProductFoundationClassification {
    Required,
    Conditional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ResolvedFoundationEntry {
    pub(crate) card_id: String,
    pub(crate) entry_id: String,
    pub(crate) card_kind: CardKind,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) applies_to: Vec<String>,
    pub(crate) scope: BTreeMap<String, Vec<String>>,
    pub(crate) evidence: Vec<String>,
    pub(crate) avoid: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) exact_paragraphs: Option<usize>,
    #[serde(skip_serializing_if = "EntryConstraints::is_empty")]
    pub(crate) constraints: EntryConstraints,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ResolvedFoundationFacet {
    pub(crate) id: String,
    pub(crate) kind: ProductFoundationFacetKind,
    pub(crate) classification: ProductFoundationClassification,
    pub(crate) reason: String,
    pub(crate) entry_refs: Vec<ProductFoundationEntryRef>,
    pub(crate) gap_refs: Vec<ProductFoundationEntryRef>,
    pub(crate) entries: Vec<ResolvedFoundationEntry>,
    pub(crate) gaps: Vec<ResolvedFoundationEntry>,
    pub(crate) conflicts_with: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProductFoundationDiagnostic {
    pub(crate) code: String,
    pub(crate) severity: String,
    pub(crate) path: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProductFoundationResolution {
    pub(crate) job_id: String,
    pub(crate) status: ProductFoundationStatus,
    pub(crate) selected_facets: Vec<ResolvedFoundationFacet>,
    pub(crate) optional_facet_ids: Vec<String>,
    pub(crate) excluded_facet_ids: Vec<String>,
    pub(crate) untriggered_facet_ids: Vec<String>,
    pub(crate) diagnostics: Vec<ProductFoundationDiagnostic>,
}

impl ProductFoundationResolution {
    pub(crate) fn blocks_activation(&self) -> bool {
        self.status == ProductFoundationStatus::Blocked
    }
}

#[derive(Debug, Default)]
pub(crate) struct ProductFoundationIndex {
    entries: BTreeMap<(String, String), IndexedEntry>,
}

#[derive(Debug)]
struct IndexedEntry {
    card_kind: CardKind,
    title: String,
    body: String,
    applies_to: Vec<String>,
    scope: BTreeMap<String, Vec<String>>,
    evidence: Vec<String>,
    avoid: Vec<String>,
    exact_paragraphs: Option<usize>,
    constraints: EntryConstraints,
    metadata: BTreeMap<String, Value>,
}

impl ProductFoundationIndex {
    pub(crate) fn from_cards(cards: &[Card]) -> Self {
        let mut entries = BTreeMap::new();
        for card in cards {
            for entry in &card.entries {
                entries.insert(
                    (card.id.clone(), entry.id.clone()),
                    IndexedEntry::from_entry(card.kind.clone(), entry),
                );
            }
        }
        Self { entries }
    }

    fn resolve(&self, reference: &ProductFoundationEntryRef) -> Option<ResolvedFoundationEntry> {
        self.entries
            .get(&(reference.card_id.clone(), reference.entry_id.clone()))
            .map(|entry| ResolvedFoundationEntry {
                card_id: reference.card_id.clone(),
                entry_id: reference.entry_id.clone(),
                card_kind: entry.card_kind.clone(),
                title: entry.title.clone(),
                body: entry.body.clone(),
                applies_to: entry.applies_to.clone(),
                scope: entry.scope.clone(),
                evidence: entry.evidence.clone(),
                avoid: entry.avoid.clone(),
                exact_paragraphs: entry.exact_paragraphs,
                constraints: entry.constraints.clone(),
                metadata: entry.metadata.clone(),
            })
    }
}

impl IndexedEntry {
    fn from_entry(card_kind: CardKind, entry: &Entry) -> Self {
        Self {
            card_kind,
            title: entry.title.clone(),
            body: entry.body.clone(),
            applies_to: entry.applies_to.clone(),
            scope: entry.scope.clone(),
            evidence: entry.evidence.clone(),
            avoid: entry.avoid.clone(),
            exact_paragraphs: entry.exact_paragraphs,
            constraints: entry.constraints.clone(),
            metadata: entry.metadata.clone(),
        }
    }
}

pub(crate) fn resolve_product_foundation(
    manifest: &Manifest,
    index: &ProductFoundationIndex,
    job_id: &str,
) -> ProductFoundationResolution {
    let Some((job_index, job)) = manifest
        .jobs
        .iter()
        .enumerate()
        .find(|(_, job)| job.id == job_id)
    else {
        return unassessed(
            job_id,
            info_diagnostic(
                "product_foundation_job_unbound",
                ".mdp/manifest.yaml#/jobs".to_string(),
                format!("job {job_id} is not a canonical manifest job"),
            ),
        );
    };
    let Some(binding) = &job.product_foundation else {
        return unassessed(
            job_id,
            info_diagnostic(
                "product_foundation_not_bound",
                format!(".mdp/manifest.yaml#/jobs/{job_index}/product_foundation"),
                format!("job {job_id} has no product foundation binding"),
            ),
        );
    };
    let Some(registry) = manifest
        .profile
        .as_ref()
        .and_then(|profile| profile.product_foundation.as_ref())
    else {
        return ProductFoundationResolution {
            job_id: job_id.to_string(),
            status: ProductFoundationStatus::Blocked,
            selected_facets: Vec::new(),
            optional_facet_ids: sorted(binding.optional.clone()),
            excluded_facet_ids: sorted(binding.excluded.clone()),
            untriggered_facet_ids: Vec::new(),
            diagnostics: vec![diagnostic(
                "product_foundation_registry_missing",
                ".mdp/manifest.yaml#/profile/product_foundation".to_string(),
                "a bound job requires profile.product_foundation".to_string(),
            )],
        };
    };

    let facets = registry
        .facets
        .iter()
        .enumerate()
        .map(|(index, facet)| (facet.id.as_str(), (index, facet)))
        .collect::<BTreeMap<_, _>>();
    let mut selections = BTreeMap::new();
    for (required_index, facet_id) in binding.required.iter().enumerate() {
        selections.insert(
            facet_id.clone(),
            (
                ProductFoundationClassification::Required,
                "required by job binding".to_string(),
                format!(
                    ".mdp/manifest.yaml#/jobs/{job_index}/product_foundation/required/{required_index}"
                ),
            ),
        );
    }
    let mut diagnostics = Vec::new();
    let mut untriggered_facet_ids = Vec::new();
    for (conditional_index, conditional) in binding.conditional.iter().enumerate() {
        if conditional.when.fact == ProductFoundationConditionFact::Unknown {
            diagnostics.push(diagnostic(
                "product_foundation_condition_fact_unknown",
                format!(
                    ".mdp/manifest.yaml#/jobs/{job_index}/product_foundation/conditional/{conditional_index}/when/fact"
                ),
                "conditional fact must be manifest_id, profile_id, or job_id".to_string(),
            ));
            continue;
        }
        if condition_matches(manifest, job_id, &conditional.when) {
            selections.insert(
                conditional.facet_id.clone(),
                (
                    ProductFoundationClassification::Conditional,
                    format!(
                        "condition {} equals {}",
                        condition_fact_name(&conditional.when.fact),
                        conditional.when.equals
                    ),
                    format!(
                        ".mdp/manifest.yaml#/jobs/{job_index}/product_foundation/conditional/{conditional_index}/facet_id"
                    ),
                ),
            );
        } else {
            untriggered_facet_ids.push(conditional.facet_id.clone());
        }
    }

    let no_selected_authority = selections.is_empty();
    if no_selected_authority {
        diagnostics.push(diagnostic(
            "product_foundation_selected_authority_empty",
            format!(".mdp/manifest.yaml#/jobs/{job_index}/product_foundation"),
            format!("job {job_id} selects no required or triggered conditional facets"),
        ));
    }
    let mut selected_facets = Vec::new();
    for (facet_id, (classification, reason, selection_path)) in selections {
        let Some((facet_index, facet)) = facets.get(facet_id.as_str()) else {
            diagnostics.push(diagnostic(
                "profile_job_product_foundation_facet_missing",
                selection_path,
                format!("selected product foundation facet {facet_id} is missing"),
            ));
            continue;
        };
        selected_facets.push(resolve_facet(
            facet,
            *facet_index,
            classification,
            reason,
            index,
            &mut diagnostics,
        ));
    }

    let selected_ids = selected_facets
        .iter()
        .map(|facet| facet.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen_conflicts = BTreeSet::new();
    for facet in &selected_facets {
        for conflict in &facet.conflicts_with {
            if selected_ids.contains(conflict.as_str()) {
                let pair = if facet.id < *conflict {
                    (facet.id.clone(), conflict.clone())
                } else {
                    (conflict.clone(), facet.id.clone())
                };
                if seen_conflicts.insert(pair.clone()) {
                    let (facet_index, source_facet) = facets
                        .get(facet.id.as_str())
                        .expect("selected facet came from registry");
                    let conflict_index = source_facet
                        .conflicts_with
                        .iter()
                        .position(|candidate| candidate == conflict)
                        .unwrap_or_default();
                    diagnostics.push(diagnostic(
                        "product_foundation_selected_conflict",
                        format!(
                            ".mdp/manifest.yaml#/profile/product_foundation/facets/{facet_index}/conflicts_with/{conflict_index}"
                        ),
                        format!("selected facets {} and {} conflict", pair.0, pair.1),
                    ));
                }
            }
        }
    }
    diagnostics.sort_by(|left, right| {
        (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
    });

    ProductFoundationResolution {
        job_id: job_id.to_string(),
        status: if diagnostics.is_empty() {
            ProductFoundationStatus::Ready
        } else {
            ProductFoundationStatus::Blocked
        },
        selected_facets,
        optional_facet_ids: sorted(binding.optional.clone()),
        excluded_facet_ids: sorted(binding.excluded.clone()),
        untriggered_facet_ids: sorted(untriggered_facet_ids),
        diagnostics,
    }
}

pub(crate) fn resolve_product_foundation_for_pack(
    root: &Path,
    manifest: &Manifest,
    job_id: &str,
) -> Result<ProductFoundationResolution> {
    let cards = manifest
        .cards
        .iter()
        .map(|card_ref| read_card(&resolve_pack_path(root, &card_ref.path)?))
        .collect::<Result<Vec<_>>>()?;
    let index = ProductFoundationIndex::from_cards(&cards);
    Ok(resolve_product_foundation(manifest, &index, job_id))
}

fn resolve_facet(
    facet: &ProductFoundationFacet,
    facet_index: usize,
    classification: ProductFoundationClassification,
    reason: String,
    index: &ProductFoundationIndex,
    diagnostics: &mut Vec<ProductFoundationDiagnostic>,
) -> ResolvedFoundationFacet {
    if facet.entries.is_empty() && facet.gaps.is_empty() {
        diagnostics.push(diagnostic(
            "product_foundation_selected_facet_empty",
            format!(".mdp/manifest.yaml#/profile/product_foundation/facets/{facet_index}"),
            format!("selected facet {} has no authoritative entries", facet.id),
        ));
    }
    if !facet.gaps.is_empty() {
        diagnostics.push(diagnostic(
            "product_foundation_selected_facet_has_gaps",
            format!(".mdp/manifest.yaml#/profile/product_foundation/facets/{facet_index}/gaps"),
            format!("selected facet {} contains explicit gaps", facet.id),
        ));
    }
    let entry_refs = facet.entries.clone();
    let gap_refs = facet.gaps.clone();
    let entries = resolve_refs(
        facet,
        facet_index,
        &entry_refs,
        "entries",
        index,
        diagnostics,
    );
    let gaps = resolve_refs(facet, facet_index, &gap_refs, "gaps", index, diagnostics);
    let mut conflicts_with = facet.conflicts_with.clone();
    conflicts_with.sort();
    ResolvedFoundationFacet {
        id: facet.id.clone(),
        kind: facet.kind.clone(),
        classification,
        reason,
        entry_refs,
        gap_refs,
        entries,
        gaps,
        conflicts_with,
    }
}

fn resolve_refs(
    facet: &ProductFoundationFacet,
    facet_index: usize,
    refs: &[ProductFoundationEntryRef],
    class: &str,
    index: &ProductFoundationIndex,
    diagnostics: &mut Vec<ProductFoundationDiagnostic>,
) -> Vec<ResolvedFoundationEntry> {
    let mut resolved = Vec::new();
    for (reference_index, reference) in refs.iter().enumerate() {
        if let Some(entry) = index.resolve(reference) {
            resolved.push(entry);
        } else {
            diagnostics.push(diagnostic(
                "product_foundation_selected_reference_dangling",
                format!(
                    ".mdp/manifest.yaml#/profile/product_foundation/facets/{facet_index}/{class}/{reference_index}"
                ),
                format!(
                    "selected facet {} references missing {}#{}",
                    facet.id, reference.card_id, reference.entry_id
                ),
            ));
        }
    }
    resolved
}

fn condition_matches(
    manifest: &Manifest,
    job_id: &str,
    condition: &ProductFoundationCondition,
) -> bool {
    let actual = match condition.fact {
        ProductFoundationConditionFact::ManifestId => Some(manifest.id.as_str()),
        ProductFoundationConditionFact::ProfileId => {
            manifest.profile.as_ref().map(|profile| profile.id.as_str())
        }
        ProductFoundationConditionFact::JobId => Some(job_id),
        ProductFoundationConditionFact::Unknown => None,
    };
    actual == Some(condition.equals.as_str())
}

fn condition_fact_name(fact: &ProductFoundationConditionFact) -> &'static str {
    match fact {
        ProductFoundationConditionFact::ManifestId => "manifest_id",
        ProductFoundationConditionFact::ProfileId => "profile_id",
        ProductFoundationConditionFact::JobId => "job_id",
        ProductFoundationConditionFact::Unknown => "unknown",
    }
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

fn unassessed(
    job_id: &str,
    diagnostic: ProductFoundationDiagnostic,
) -> ProductFoundationResolution {
    ProductFoundationResolution {
        job_id: job_id.to_string(),
        status: ProductFoundationStatus::Unassessed,
        selected_facets: Vec::new(),
        optional_facet_ids: Vec::new(),
        excluded_facet_ids: Vec::new(),
        untriggered_facet_ids: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

fn diagnostic(code: &str, path: String, message: String) -> ProductFoundationDiagnostic {
    ProductFoundationDiagnostic {
        code: code.to_string(),
        severity: "error".to_string(),
        path,
        message,
    }
}

fn info_diagnostic(code: &str, path: String, message: String) -> ProductFoundationDiagnostic {
    ProductFoundationDiagnostic {
        code: code.to_string(),
        severity: "info".to_string(),
        path,
        message,
    }
}

pub(crate) fn resolution_json(resolution: &ProductFoundationResolution) -> Value {
    serde_json::to_value(resolution).unwrap_or_else(|_| {
        json!({
            "job_id": resolution.job_id,
            "status": "blocked",
            "diagnostics": [{
                "code": "product_foundation_serialization_failed",
                "severity": "error",
                "path": ".mdp/manifest.yaml",
                "message": "product foundation resolution could not be serialized"
            }]
        })
    })
}

pub(crate) fn apply_validation_errors(
    resolution: &mut ProductFoundationResolution,
    issues: &[Value],
) {
    apply_matching_validation_errors(resolution, issues, |_| true);
}

pub(crate) fn apply_validation_errors_for_job(
    resolution: &mut ProductFoundationResolution,
    manifest: &Manifest,
    issues: &[Value],
) {
    let relevance = resolution.clone();
    apply_matching_validation_errors(resolution, issues, |issue| {
        product_foundation_issue_applies_to_job(manifest, &relevance, issue)
    });
}

fn apply_matching_validation_errors(
    resolution: &mut ProductFoundationResolution,
    issues: &[Value],
    applies: impl Fn(&Value) -> bool,
) {
    if resolution.status == ProductFoundationStatus::Unassessed {
        return;
    }

    let mut added = false;
    for issue in issues {
        if issue["severity"] != "error" {
            continue;
        }
        let Some(code) = issue["code"].as_str() else {
            continue;
        };
        if !is_product_foundation_validation_code(code) || !applies(issue) {
            continue;
        }
        let path = issue["path"]
            .as_str()
            .unwrap_or(".mdp/manifest.yaml")
            .to_string();
        let message = issue["message"]
            .as_str()
            .unwrap_or("product foundation validation failed")
            .to_string();
        if resolution
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code && diagnostic.path == path)
        {
            continue;
        }
        resolution.diagnostics.push(ProductFoundationDiagnostic {
            code: code.to_string(),
            severity: "error".to_string(),
            path,
            message,
        });
        added = true;
    }

    if added {
        resolution.status = ProductFoundationStatus::Blocked;
        resolution.diagnostics.sort_by(|left, right| {
            (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
        });
    }
}

pub(crate) fn validation_errors_block_job(
    manifest: &Manifest,
    resolution: &ProductFoundationResolution,
    issues: &[Value],
) -> bool {
    issues.iter().any(|issue| {
        if issue["severity"] != "error" {
            return false;
        }
        let Some(code) = issue["code"].as_str() else {
            return true;
        };
        !is_product_foundation_validation_code(code)
            || product_foundation_issue_applies_to_job(manifest, resolution, issue)
    })
}

pub(crate) fn validation_issues_for_job(
    manifest: &Manifest,
    resolution: &ProductFoundationResolution,
    issues: &[Value],
) -> Vec<Value> {
    issues
        .iter()
        .filter(|issue| {
            issue["code"].as_str().is_none_or(|code| {
                !is_product_foundation_validation_code(code)
                    || product_foundation_issue_applies_to_job(manifest, resolution, issue)
            })
        })
        .cloned()
        .collect()
}

fn product_foundation_issue_applies_to_job(
    manifest: &Manifest,
    resolution: &ProductFoundationResolution,
    issue: &Value,
) -> bool {
    if resolution.status == ProductFoundationStatus::Unassessed {
        return false;
    }

    let path = issue["path"].as_str().unwrap_or(".mdp/manifest.yaml");
    let Some(job_index) = manifest
        .jobs
        .iter()
        .position(|job| job.id == resolution.job_id)
    else {
        return false;
    };
    let job_prefix = format!(".mdp/manifest.yaml#/jobs/{job_index}/product_foundation");
    if path.starts_with(&job_prefix) {
        return true;
    }
    if path.starts_with(".mdp/manifest.yaml#/jobs/") {
        return false;
    }

    let facet_prefix = ".mdp/manifest.yaml#/profile/product_foundation/facets/";
    if let Some(suffix) = path.strip_prefix(facet_prefix) {
        let Some(index) = suffix
            .split('/')
            .next()
            .and_then(|value| value.parse::<usize>().ok())
        else {
            return true;
        };
        let selected_ids = resolution
            .selected_facets
            .iter()
            .map(|facet| facet.id.as_str())
            .collect::<BTreeSet<_>>();
        return manifest
            .profile
            .as_ref()
            .and_then(|profile| profile.product_foundation.as_ref())
            .and_then(|registry| registry.facets.get(index))
            .is_some_and(|facet| selected_ids.contains(facet.id.as_str()));
    }

    path.starts_with(".mdp/manifest.yaml#/profile/product_foundation")
        || path == ".mdp/manifest.yaml"
}

fn is_product_foundation_validation_code(code: &str) -> bool {
    code.starts_with("product_foundation_")
        || code.starts_with("profile_job_product_foundation_")
        || code.starts_with("manifest_product_foundation_")
        || code.starts_with("manifest_profile_job_product_foundation_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ProductFoundationBinding, ProductFoundationConditionalFacet, ProductFoundationRegistry,
    };
    use crate::starter::starter_manifest;

    fn reference(card_id: &str, entry_id: &str) -> ProductFoundationEntryRef {
        ProductFoundationEntryRef {
            card_id: card_id.to_string(),
            entry_id: entry_id.to_string(),
        }
    }

    fn entry(id: &str, body: &str) -> Entry {
        Entry {
            id: id.to_string(),
            title: id.to_string(),
            body: body.to_string(),
            applies_to: Vec::new(),
            scope: BTreeMap::new(),
            evidence: vec![format!("evidence-{id}")],
            avoid: Vec::new(),
            exact_paragraphs: None,
            constraints: EntryConstraints::default(),
            metadata: BTreeMap::new(),
        }
    }

    fn card(id: &str, kind: CardKind, entries: Vec<Entry>) -> Card {
        Card {
            id: id.to_string(),
            kind,
            title: id.to_string(),
            description: String::new(),
            personas: Vec::new(),
            tags: Vec::new(),
            entries,
        }
    }

    fn facet(id: &str, entries: Vec<ProductFoundationEntryRef>) -> ProductFoundationFacet {
        ProductFoundationFacet {
            id: id.to_string(),
            kind: ProductFoundationFacetKind::ProductIdentity,
            entries,
            gaps: Vec::new(),
            conflicts_with: Vec::new(),
        }
    }

    fn binding(required: &[&str]) -> ProductFoundationBinding {
        ProductFoundationBinding {
            required: required.iter().map(|id| (*id).to_string()).collect(),
            conditional: Vec::new(),
            optional: Vec::new(),
            excluded: Vec::new(),
        }
    }

    fn manifest_with_foundation(facets: Vec<ProductFoundationFacet>) -> Manifest {
        let mut manifest = starter_manifest("Test", "test", "gtm");
        manifest
            .profile
            .as_mut()
            .expect("starter profile")
            .product_foundation = Some(ProductFoundationRegistry { facets });
        manifest
    }

    #[test]
    fn two_jobs_resolve_disjoint_required_facets_in_stable_order() {
        let mut alpha = entry("alpha", "Alpha");
        alpha.exact_paragraphs = Some(2);
        let cards = vec![card(
            "positioning",
            CardKind::Positioning,
            vec![alpha, entry("beta", "Beta")],
        )];
        let index = ProductFoundationIndex::from_cards(&cards);
        let mut manifest = manifest_with_foundation(vec![
            facet("zeta", vec![reference("positioning", "beta")]),
            facet("alpha", vec![reference("positioning", "alpha")]),
        ]);
        manifest.jobs[0].product_foundation = Some(binding(&["zeta", "alpha"]));
        manifest.jobs[1].product_foundation = Some(binding(&["zeta"]));

        let first = resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id);
        let second = resolve_product_foundation(&manifest, &index, &manifest.jobs[1].id);

        assert_eq!(first.status, ProductFoundationStatus::Ready);
        assert_eq!(
            first
                .selected_facets
                .iter()
                .map(|facet| facet.id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        assert_eq!(
            second
                .selected_facets
                .iter()
                .map(|facet| facet.id.as_str())
                .collect::<Vec<_>>(),
            vec!["zeta"]
        );
        assert_eq!(first.selected_facets[0].entries[0].body, "Alpha");
        assert_eq!(
            first.selected_facets[0].entries[0].exact_paragraphs,
            Some(2)
        );
        assert_eq!(
            resolution_json(&first)["selected_facets"][0]["entries"][0]["exact_paragraphs"],
            2
        );
    }

    #[test]
    fn unbound_legacy_and_unknown_jobs_are_unassessed() {
        let mut manifest = starter_manifest("Test", "test", "gtm");
        manifest
            .profile
            .as_mut()
            .expect("starter profile")
            .product_foundation = None;
        for job in &mut manifest.jobs {
            job.product_foundation = None;
        }
        let index = ProductFoundationIndex::default();

        assert_eq!(
            resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id).status,
            ProductFoundationStatus::Unassessed
        );
        assert_eq!(
            resolve_product_foundation(&manifest, &index, "free-text job").status,
            ProductFoundationStatus::Unassessed
        );
    }

    #[test]
    fn opted_in_binding_without_selected_authority_blocks() {
        let mut manifest = manifest_with_foundation(vec![facet(
            "optional",
            vec![reference("positioning", "optional")],
        )]);
        let job_id = manifest.jobs[0].id.clone();

        for binding in [
            ProductFoundationBinding::default(),
            ProductFoundationBinding {
                optional: vec!["optional".to_string()],
                ..ProductFoundationBinding::default()
            },
            ProductFoundationBinding {
                conditional: vec![ProductFoundationConditionalFacet {
                    facet_id: "optional".to_string(),
                    when: ProductFoundationCondition {
                        fact: ProductFoundationConditionFact::JobId,
                        equals: "different-job".to_string(),
                    },
                }],
                ..ProductFoundationBinding::default()
            },
        ] {
            manifest.jobs[0].product_foundation = Some(binding);

            let result =
                resolve_product_foundation(&manifest, &ProductFoundationIndex::default(), &job_id);

            assert_eq!(result.status, ProductFoundationStatus::Blocked);
            assert!(result.selected_facets.is_empty());
            assert!(result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "product_foundation_selected_authority_empty"
                    && diagnostic.path == ".mdp/manifest.yaml#/jobs/0/product_foundation"
            }));
        }
    }

    #[test]
    fn empty_required_facet_and_selected_gap_block_with_exact_refs() {
        let cards = vec![card(
            "gaps",
            CardKind::Gaps,
            vec![entry("missing-proof", "Proof is not established")],
        )];
        let index = ProductFoundationIndex::from_cards(&cards);
        let mut gap_facet = facet("proof", Vec::new());
        gap_facet.kind = ProductFoundationFacetKind::Gaps;
        gap_facet.gaps = vec![reference("gaps", "missing-proof")];
        let mut manifest = manifest_with_foundation(vec![facet("empty", Vec::new()), gap_facet]);
        manifest.jobs[0].product_foundation = Some(binding(&["empty", "proof"]));

        let result = resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id);

        assert_eq!(result.status, ProductFoundationStatus::Blocked);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "product_foundation_selected_facet_empty" })
        );
        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "product_foundation_selected_facet_has_gaps"
            })
        );
        assert_eq!(
            result.selected_facets[1].gap_refs[0].entry_id,
            "missing-proof"
        );
        assert_eq!(
            result.selected_facets[1].gaps[0].body,
            "Proof is not established"
        );
    }

    #[test]
    fn dangling_selected_reference_blocks() {
        let index = ProductFoundationIndex::default();
        let mut manifest = manifest_with_foundation(vec![facet(
            "identity",
            vec![reference("positioning", "missing")],
        )]);
        manifest.jobs[0].product_foundation = Some(binding(&["identity"]));

        let result = resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id);

        assert_eq!(result.status, ProductFoundationStatus::Blocked);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "product_foundation_selected_reference_dangling"
                && diagnostic.message.contains("positioning#missing")
        }));
    }

    #[test]
    fn selected_conflict_blocks_but_optional_excluded_and_untriggered_do_not() {
        let cards = vec![card(
            "positioning",
            CardKind::Positioning,
            vec![entry("one", "One"), entry("two", "Two")],
        )];
        let index = ProductFoundationIndex::from_cards(&cards);
        let mut first = facet("first", vec![reference("positioning", "one")]);
        first.conflicts_with = vec!["second".to_string(), "optional".to_string()];
        let second = facet("second", vec![reference("positioning", "two")]);
        let mut optional = facet("optional", Vec::new());
        optional.conflicts_with = vec!["excluded".to_string()];
        let excluded = facet("excluded", Vec::new());
        let conditional = facet("conditional", Vec::new());
        let mut manifest =
            manifest_with_foundation(vec![first, second, optional, excluded, conditional]);
        manifest.jobs[0].product_foundation = Some(ProductFoundationBinding {
            required: vec!["first".to_string(), "second".to_string()],
            conditional: vec![ProductFoundationConditionalFacet {
                facet_id: "conditional".to_string(),
                when: ProductFoundationCondition {
                    fact: ProductFoundationConditionFact::ManifestId,
                    equals: "not-this-pack".to_string(),
                },
            }],
            optional: vec!["optional".to_string()],
            excluded: vec!["excluded".to_string()],
        });

        let result = resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id);

        assert_eq!(result.status, ProductFoundationStatus::Blocked);
        assert_eq!(result.optional_facet_ids, vec!["optional"]);
        assert_eq!(result.excluded_facet_ids, vec!["excluded"]);
        assert_eq!(result.untriggered_facet_ids, vec!["conditional"]);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "product_foundation_selected_conflict")
                .count(),
            1
        );

        manifest.jobs[0].product_foundation = Some(ProductFoundationBinding {
            required: vec!["first".to_string()],
            conditional: vec![ProductFoundationConditionalFacet {
                facet_id: "conditional".to_string(),
                when: ProductFoundationCondition {
                    fact: ProductFoundationConditionFact::JobId,
                    equals: "different-job".to_string(),
                },
            }],
            optional: vec!["optional".to_string()],
            excluded: vec!["second".to_string(), "excluded".to_string()],
        });
        let non_blocking = resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id);
        assert_eq!(non_blocking.status, ProductFoundationStatus::Ready);
    }

    #[test]
    fn static_manifest_profile_and_job_predicates_select_conditionals() {
        let cards = vec![card(
            "positioning",
            CardKind::Positioning,
            vec![entry("one", "One")],
        )];
        let index = ProductFoundationIndex::from_cards(&cards);
        let mut manifest = manifest_with_foundation(vec![
            facet("manifest", vec![reference("positioning", "one")]),
            facet("profile", vec![reference("positioning", "one")]),
            facet("job", vec![reference("positioning", "one")]),
            facet("false", Vec::new()),
        ]);
        let job_id = manifest.jobs[0].id.clone();
        manifest.jobs[0].product_foundation = Some(ProductFoundationBinding {
            required: Vec::new(),
            conditional: vec![
                ProductFoundationConditionalFacet {
                    facet_id: "manifest".to_string(),
                    when: ProductFoundationCondition {
                        fact: ProductFoundationConditionFact::ManifestId,
                        equals: manifest.id.clone(),
                    },
                },
                ProductFoundationConditionalFacet {
                    facet_id: "profile".to_string(),
                    when: ProductFoundationCondition {
                        fact: ProductFoundationConditionFact::ProfileId,
                        equals: manifest.profile.as_ref().expect("profile").id.clone(),
                    },
                },
                ProductFoundationConditionalFacet {
                    facet_id: "job".to_string(),
                    when: ProductFoundationCondition {
                        fact: ProductFoundationConditionFact::JobId,
                        equals: job_id.clone(),
                    },
                },
                ProductFoundationConditionalFacet {
                    facet_id: "false".to_string(),
                    when: ProductFoundationCondition {
                        fact: ProductFoundationConditionFact::JobId,
                        equals: "other".to_string(),
                    },
                },
            ],
            optional: Vec::new(),
            excluded: Vec::new(),
        });

        let result = resolve_product_foundation(&manifest, &index, &job_id);

        assert_eq!(result.status, ProductFoundationStatus::Ready);
        assert_eq!(
            result
                .selected_facets
                .iter()
                .map(|facet| facet.id.as_str())
                .collect::<Vec<_>>(),
            vec!["job", "manifest", "profile"]
        );
        assert_eq!(result.untriggered_facet_ids, vec!["false"]);
    }

    #[test]
    fn unknown_conditional_fact_blocks_direct_resolution() {
        let cards = vec![card(
            "positioning",
            CardKind::Positioning,
            vec![entry("one", "One")],
        )];
        let index = ProductFoundationIndex::from_cards(&cards);
        let mut manifest = manifest_with_foundation(vec![facet(
            "identity",
            vec![reference("positioning", "one")],
        )]);
        let job_id = manifest.jobs[0].id.clone();
        manifest.jobs[0].product_foundation = Some(ProductFoundationBinding {
            required: vec!["identity".to_string()],
            conditional: vec![ProductFoundationConditionalFacet {
                facet_id: "identity".to_string(),
                when: ProductFoundationCondition {
                    fact: ProductFoundationConditionFact::Unknown,
                    equals: job_id.clone(),
                },
            }],
            optional: Vec::new(),
            excluded: Vec::new(),
        });

        let result = resolve_product_foundation(&manifest, &index, &job_id);

        assert_eq!(result.status, ProductFoundationStatus::Blocked);
        assert!(result.untriggered_facet_ids.is_empty());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "product_foundation_condition_fact_unknown"
                && diagnostic.path
                    == ".mdp/manifest.yaml#/jobs/0/product_foundation/conditional/0/when/fact"
        }));
    }

    #[test]
    fn job_aware_validation_ignores_other_job_foundation_errors() {
        let cards = vec![card(
            "positioning",
            CardKind::Positioning,
            vec![entry("one", "One")],
        )];
        let index = ProductFoundationIndex::from_cards(&cards);
        let mut manifest = manifest_with_foundation(vec![facet(
            "identity",
            vec![reference("positioning", "one")],
        )]);
        manifest.jobs[0].product_foundation = Some(binding(&["identity"]));
        manifest.jobs[1].product_foundation = Some(binding(&["identity"]));
        let mut selected = resolve_product_foundation(&manifest, &index, &manifest.jobs[0].id);
        let unrelated_issue = json!({
            "code": "product_foundation_condition_fact_unknown",
            "severity": "error",
            "path": ".mdp/manifest.yaml#/jobs/1/product_foundation/conditional/0/when/fact",
            "message": "conditional fact is invalid"
        });

        apply_validation_errors_for_job(
            &mut selected,
            &manifest,
            std::slice::from_ref(&unrelated_issue),
        );

        assert_eq!(selected.status, ProductFoundationStatus::Ready);
        assert!(!validation_errors_block_job(
            &manifest,
            &selected,
            std::slice::from_ref(&unrelated_issue)
        ));

        let selected_issue = json!({
            "code": "product_foundation_condition_fact_unknown",
            "severity": "error",
            "path": ".mdp/manifest.yaml#/jobs/0/product_foundation/conditional/0/when/fact",
            "message": "conditional fact is invalid"
        });
        apply_validation_errors_for_job(
            &mut selected,
            &manifest,
            std::slice::from_ref(&selected_issue),
        );
        assert_eq!(selected.status, ProductFoundationStatus::Blocked);
        assert!(validation_errors_block_job(
            &manifest,
            &selected,
            std::slice::from_ref(&selected_issue)
        ));
    }
}
