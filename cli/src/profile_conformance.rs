//! Test-only contract shared by the shipped profile registries.

use crate::artifact_hash::pack_content_sha256;
use crate::commands::evals::eval_pack;
use crate::commands::health::{profile_activation_decision, validate_pack};
use crate::models::Manifest;
use crate::pack_io::read_manifest;
use crate::primitives::PrimitiveId;
use crate::routing::route_budget_preflight;
use crate::skill_catalog::{PROFILE_DESCRIPTORS, is_packaged_skill};
use crate::template_registry::{descriptors, lookup};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const BASIC_DIGEST: &str = "7d7acb7abb1954f2782b3eb9aa09d5730d7700b9d49936cd957942ccebf55e7d";
const PROPOSAL_DIGEST: &str = "2bcdaeefe2334215cde0e68aba650abd1d69806825feadfadde3c1333b9bfad9";
const FORBIDDEN: &[&str] = &[
    "prospect",
    "lead",
    "cta",
    "hook",
    "pain",
    "outbound",
    "proposal",
    "rfp",
    "bid",
    "compliance",
    "pursuit",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Subject {
    id: String,
    registered: bool,
    primitive_ids: Vec<String>,
    mappings: BTreeMap<String, Vec<String>>,
    jobs: Vec<Job>,
    input_contracts: Vec<String>,
    route_ready: bool,
    eval_categories: Vec<String>,
    activation_ready: bool,
    health_valid: bool,
    eval_valid: bool,
    output_mapping: bool,
    gap_mapping: bool,
    eval_mapping: bool,
    receipt_trace_clean_replay: bool,
    authored_digest: Option<String>,
    #[serde(default)]
    text: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Job {
    id: String,
    skill_id: String,
    required_primitives: Vec<String>,
    input_contracts: Vec<String>,
    max_entries: usize,
    max_bytes: usize,
}

fn finding(subject: &str, check: &str) -> String {
    format!("[{subject}:{check}]")
}

fn check_subject(subject: &Subject) -> Vec<String> {
    let mut out = Vec::new();
    let expected = PrimitiveId::names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if subject.primitive_ids != expected {
        out.push(finding(&subject.id, "primitive-coverage"));
    }
    if subject.mappings.len() != expected.len()
        || subject
            .mappings
            .keys()
            .any(|key| !expected.iter().any(|x| x == key))
    {
        out.push(finding(&subject.id, "primitive-map-keys"));
    }
    if subject.mappings.values().any(Vec::is_empty) {
        out.push(finding(&subject.id, "primitive-mappings"));
    }
    let mut jobs = BTreeSet::new();
    for job in &subject.jobs {
        if !jobs.insert(&job.id) {
            out.push(finding(&subject.id, "job-ownership"));
        }
        if job.skill_id.is_empty() || !job.required_primitives.iter().all(|p| expected.contains(p))
        {
            out.push(finding(&subject.id, "job-ownership"));
        }
        if job.input_contracts.is_empty()
            || !job
                .input_contracts
                .iter()
                .all(|c| subject.input_contracts.contains(c))
        {
            out.push(finding(&subject.id, "input-contracts"));
        }
        if job.max_entries == 0 || job.max_bytes == 0 {
            out.push(finding(&subject.id, "route-budget"));
        }
    }
    if subject.jobs.is_empty() || subject.input_contracts.is_empty() {
        out.push(finding(&subject.id, "input-contracts"));
    }
    if !subject.route_ready {
        out.push(finding(&subject.id, "route-budget"));
    }
    let categories = subject.eval_categories.iter().collect::<BTreeSet<_>>();
    let required_categories = [
        "proceed",
        "insufficient-context",
        "refusal",
        "unsafe-output",
        "job-routing",
    ];
    if required_categories
        .iter()
        .any(|category| !categories.iter().any(|value| value.as_str() == *category))
    {
        out.push(finding(&subject.id, "eval-categories"));
    }
    if !subject.activation_ready {
        out.push(finding(&subject.id, "activation"));
    }
    if !subject.health_valid {
        out.push(finding(&subject.id, "health"));
    }
    if !subject.eval_valid {
        out.push(finding(&subject.id, "eval"));
    }
    if !subject.output_mapping {
        out.push(finding(&subject.id, "output-mapping"));
    }
    if !subject.gap_mapping {
        out.push(finding(&subject.id, "gap-mapping"));
    }
    if !subject.eval_mapping {
        out.push(finding(&subject.id, "eval-mapping"));
    }
    if !subject.receipt_trace_clean_replay {
        out.push(finding(&subject.id, "runtime-authority"));
    }
    if subject.id == "neutral"
        && subject.text.iter().any(|value| {
            FORBIDDEN.iter().any(|term| {
                value
                    .to_ascii_lowercase()
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|token| token == *term)
            })
        })
    {
        out.push(finding(&subject.id, "vocabulary-isolation"));
    }
    out
}

fn real_subject(profile_id: &str) -> Subject {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates");
    let descriptor = PROFILE_DESCRIPTORS
        .iter()
        .find(|d| d.profile_id == profile_id)
        .expect("registered profile");
    let template = lookup(descriptor.template_id).expect("registered template");
    let pack = root.join(template.asset_root);
    let manifest: Manifest = read_manifest(&pack).expect("canonical manifest");
    let validation = validate_pack(&pack).expect("canonical pack validates");
    let eval = eval_pack(&pack).expect("canonical evals run");
    let budget = route_budget_preflight(&pack, &manifest).expect("canonical routes preflight");
    for job in &manifest.jobs {
        crate::commands::requirements::requirements(&pack, &job.id)
            .unwrap_or_else(|error| panic!("requirements for {} failed: {error}", job.id));
    }
    let activation =
        profile_activation_decision(&validation, manifest.profile_eval.blocks_activation(), None);
    let mappings = manifest
        .primitive_map
        .iter()
        .map(|(key, value)| {
            let mut values = value.cards.clone();
            values.extend(value.prompts.clone());
            values.extend(value.input_contracts.clone());
            values.extend(value.jobs.clone());
            values.extend(value.evals.clone());
            (key.clone(), values)
        })
        .collect();
    let jobs = manifest
        .jobs
        .iter()
        .map(|job| Job {
            id: job.id.clone(),
            skill_id: job.skill_id.clone(),
            required_primitives: job.required_primitives.clone(),
            input_contracts: job.input_contracts.clone(),
            max_entries: job.context_budget.as_ref().map_or(0, |b| b.max_entries),
            max_bytes: job.context_budget.as_ref().map_or(0, |b| b.max_bytes),
        })
        .collect();
    Subject {
        id: profile_id.to_string(),
        registered: true,
        primitive_ids: manifest.required_primitives.clone(),
        mappings,
        jobs,
        input_contracts: manifest
            .input_contracts
            .iter()
            .map(|c| c.id.clone())
            .collect(),
        route_ready: budget["valid"] == true,
        eval_categories: manifest.profile_eval.required_categories.clone(),
        activation_ready: activation["activation_ready"] == true,
        health_valid: validation["valid"] == true,
        eval_valid: eval["valid"] == true,
        output_mapping: manifest
            .primitive_map
            .get("output-contracts")
            .is_some_and(|m| !m.cards.is_empty()),
        gap_mapping: manifest
            .primitive_map
            .get("gaps")
            .is_some_and(|m| !m.cards.is_empty()),
        eval_mapping: manifest
            .primitive_map
            .get("evals")
            .is_some_and(|m| !m.evals.is_empty()),
        receipt_trace_clean_replay: true,
        authored_digest: Some(pack_content_sha256(&pack).expect("canonical digest")),
        text: vec![],
    }
}

fn neutral_subject() -> Subject {
    serde_json::from_str(include_str!(
        "../tests/fixtures/profile-conformance/neutral-profile.json"
    ))
    .expect("neutral fixture")
}

#[test]
fn both_shipped_profiles_pass_one_shared_contract() {
    let subjects = [real_subject("gtm"), real_subject("proposal")];
    let mut findings = Vec::new();
    for subject in &subjects {
        let expected = if subject.id == "gtm" {
            BASIC_DIGEST
        } else {
            PROPOSAL_DIGEST
        };
        let mut current = check_subject(subject);
        if subject.authored_digest.as_deref() != Some(expected) {
            current.push(finding(&subject.id, "pack-digest"));
        }
        if current.is_empty() {
            println!("{}: PASS", subject.id);
        }
        findings.extend(current);
    }
    assert!(findings.is_empty(), "conformance findings: {findings:?}");
}

#[test]
fn neutral_fixture_passes_core_but_is_not_registered_or_packaged() {
    let subject = neutral_subject();
    assert!(!subject.registered);
    assert!(check_subject(&subject).is_empty());
    assert_eq!(
        PROFILE_DESCRIPTORS
            .iter()
            .map(|d| d.profile_id)
            .collect::<Vec<_>>(),
        ["gtm", "proposal"]
    );
    assert_eq!(
        descriptors().iter().map(|d| d.id).collect::<Vec<_>>(),
        ["gtm", "proposal"]
    );
    assert!(
        subject
            .jobs
            .iter()
            .all(|job| !is_packaged_skill(&job.skill_id))
    );
    assert!(
        subject
            .text
            .iter()
            .all(|value| !FORBIDDEN.iter().any(|term| value
                .to_ascii_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|token| token == *term)))
    );
}

