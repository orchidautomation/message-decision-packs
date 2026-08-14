use crate::artifact_hash::canonical_json_sha256;
use crate::constants::DEFAULT_DIR;
use crate::models::{Manifest, ProfileJob, PromptFile, PromptInput, PromptOutputContract};
use crate::pack_io::{read_canonical_prompt_by_id, read_prompt, resolve_pack_path};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) const MODEL_STEP_RESOLUTION_V1: &str = "mdp.model-step-resolution.v1";
pub(crate) const COMPILED_MODEL_STEP_V1: &str = "mdp.compiled-model-step.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModelStepPhase {
    Normalization,
    Generation,
    Review,
}

impl ModelStepPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Normalization => "normalization",
            Self::Generation => "generation",
            Self::Review => "review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelStepAuthorityV1 {
    pub(crate) kind: String,
    pub(crate) ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CompiledModelStepV1 {
    pub(crate) contract: String,
    pub(crate) step_id: String,
    pub(crate) job_id: String,
    pub(crate) skill_id: String,
    pub(crate) phase: ModelStepPhase,
    pub(crate) authority: ModelStepAuthorityV1,
    pub(crate) prompt_id: String,
    pub(crate) prompt_version: String,
    pub(crate) prompt_path: String,
    pub(crate) prompt_sha256: String,
    pub(crate) declared_inputs: Vec<PromptInput>,
    pub(crate) routed_context_required: bool,
    pub(crate) output_contract: PromptOutputContract,
    pub(crate) output_contract_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelStepResolutionV1 {
    pub(crate) contract: String,
    pub(crate) job_id: String,
    pub(crate) status: String,
    pub(crate) steps: Vec<CompiledModelStepV1>,
}

pub(crate) fn resolve_model_steps(
    root: &Path,
    manifest: &Manifest,
    job: &ProfileJob,
) -> Result<ModelStepResolutionV1> {
    let selected_inputs = job
        .input_contracts
        .iter()
        .map(|id| {
            manifest
                .input_contracts
                .iter()
                .find(|contract| contract.id == *id)
                .ok_or_else(|| anyhow!("job {} references missing input contract {id}", job.id))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut decision_input_ids = job
        .decision_input_contracts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for input in &selected_inputs {
        decision_input_ids.extend(input.decision_input_contracts.iter().cloned());
    }
    let selected_decision_inputs = decision_input_ids
        .iter()
        .map(|id| {
            manifest
                .decision_input_contracts
                .iter()
                .find(|contract| contract.id == *id)
                .ok_or_else(|| {
                    anyhow!(
                        "job {} references missing decision input contract {id}",
                        job.id
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;

    let legacy_normalizers = selected_inputs
        .iter()
        .filter_map(|contract| {
            contract
                .prompt
                .as_ref()
                .filter(|prompt| !prompt.trim().is_empty())
                .map(|prompt| (contract.id.clone(), prompt.clone()))
        })
        .collect::<Vec<_>>();
    let decision_input_normalizers = selected_decision_inputs
        .iter()
        .map(|contract| {
            (
                contract.id.clone(),
                contract.normalization.prompt.clone(),
                contract.normalization.prompt_version.clone(),
            )
        })
        .collect::<Vec<_>>();

    let mut steps = Vec::new();
    if !legacy_normalizers.is_empty() && !decision_input_normalizers.is_empty() {
        let resolved_legacy = legacy_normalizers
            .iter()
            .map(|(_, prompt_ref)| resolve_prompt_ref(root, prompt_ref))
            .collect::<Result<Vec<_>>>()?;
        let resolved_decision = decision_input_normalizers
            .iter()
            .map(|(_, prompt_ref, version)| {
                let (path, prompt) = resolve_prompt_ref(root, prompt_ref)?;
                Ok((path, prompt, version.as_str()))
            })
            .collect::<Result<Vec<_>>>()?;
        let legacy_authorities = resolved_legacy
            .iter()
            .map(|(path, prompt)| (path, prompt.id.as_str(), prompt.version.as_deref()))
            .collect::<BTreeSet<_>>();
        let decision_authorities = resolved_decision
            .iter()
            .map(|(path, prompt, version)| (path, prompt.id.as_str(), *version))
            .collect::<BTreeSet<_>>();
        if legacy_authorities.len() != 1 || decision_authorities.len() != 1 {
            return Err(anyhow!(
                "job {} has ambiguous normalization authority",
                job.id
            ));
        }
        let (legacy_path, legacy_prompt) = &resolved_legacy[0];
        let (decision_path, decision_prompt, decision_version) = &resolved_decision[0];
        if legacy_prompt.id != decision_prompt.id
            || legacy_prompt.version.as_deref() != Some(*decision_version)
            || legacy_path != decision_path
        {
            return Err(anyhow!(
                "job {} declares conflicting legacy and Decision Input normalization authority",
                job.id
            ));
        }
        steps.push(compile_step(
            root,
            job,
            ModelStepPhase::Normalization,
            ModelStepAuthorityV1 {
                kind: "decision_input_contract".to_string(),
                ids: decision_input_normalizers
                    .iter()
                    .map(|(id, _, _)| id.clone())
                    .collect(),
            },
            decision_path.clone(),
            decision_prompt.clone(),
            Some(decision_version),
        )?);
    } else if !legacy_normalizers.is_empty() {
        let resolved = legacy_normalizers
            .iter()
            .map(|(_, prompt_ref)| resolve_prompt_ref(root, prompt_ref))
            .collect::<Result<Vec<_>>>()?;
        let authorities = resolved
            .iter()
            .map(|(path, prompt)| (path, prompt.id.as_str(), prompt.version.as_deref()))
            .collect::<BTreeSet<_>>();
        if authorities.len() != 1 {
            return Err(anyhow!(
                "job {} has ambiguous legacy normalization prompts",
                job.id
            ));
        }
        let (path, prompt) = resolved.into_iter().next().expect("non-empty normalizers");
        steps.push(compile_step(
            root,
            job,
            ModelStepPhase::Normalization,
            ModelStepAuthorityV1 {
                kind: "input_contract".to_string(),
                ids: legacy_normalizers
                    .iter()
                    .map(|(id, _)| id.clone())
                    .collect(),
            },
            path,
            prompt,
            None,
        )?);
    } else if !decision_input_normalizers.is_empty() {
        let resolved = decision_input_normalizers
            .iter()
            .map(|(_, prompt_ref, version)| {
                if prompt_ref.trim().is_empty() || version.trim().is_empty() {
                    return Err(anyhow!(
                        "job {} has an incomplete Decision Input normalization binding",
                        job.id
                    ));
                }
                let (path, prompt) = resolve_prompt_ref(root, prompt_ref)?;
                Ok((path, prompt, version.as_str()))
            })
            .collect::<Result<Vec<_>>>()?;
        let authorities = resolved
            .iter()
            .map(|(path, prompt, version)| (path, prompt.id.as_str(), *version))
            .collect::<BTreeSet<_>>();
        if authorities.len() != 1 {
            return Err(anyhow!(
                "job {} has ambiguous Decision Input normalization prompts",
                job.id
            ));
        }
        let (path, prompt, version) = resolved.into_iter().next().expect("non-empty normalizers");
        steps.push(compile_step(
            root,
            job,
            ModelStepPhase::Normalization,
            ModelStepAuthorityV1 {
                kind: "decision_input_contract".to_string(),
                ids: decision_input_normalizers
                    .iter()
                    .map(|(id, _, _)| id.clone())
                    .collect(),
            },
            path,
            prompt,
            Some(version),
        )?);
    }

    if let Some(binding) = &job.model_task {
        let phase = match binding.kind.as_str() {
            "generation" => ModelStepPhase::Generation,
            "review" => ModelStepPhase::Review,
            other => {
                return Err(anyhow!(
                    "job {} declares unsupported model_task kind {other}",
                    job.id
                ));
            }
        };
        if binding.prompt.trim().is_empty()
            || binding.prompt.contains('/')
            || binding.prompt.ends_with(".yaml")
            || binding.prompt.ends_with(".yml")
        {
            return Err(anyhow!(
                "job {} model_task must bind one canonical prompt id",
                job.id
            ));
        }
        let (path, prompt) =
            read_canonical_prompt_by_id(root, &binding.prompt)?.ok_or_else(|| {
                anyhow!(
                    "job {} references missing prompt {}",
                    job.id,
                    binding.prompt
                )
            })?;
        steps.push(compile_step(
            root,
            job,
            phase,
            ModelStepAuthorityV1 {
                kind: "model_task".to_string(),
                ids: vec![binding.prompt.clone()],
            },
            path,
            prompt,
            None,
        )?);
    }

    Ok(ModelStepResolutionV1 {
        contract: MODEL_STEP_RESOLUTION_V1.to_string(),
        job_id: job.id.clone(),
        status: if steps.is_empty() {
            "unassessed".to_string()
        } else {
            "ready".to_string()
        },
        steps,
    })
}

pub(crate) fn resolve_selected_model_step(
    root: &Path,
    manifest: &Manifest,
    job_id: &str,
    operation: &str,
) -> Result<CompiledModelStepV1> {
    let job = manifest
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| anyhow!("unknown profile job {job_id}"))?;
    let resolution = resolve_model_steps(root, manifest, job)?;
    if resolution.steps.is_empty() {
        return Err(anyhow!(
            "job {job_id} has no declared native model step; status is unassessed"
        ));
    }
    resolution
        .steps
        .into_iter()
        .find(|step| step.step_id == operation)
        .ok_or_else(|| {
            anyhow!("operation {operation} does not select a declared model step for job {job_id}")
        })
}

fn resolve_prompt_ref(root: &Path, prompt_ref: &str) -> Result<(PathBuf, PromptFile)> {
    if prompt_ref.trim().is_empty() {
        return Err(anyhow!("normalization prompt reference must not be blank"));
    }
    if prompt_ref.contains('/') || prompt_ref.ends_with(".yaml") || prompt_ref.ends_with(".yml") {
        let path = resolve_pack_path(root, prompt_ref)?;
        let prompt = read_prompt(&path)
            .with_context(|| format!("reading normalization prompt {prompt_ref}"))?;
        Ok((path, prompt))
    } else {
        read_canonical_prompt_by_id(root, prompt_ref)?
            .ok_or_else(|| anyhow!("normalization prompt {prompt_ref} is missing"))
    }
}

fn compile_step(
    root: &Path,
    job: &ProfileJob,
    phase: ModelStepPhase,
    authority: ModelStepAuthorityV1,
    path: PathBuf,
    prompt: PromptFile,
    expected_version: Option<&str>,
) -> Result<CompiledModelStepV1> {
    let legacy_normalization = prompt.format == "mdp.prompt.v0"
        && phase == ModelStepPhase::Normalization
        && prompt.kind.is_none();
    if prompt.format != "mdp.prompt.v1" && !legacy_normalization {
        return Err(anyhow!(
            "job {} prompt {} uses unsupported runtime format {}",
            job.id,
            prompt.id,
            prompt.format
        ));
    }
    if !legacy_normalization && prompt.kind.as_deref() != Some(phase.as_str()) {
        return Err(anyhow!(
            "job {} {} step references prompt {} with kind {:?}",
            job.id,
            phase.as_str(),
            prompt.id,
            prompt.kind
        ));
    }
    let prompt_version = prompt
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("prompt {} has no version", prompt.id))?
        .to_string();
    if let Some(expected) = expected_version
        && expected != prompt_version
    {
        return Err(anyhow!(
            "job {} expects prompt {} version {expected}, found {prompt_version}",
            job.id,
            prompt.id
        ));
    }
    let prompt_json = serde_json::to_value(&prompt)?;
    let prompt_sha256 = canonical_json_sha256(&prompt_json)?;
    let output_contract_json = serde_json::to_value(&prompt.output_contract)?;
    let output_contract_sha256 = canonical_json_sha256(&output_contract_json)?;
    let prompt_path = path
        .strip_prefix(root.join(DEFAULT_DIR))
        .or_else(|_| path.strip_prefix(root))
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let routed_context_required = prompt
        .inputs
        .iter()
        .any(|input| input.required && input.name == "routed_context");
    Ok(CompiledModelStepV1 {
        contract: COMPILED_MODEL_STEP_V1.to_string(),
        step_id: format!("model:{}/{}", job.id, phase.as_str()),
        job_id: job.id.clone(),
        skill_id: job.skill_id.clone(),
        phase,
        authority,
        prompt_id: prompt.id,
        prompt_version,
        prompt_path,
        prompt_sha256,
        declared_inputs: prompt.inputs,
        routed_context_required,
        output_contract: prompt.output_contract,
        output_contract_sha256,
    })
}

pub(crate) fn compiled_model_step_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Compiled Model Step v1",
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "step_id", "job_id", "skill_id", "phase", "authority", "prompt_id", "prompt_version", "prompt_path", "prompt_sha256", "declared_inputs", "routed_context_required", "output_contract", "output_contract_sha256"],
        "properties": {
            "contract": {"const": COMPILED_MODEL_STEP_V1},
            "step_id": {"type": "string", "pattern": "^model:[a-z0-9][a-z0-9-]*/(normalization|generation|review)$"},
            "job_id": {"type": "string", "minLength": 1},
            "skill_id": {"type": "string", "minLength": 1},
            "phase": {"enum": ["normalization", "generation", "review"]},
            "authority": {
                "type": "object",
                "additionalProperties": false,
                "required": ["kind", "ids"],
                "properties": {
                    "kind": {"enum": ["input_contract", "decision_input_contract", "model_task"]},
                    "ids": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}}
                }
            },
            "prompt_id": {"type": "string", "minLength": 1},
            "prompt_version": {"type": "string", "minLength": 1},
            "prompt_path": {"type": "string", "pattern": "^prompts/[^/].*\\.ya?ml$"},
            "prompt_sha256": {"type": "string", "pattern": "^[a-f0-9]{64}$"},
            "declared_inputs": {"type": "array"},
            "routed_context_required": {"type": "boolean"},
            "output_contract": {"type": "object"},
            "output_contract_sha256": {"type": "string", "pattern": "^[a-f0-9]{64}$"}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ModelStepPhase, compiled_model_step_schema, resolve_model_steps,
        resolve_selected_model_step,
    };
    use crate::models::{InputContract, ProfileJob};
    use crate::pack_io::read_manifest;
    use serde_json::to_value;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn template(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("plugin/assets/templates")
            .join(name)
    }

    #[test]
    fn resolves_all_shipped_job_step_bindings_in_phase_order() {
        let mut bindings = Vec::new();
        for template_name in ["basic", "proposal"] {
            let root = template(template_name);
            let manifest = read_manifest(&root).unwrap();
            for job in &manifest.jobs {
                let resolution = resolve_model_steps(&root, &manifest, job).unwrap();
                assert_eq!(resolution.steps[0].phase, ModelStepPhase::Normalization);
                bindings.extend(resolution.steps);
            }
        }
        assert_eq!(bindings.len(), 13);
        assert_eq!(
            bindings
                .iter()
                .map(|step| step.prompt_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            8
        );
        for step in bindings {
            jsonschema::draft202012::validate(
                &compiled_model_step_schema(),
                &to_value(step).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn no_bound_prompt_is_explicitly_unassessed() {
        let root = template("basic");
        let manifest = read_manifest(&root).unwrap();
        let job = ProfileJob {
            id: "no-model-step".into(),
            skill_id: "mdp".into(),
            ..ProfileJob::default()
        };
        let resolution = resolve_model_steps(&root, &manifest, &job).unwrap();
        assert_eq!(resolution.status, "unassessed");
        assert!(resolution.steps.is_empty());
    }

    #[test]
    fn does_not_infer_an_unbound_prompt() {
        let root = template("basic");
        let manifest = read_manifest(&root).unwrap();
        let job = ProfileJob {
            id: "looks-like-normalization".into(),
            skill_id: "mdp-gtm-brief".into(),
            ..ProfileJob::default()
        };
        assert!(
            resolve_model_steps(&root, &manifest, &job)
                .unwrap()
                .steps
                .is_empty()
        );
    }

    #[test]
    fn rejects_ambiguous_legacy_normalizers() {
        let root = template("basic");
        let mut manifest = read_manifest(&root).unwrap();
        manifest.input_contracts.push(InputContract {
            id: "other".into(),
            prompt: Some("prompts/generate-outbound-copy.yaml".into()),
            ..InputContract::default()
        });
        let mut job = manifest.jobs[0].clone();
        job.input_contracts.push("other".into());
        let error = resolve_model_steps(&root, &manifest, &job).unwrap_err();
        assert!(error.to_string().contains("ambiguous legacy normalization"));
    }

    #[test]
    fn coalesces_equivalent_legacy_prompt_path_and_id_aliases() {
        let root = template("basic");
        let mut manifest = read_manifest(&root).unwrap();
        manifest.input_contracts.push(InputContract {
            id: "prompt-id-alias".into(),
            prompt: Some("normalize-prospect-row".into()),
            ..InputContract::default()
        });
        let mut job = manifest.jobs[0].clone();
        job.input_contracts.push("prompt-id-alias".into());

        let resolution = resolve_model_steps(&root, &manifest, &job).unwrap();
        assert_eq!(resolution.steps[0].prompt_id, "normalize-prospect-row");
        assert_eq!(
            resolution.steps[0].authority.ids,
            vec!["prospect", "prompt-id-alias"]
        );
    }

    #[test]
    fn rejects_conflicting_legacy_and_decision_input_normalization_authority() {
        let root = template("basic");
        let mut manifest = read_manifest(&root).unwrap();
        let mut job = manifest.jobs[0].clone();
        let mut decision_input = crate::models::DecisionInputContract::default();
        decision_input.id = "future".into();
        decision_input.normalization.prompt = "prompts/generate-outbound-copy.yaml".into();
        decision_input.normalization.prompt_version = "1".into();
        manifest.decision_input_contracts.push(decision_input);
        job.decision_input_contracts.push("future".into());
        let error = resolve_model_steps(&root, &manifest, &job).unwrap_err();
        assert!(error.to_string().contains("conflicting legacy"));
    }

    #[test]
    fn coalesces_matching_legacy_and_decision_input_normalization_authority() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("examples/clay-audiences-self-serve-enterprise-expansion");
        let manifest = read_manifest(&root).unwrap();
        let job = manifest
            .jobs
            .iter()
            .find(|job| job.id == "prospect-fit-or-brief")
            .unwrap();
        let resolution = resolve_model_steps(&root, &manifest, job).unwrap();
        assert_eq!(resolution.steps[0].phase, ModelStepPhase::Normalization);
        assert_eq!(resolution.steps[0].prompt_id, "normalize-prospect-row");
        assert_eq!(
            resolution.steps[0].authority.kind,
            "decision_input_contract"
        );
    }

    #[test]
    fn resolves_future_decision_input_normalization_when_legacy_authority_is_absent() {
        let root = template("basic");
        let mut manifest = read_manifest(&root).unwrap();
        manifest.input_contracts[0].prompt = None;
        let mut job = manifest.jobs[0].clone();
        let mut decision_input = crate::models::DecisionInputContract::default();
        decision_input.id = "future".into();
        decision_input.normalization.prompt = "prompts/normalize-prospect.yaml".into();
        decision_input.normalization.prompt_version = "1".into();
        manifest.decision_input_contracts.push(decision_input);
        job.decision_input_contracts.push("future".into());

        let resolution = resolve_model_steps(&root, &manifest, &job).unwrap();
        assert_eq!(resolution.steps.len(), 1);
        assert_eq!(resolution.steps[0].phase, ModelStepPhase::Normalization);
        assert_eq!(
            resolution.steps[0].authority.kind,
            "decision_input_contract"
        );
    }

    #[test]
    fn rejects_missing_and_wrong_phase_model_task_prompts() {
        let root = template("basic");
        let manifest = read_manifest(&root).unwrap();
        let mut missing = manifest.jobs[1].clone();
        missing.model_task.as_mut().unwrap().prompt = "not-a-prompt".into();
        assert!(
            resolve_model_steps(&root, &manifest, &missing)
                .unwrap_err()
                .to_string()
                .contains("missing prompt")
        );

        let mut wrong_phase = manifest.jobs[1].clone();
        wrong_phase.model_task.as_mut().unwrap().prompt = "review-outbound-copy-v1".into();
        assert!(
            resolve_model_steps(&root, &manifest, &wrong_phase)
                .unwrap_err()
                .to_string()
                .contains("with kind")
        );
    }

    #[test]
    fn selected_step_requires_the_exact_job_bound_operation() {
        let root = template("basic");
        let manifest = read_manifest(&root).unwrap();
        let step = resolve_selected_model_step(
            &root,
            &manifest,
            "outbound-copy-brief",
            "model:outbound-copy-brief/generation",
        )
        .unwrap();
        assert_eq!(step.phase, ModelStepPhase::Generation);
        assert!(
            resolve_selected_model_step(
                &root,
                &manifest,
                "outbound-copy-brief",
                "model:outbound-copy-review/review",
            )
            .unwrap_err()
            .to_string()
            .contains("does not select")
        );
    }
}
