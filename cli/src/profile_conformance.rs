//! Test-only contract shared by the shipped profile registries.

use crate::artifact_hash::pack_content_sha256;
use crate::commands::capabilities::capabilities;
use crate::commands::decision_trace::DECISION_TRACE_V1;
use crate::commands::evals::eval_pack;
use crate::commands::health::{profile_activation_decision, validate_pack};
use crate::constants::RUN_RECEIPT_CONTRACT;
use crate::models::Manifest;
use crate::pack_io::read_manifest;
use crate::primitives::PrimitiveId;
use crate::routing::route_budget_preflight;
use crate::run_contracts::RUN_BUNDLE_V1;
use crate::run_replay::REPLAY_LEDGER_CONTRACT;
use crate::skill_catalog::{PROFILE_DESCRIPTORS, is_packaged_skill};
use crate::template_registry::{descriptors, lookup};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;

const BASIC_DIGEST: &str = "4abebfbf78dfce74312f7a727e6b31466ffd2eefffe56237e5c52f9b1b9b922f";
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
    registry_valid: bool,
    safe_boundaries: bool,
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
    contracts: Vec<String>,
    authored_digest: Option<String>,
    expected_digest: Option<String>,
    routes: Vec<Route>,
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
    #[serde(default)]
    model_task: Option<ModelTask>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ModelTask {
    kind: String,
    prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Route {
    job_id: String,
    skill_id: String,
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
    if !subject.registry_valid {
        out.push(finding(&subject.id, "registry"));
    }
    if !subject.safe_boundaries {
        out.push(finding(&subject.id, "safe-boundaries"));
    }
    if subject.id == "neutral" && subject.registered {
        out.push(finding(&subject.id, "registration"));
    }
    if let (Some(actual), Some(expected_digest)) =
        (&subject.authored_digest, &subject.expected_digest)
    {
        if actual != expected_digest {
            out.push(finding(&subject.id, "pack-digest"));
        }
    }
    let routes = subject
        .routes
        .iter()
        .map(|route| (&route.job_id, &route.skill_id))
        .collect::<BTreeSet<_>>();
    let mut jobs = BTreeSet::new();
    for job in &subject.jobs {
        if !jobs.insert(&job.id) {
            out.push(finding(&subject.id, "job-ownership"));
        }
        if job.skill_id.is_empty() || !job.required_primitives.iter().all(|p| expected.contains(p))
        {
            out.push(finding(&subject.id, "job-ownership"));
        }
        if !routes.contains(&(&job.id, &job.skill_id)) {
            out.push(finding(&subject.id, "route-ownership"));
        }
        if let Some(task) = &job.model_task
            && (task.kind.trim().is_empty() || task.prompt.trim().is_empty())
        {
            out.push(finding(&subject.id, "model-task"));
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
    let required_contracts = [
        RUN_RECEIPT_CONTRACT,
        DECISION_TRACE_V1,
        RUN_BUNDLE_V1,
        REPLAY_LEDGER_CONTRACT,
    ];
    if required_contracts
        .iter()
        .any(|contract| !subject.contracts.iter().any(|value| value == contract))
    {
        out.push(finding(&subject.id, "runtime-authority"));
    }
    if subject.id == "neutral"
        && fixture_strings(subject)
            .iter()
            .any(|value| forbidden(value))
    {
        out.push(finding(&subject.id, "vocabulary-isolation"));
    }
    out
}

fn forbidden(value: &str) -> bool {
    FORBIDDEN.iter().any(|term| {
        value
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|token| token == *term)
    })
}

fn fixture_strings(subject: &Subject) -> Vec<String> {
    fn visit(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::String(value) if PrimitiveId::from_str(value).is_err() => {
                out.push(value.clone())
            }
            serde_json::Value::Array(values) => values.iter().for_each(|value| visit(value, out)),
            serde_json::Value::Object(values) => values.iter().for_each(|(key, value)| {
                if PrimitiveId::from_str(key).is_err() {
                    out.push(key.clone());
                }
                visit(value, out);
            }),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(subject).expect("subject serializes");
    if let Some(object) = value.as_object_mut() {
        object.remove("primitive_ids");
    }
    let mut strings = Vec::new();
    visit(&value, &mut strings);
    strings
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
            model_task: job.model_task.as_ref().map(|task| ModelTask {
                kind: task.kind.clone(),
                prompt: task.prompt.clone(),
            }),
        })
        .collect();
    Subject {
        id: profile_id.to_string(),
        registered: true,
        registry_valid: crate::skill_catalog::validate_registry(PROFILE_DESCRIPTORS).is_ok(),
        safe_boundaries: true,
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
        contracts: vec![
            RUN_RECEIPT_CONTRACT.into(),
            DECISION_TRACE_V1.into(),
            RUN_BUNDLE_V1.into(),
            REPLAY_LEDGER_CONTRACT.into(),
        ],
        authored_digest: Some(pack_content_sha256(&pack).expect("canonical digest")),
        expected_digest: Some(if profile_id == "gtm" {
            BASIC_DIGEST.into()
        } else {
            PROPOSAL_DIGEST.into()
        }),
        routes: descriptor
            .jobs
            .iter()
            .map(|route| Route {
                job_id: route.job_id.into(),
                skill_id: route.skill_id.into(),
            })
            .collect(),
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
    assert_eq!(
        capabilities()["defaults"]["init_templates"],
        serde_json::json!(["gtm", "proposal"])
    );
    assert!(
        subject
            .jobs
            .iter()
            .all(|job| !is_packaged_skill(&job.skill_id))
    );
    assert!(subject.text.iter().all(|value| !forbidden(value)));
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
                s.contracts.clear();
            }),
        ),
        ("safe-boundaries", Box::new(|s| s.safe_boundaries = false)),
        ("registry", Box::new(|s| s.registry_valid = false)),
        ("registration", Box::new(|s| s.registered = true)),
        (
            "route-ownership",
            Box::new(|s| s.routes[0].skill_id = "other".into()),
        ),
        (
            "model-task",
            Box::new(|s| s.jobs[0].model_task.as_mut().unwrap().prompt.clear()),
        ),
        ("output-mapping", Box::new(|s| s.output_mapping = false)),
        ("gap-mapping", Box::new(|s| s.gap_mapping = false)),
        ("eval-mapping", Box::new(|s| s.eval_mapping = false)),
        (
            "pack-digest",
            Box::new(|s| {
                s.authored_digest = Some("changed".into());
                s.expected_digest = Some("drift".into());
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
    gtm.text.push("prospect".into());
    assert!(check_subject(&gtm).contains(&finding("neutral", "vocabulary-isolation")));
    let mut proposal = base;
    proposal.text.push("proposal".into());
    assert!(check_subject(&proposal).contains(&finding("neutral", "vocabulary-isolation")));
    for profile in ["gtm", "proposal"] {
        let mut subject = real_subject(profile);
        subject.route_ready = false;
        assert!(check_subject(&subject).contains(&finding(profile, "route-budget")));
    }
}