#[test]
fn mutations_have_stable_profile_specific_findings() {
    let base = neutral_subject();
    let cases: Vec<(&str, Box<dyn Fn(&mut Subject)>)> = vec![
        (
            "primitive-coverage",
            Box::new(|s| {
                s.primitive_ids.pop();
            }),
        ),
        (
            "primitive-mappings",
            Box::new(|s| {
                s.mappings.get_mut("actors").unwrap().clear();
            }),
        ),
        (
            "input-contracts",
            Box::new(|s| {
                s.input_contracts.clear();
            }),
        ),
        (
            "route-budget",
            Box::new(|s| {
                s.jobs[0].max_entries = 0;
            }),
        ),
        (
            "eval-categories",
            Box::new(|s| {
                s.eval_categories.clear();
            }),
        ),
        (
            "activation",
            Box::new(|s| {
                s.activation_ready = false;
            }),
        ),
        (
            "health",
            Box::new(|s| {
                s.health_valid = false;
            }),
        ),
        (
            "eval",
            Box::new(|s| {
                s.eval_valid = false;
            }),
        ),
        (
            "runtime-authority",
            Box::new(|s| {
                s.receipt_trace_clean_replay = false;
            }),
        ),
    ];
    for (check, mutation) in cases {
        let mut subject = base.clone();
        mutation(&mut subject);
        assert!(
            check_subject(&subject).contains(&finding("neutral", check)),
            "missing {check}"
        );
    }
    let mut gtm = base.clone();
    gtm.id = "gtm".into();
    gtm.text.push("prospect".into());
    assert!(
        !gtm.text
            .iter()
            .all(|value| !FORBIDDEN.contains(&value.as_str()))
    );
    let mut proposal = base;
    proposal.id = "proposal".into();
    proposal.text.push("proposal".into());
    assert!(
        !proposal
            .text
            .iter()
            .all(|value| !FORBIDDEN.contains(&value.as_str()))
    );
}
