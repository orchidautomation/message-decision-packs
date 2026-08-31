use crate::commands::health::validate_pack;
use crate::commands::init_transaction::{
    self, GeneratedArtifact, dry_run as tx_dry_run, fresh_nonce, preflight, stage_artifacts,
};
use crate::constants::{DEFAULT_DIR, FORMAT_VERSION};
use crate::models::{Card, Manifest, TargetIdentity};
use crate::pack_io::{planned_directory, read_manifest};
use crate::pack_readme::render_pack_readme;
use crate::starter::{
    decision_input_scenarios, generated_starter_evals, generated_starter_manifest,
    generated_starter_prompts, starter_cards, starter_evals, starter_manifest, starter_prompts,
    starter_prospect, starter_source_ledger,
};
use crate::target_starter::{
    target_cards, target_evals, target_manifest, target_prompts, target_prospect,
    target_source_ledger,
};
use crate::template_registry;
use crate::utils::slugify;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use serde_yaml::Value as YamlValue;
use std::path::{Path, PathBuf};

#[cfg(test)]
const PROPOSAL_TEMPLATE_NAME: &str = "Proposal Reference Profile Sample";

pub(crate) struct TargetInitOptions<'a> {
    pub(crate) custom_name: bool,
    pub(crate) name: Option<&'a str>,
    pub(crate) kind: &'a str,
    pub(crate) aliases: &'a [String],
    pub(crate) excluded_terms: &'a [String],
}

struct InitRequest<'a> {
    descriptor: &'static crate::template_registry::TemplateDescriptor,
    root: &'a Path,
    name: &'a str,
    target: Option<&'a TargetIdentity>,
    force: bool,
    include_output_schemas: bool,
    governed: bool,
    dry_run: bool,
}
impl Default for TargetInitOptions<'_> {
    fn default() -> Self {
        Self {
            custom_name: false,
            name: None,
            kind: "company",
            aliases: &[],
            excluded_terms: &[],
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn init_pack(
    root: &Path,
    name: &str,
    template: &str,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    let descriptor =
        template_registry::lookup(template).ok_or_else(|| unsupported_template(template))?;
    run_init(InitRequest {
        descriptor,
        root,
        name,
        target: None,
        force,
        include_output_schemas,
        governed: false,
        dry_run: false,
    })
}

pub(crate) fn init_pack_targeted(
    root: &Path,
    name: &str,
    template: &str,
    target_options: &TargetInitOptions<'_>,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    let target = resolve_target_identity(
        target_options.custom_name,
        template,
        target_options.name,
        target_options.kind,
        target_options.aliases,
        target_options.excluded_terms,
    )?;
    let descriptor =
        template_registry::lookup(template).ok_or_else(|| unsupported_template(template))?;
    run_init(InitRequest {
        descriptor,
        root,
        name,
        target: target.as_ref(),
        force,
        include_output_schemas,
        governed: true,
        dry_run: false,
    })
}

pub(crate) fn init_pack_dry_run(
    root: &Path,
    name: &str,
    template: &str,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    init_pack_targeted_dry_run(
        root,
        name,
        template,
        &TargetInitOptions::default(),
        force,
        include_output_schemas,
    )
}

pub(crate) fn init_pack_targeted_dry_run(
    root: &Path,
    name: &str,
    template: &str,
    target_options: &TargetInitOptions<'_>,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    let target = resolve_target_identity(
        target_options.custom_name,
        template,
        target_options.name,
        target_options.kind,
        target_options.aliases,
        target_options.excluded_terms,
    )?;
    let descriptor =
        template_registry::lookup(template).ok_or_else(|| unsupported_template(template))?;
    run_init(InitRequest {
        descriptor,
        root,
        name,
        target: target.as_ref(),
        force,
        include_output_schemas,
        governed: true,
        dry_run: true,
    })
}

fn run_init(request: InitRequest<'_>) -> Result<Value> {
    validate_target_destination(request.root, request.target)?;
    let mut inventory = match request.descriptor.postprocess {
        crate::template_registry::TemplatePostprocess::Gtm => build_gtm_inventory(
            request.root,
            request.name,
            request.descriptor.id,
            request.descriptor,
            request.target,
            request.force,
            request.include_output_schemas,
            request.governed,
        )?,
        crate::template_registry::TemplatePostprocess::Proposal => {
            proposal_inventory(request.descriptor, request.name)?
        }
    };
    append_required_directories(request.descriptor, &mut inventory);
    validate_generated_inventory(request.descriptor, &inventory)?;
    if request.dry_run {
        let plan = tx_dry_run(request.root, &inventory, request.force)?;
        let mut payload = match request.descriptor.postprocess {
            crate::template_registry::TemplatePostprocess::Gtm => gtm_init_payload(
                request.root,
                request.name,
                request.descriptor.id,
                request.target,
            ),
            crate::template_registry::TemplatePostprocess::Proposal => {
                proposal_init_payload(request.root, request.name)
            }
        };
        if let Some(object) = payload.as_object_mut() {
            object.insert("dry_run".into(), json!(true));
            object.insert("template".into(), json!(request.descriptor.id));
            object.insert("slug".into(), json!(slugify(request.name)));
            object.insert("force".into(), json!(request.force));
            object.insert(
                "write_plan".into(),
                Value::Array(dry_run_plan_to_legacy(&plan, &inventory, request.root)),
            );
            object.insert(
                "publication".into(),
                init_transaction::dry_run_envelope(&plan),
            );
        }
        return Ok(payload);
    }
    let outcome = run_publish(request.root, &inventory, request.force, |staging_root| {
        let diagnostics = validate_pack(staging_root)?;
        if !diagnostics
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(anyhow!(
                "staged init pack at {} failed validation: {}",
                staging_root.display(),
                diagnostics
                    .get("issues")
                    .cloned()
                    .unwrap_or(Value::Array(Vec::new()))
            ));
        }
        Ok(())
    })?;
    let mut payload = match request.descriptor.postprocess {
        crate::template_registry::TemplatePostprocess::Gtm => gtm_init_payload(
            request.root,
            request.name,
            request.descriptor.id,
            request.target,
        ),
        crate::template_registry::TemplatePostprocess::Proposal => {
            proposal_init_payload(request.root, request.name)
        }
    };
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "publication".into(),
            init_transaction::publication_envelope(&outcome),
        );
    }
    Ok(payload)
}

fn resolve_target_identity(
    custom_name: bool,
    template: &str,
    target_name: Option<&str>,
    target_kind: &str,
    target_aliases: &[String],
    exclude_terms: &[String],
) -> Result<Option<TargetIdentity>> {
    let has_target_details =
        !target_aliases.is_empty() || !exclude_terms.is_empty() || target_kind != "company";
    let Some(target_name) = target_name.map(str::trim).filter(|value| !value.is_empty()) else {
        if (template == "gtm" && custom_name) || has_target_details {
            return Err(anyhow!(
                "target identity is ambiguous; pass --target-name with --target-kind company|product|project, or omit custom target arguments for the generic reference template"
            ));
        }
        return Ok(None);
    };
    if template != "gtm" {
        return Err(anyhow!(
            "explicit target-aware initialization currently requires --template gtm; proposal packs use the proposal-specific builder workflow"
        ));
    }
    if !matches!(target_kind, "company" | "product" | "project") {
        return Err(anyhow!(
            "unsupported target kind '{target_kind}'; available: company, product, project"
        ));
    }
    let mut excluded = vec![
        "Basic MDP Template".to_string(),
        "agent-assisted GTM".to_string(),
        "local-cli".to_string(),
        "agent-plugin".to_string(),
        "example-mdp-demo".to_string(),
    ];
    extend_unique(&mut excluded, exclude_terms);
    let mut aliases = Vec::new();
    extend_unique(&mut aliases, target_aliases);
    let external_terms = vec![target_name.to_string()];
    if let Some(conflict) = excluded.iter().find(|excluded| {
        excluded.eq_ignore_ascii_case(target_name)
            || aliases
                .iter()
                .chain(external_terms.iter())
                .any(|allowed| allowed.eq_ignore_ascii_case(excluded))
    }) {
        return Err(anyhow!(
            "target lexicon conflict: excluded term '{conflict}' is also the active target name, alias, or external term"
        ));
    }
    Ok(Some(TargetIdentity {
        kind: target_kind.to_string(),
        name: target_name.to_string(),
        aliases,
        external_terms,
        excluded_terms: excluded,
        internal_terms: vec![
            "MDP".to_string(),
            "Message Decision Pack".to_string(),
            "mdp CLI".to_string(),
            "manifest plus modular cards".to_string(),
            "local offline decision layer".to_string(),
            "agent handoffs".to_string(),
        ],
        source_ids: vec!["target-identity".to_string()],
    }))
}

fn validate_target_destination(root: &Path, target: Option<&TargetIdentity>) -> Result<()> {
    let Some(target) = target else {
        return Ok(());
    };
    let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
    if !manifest_path.exists() {
        return Ok(());
    }
    let existing = read_manifest(root).with_context(|| {
        format!(
            "reading existing target identity from {} before target-aware initialization",
            manifest_path.display()
        )
    })?;
    let same_target = existing.target.as_ref().is_some_and(|existing| {
        existing.kind == target.kind && existing.name.eq_ignore_ascii_case(&target.name)
    });
    if !same_target {
        return Err(anyhow!(
            "refusing to retarget an existing pack with init --force; use a clean directory or explicitly migrate the existing pack, add prior nouns to target.excluded_terms, and validate every surface"
        ));
    }
    Ok(())
}

fn extend_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        let value = value.trim();
        if !value.is_empty()
            && !target
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(value))
        {
            target.push(value.to_string());
        }
    }
}

fn proposal_readme_from_inventory(inventory: &[GeneratedArtifact], _name: &str) -> Result<String> {
    let manifest_raw = inventory
        .iter()
        .find(|a| a.relative == ".mdp/manifest.yaml")
        .map(|a| a.bytes.as_slice())
        .ok_or_else(|| anyhow!("embedded proposal manifest missing"))?;
    let manifest: Manifest =
        serde_yaml::from_slice(manifest_raw).context("parsing proposal manifest")?;
    let mut cards = Vec::new();
    let mut prompt_ids = Vec::new();
    let mut source_ledger = Value::Null;
    for artifact in inventory {
        if artifact.relative.starts_with(".mdp/cards/") && !artifact.is_directory {
            cards.push(
                serde_yaml::from_slice::<Card>(&artifact.bytes)
                    .with_context(|| format!("parsing embedded {}", artifact.relative))?,
            );
        } else if artifact.relative == ".mdp/sources.yaml" {
            source_ledger = serde_yaml::from_slice(&artifact.bytes)
                .context("parsing embedded proposal source ledger")?;
        } else if artifact.relative.starts_with(".mdp/prompts/") && !artifact.is_directory {
            let prompt: Value = serde_yaml::from_slice(&artifact.bytes)
                .with_context(|| format!("parsing embedded {}", artifact.relative))?;
            if let Some(id) = prompt["id"].as_str() {
                prompt_ids.push(id.to_string());
            }
        }
    }
    Ok(render_pack_readme(
        &manifest,
        &cards.iter().collect::<Vec<_>>(),
        &source_ledger,
        &prompt_ids,
    ))
}

fn proposal_inventory(
    descriptor: &'static crate::template_registry::TemplateDescriptor,
    name: &str,
) -> Result<Vec<GeneratedArtifact>> {
    let mut inventory = template_registry::inventory(descriptor);
    if name != descriptor.default_name {
        let manifest = inventory
            .iter_mut()
            .find(|a| a.relative == ".mdp/manifest.yaml")
            .ok_or_else(|| anyhow!("embedded proposal manifest missing"))?;
        let mut value: YamlValue = serde_yaml::from_slice(&manifest.bytes)
            .context("parsing embedded proposal manifest")?;
        let map = value
            .as_mapping_mut()
            .ok_or_else(|| anyhow!("embedded proposal manifest must be a mapping"))?;
        map.insert(
            YamlValue::String("id".into()),
            YamlValue::String(slugify(name)),
        );
        map.insert(
            YamlValue::String("name".into()),
            YamlValue::String(name.into()),
        );
        manifest.bytes = serde_yaml::to_string(&value)
            .context("serializing embedded proposal manifest")?
            .into_bytes();
    }
    let readme = proposal_readme_from_inventory(&inventory, name)?;
    if let Some(existing) = inventory
        .iter_mut()
        .find(|a| a.relative == ".mdp/README.md")
    {
        existing.bytes = readme.into_bytes();
    }
    Ok(inventory)
}

fn unsupported_template(template: &str) -> anyhow::Error {
    anyhow!(
        "unsupported template '{template}'; available: {}",
        template_registry::available()
    )
}

fn append_required_directories(
    descriptor: &crate::template_registry::TemplateDescriptor,
    inventory: &mut Vec<GeneratedArtifact>,
) {
    for directory in descriptor.required_directories {
        if !inventory.iter().any(|a| a.relative == *directory) {
            inventory.push(GeneratedArtifact::directory(*directory));
        }
    }
}

fn validate_generated_inventory(
    descriptor: &crate::template_registry::TemplateDescriptor,
    inventory: &[GeneratedArtifact],
) -> Result<()> {
    let mut paths = std::collections::BTreeSet::new();
    for artifact in inventory {
        let path = artifact.relative.as_str();
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !paths.insert(path)
        {
            return Err(anyhow!(
                "generated inventory contains unsafe or duplicate path '{path}'"
            ));
        }
    }
    for directory in descriptor.required_directories {
        let Some(artifact) = inventory
            .iter()
            .find(|artifact| artifact.relative == *directory)
        else {
            return Err(anyhow!(
                "generated inventory is missing required directory '{directory}'"
            ));
        };
        if !artifact.is_directory {
            return Err(anyhow!(
                "generated inventory requires directory '{directory}'"
            ));
        }
    }
    Ok(())
}

/// Render a complete GTM starter tree as a list of generated artifacts.
/// The function never touches the destination; callers stage and
/// publish the inventory through `init_transaction`.
fn build_gtm_inventory(
    root: &Path,
    name: &str,
    template: &str,
    descriptor: &crate::template_registry::TemplateDescriptor,
    target: Option<&TargetIdentity>,
    force: bool,
    include_output_schemas: bool,
    governed: bool,
) -> Result<Vec<GeneratedArtifact>> {
    let _ = (root, force);
    if target.is_none() && !include_output_schemas && name == "Basic MDP Template" {
        let inventory = template_registry::inventory(descriptor);
        return Ok(inventory);
    }
    let slug = slugify(name);
    let manifest = if let Some(target) = target {
        target_manifest(name, &slug, template, target)
    } else if governed {
        generated_starter_manifest(name, &slug, template)
    } else {
        starter_manifest(name, &slug, template)
    };
    let source_ledger = target
        .map(target_source_ledger)
        .unwrap_or_else(|| starter_source_ledger(template));
    let cards: Vec<(&'static str, Card)> = target
        .map(target_cards)
        .unwrap_or_else(|| starter_cards(template));
    let evals: Vec<(&'static str, Value)> = target.map(target_evals).unwrap_or_else(|| {
        if governed {
            generated_starter_evals()
        } else {
            starter_evals()
        }
    });
    let prompts: Vec<(&'static str, Value)> = target
        .map(|target| target_prompts(target, include_output_schemas))
        .unwrap_or_else(|| {
            if governed {
                generated_starter_prompts(include_output_schemas)
            } else {
                starter_prompts(include_output_schemas)
            }
        });
    let prompt_ids = prompts
        .iter()
        .filter_map(|(_, prompt)| prompt["id"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let card_models = cards.iter().map(|(_, card)| card).collect::<Vec<_>>();
    let readme = render_pack_readme(&manifest, &card_models, &source_ledger, &prompt_ids);
    let prospect = target
        .map(target_prospect)
        .unwrap_or_else(|| starter_prospect(template));
    let prospect_relative = if target.is_some() {
        "examples/prospect-row.json".to_string()
    } else {
        "examples/clay-row.json".to_string()
    };
    let mut prospect_bytes =
        serde_json::to_vec_pretty(&prospect).context("serializing example prospect")?;
    prospect_bytes.push(b'\n');
    let mut scenarios_bytes = serde_json::to_vec_pretty(&decision_input_scenarios())
        .context("serializing decision-input scenarios")?;
    scenarios_bytes.push(b'\n');

    let mut inventory = Vec::new();
    inventory.push(GeneratedArtifact {
        relative: ".mdp/manifest.yaml".to_string(),
        bytes: serde_yaml::to_string(&manifest)
            .context("serializing GTM manifest")?
            .into_bytes(),
        kind: "yaml-file",
        eligible_for_force: true,
        is_directory: false,
    });
    inventory.push(GeneratedArtifact {
        relative: ".mdp/sources.yaml".to_string(),
        bytes: serde_yaml::to_string(&source_ledger)
            .context("serializing GTM source ledger")?
            .into_bytes(),
        kind: "yaml-file",
        eligible_for_force: true,
        is_directory: false,
    });
    for (filename, card) in &cards {
        inventory.push(GeneratedArtifact {
            relative: format!(".mdp/cards/{filename}"),
            bytes: serde_yaml::to_string(card)
                .context("serializing GTM card")?
                .into_bytes(),
            kind: "yaml-file",
            eligible_for_force: true,
            is_directory: false,
        });
    }
    for (filename, eval) in &evals {
        inventory.push(GeneratedArtifact {
            relative: format!(".mdp/evals/{filename}"),
            bytes: serde_yaml::to_string(eval)
                .context("serializing GTM eval")?
                .into_bytes(),
            kind: "yaml-file",
            eligible_for_force: true,
            is_directory: false,
        });
    }
    for (filename, prompt) in &prompts {
        inventory.push(GeneratedArtifact {
            relative: format!(".mdp/prompts/{filename}"),
            bytes: serde_yaml::to_string(prompt)
                .context("serializing GTM prompt")?
                .into_bytes(),
            kind: "yaml-file",
            eligible_for_force: true,
            is_directory: false,
        });
    }
    inventory.push(GeneratedArtifact {
        relative: ".mdp/README.md".to_string(),
        bytes: readme.into_bytes(),
        kind: "markdown-file",
        eligible_for_force: true,
        is_directory: false,
    });
    inventory.push(GeneratedArtifact {
        relative: prospect_relative,
        bytes: prospect_bytes,
        kind: "json-file",
        eligible_for_force: true,
        is_directory: false,
    });
    inventory.push(GeneratedArtifact {
        relative: "examples/decision-input-scenarios.json".to_string(),
        bytes: scenarios_bytes,
        kind: "json-file",
        eligible_for_force: true,
        is_directory: false,
    });
    Ok(inventory)
}

/// Render a transaction-owned staging tree, run the staged validation
/// hook, then publish through the transaction module. The hook receives
/// the staging root and is expected to fail fast when the staged tree
/// is invalid.
fn run_publish<F>(
    root: &Path,
    inventory: &[GeneratedArtifact],
    force: bool,
    validate: F,
) -> Result<init_transaction::PublicationOutcome>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let parent = root.parent().unwrap_or(root);
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating destination parent {}", parent.display()))?;
    }
    let nonce = fresh_nonce();
    let preflight_entries = match preflight(root, inventory, force) {
        Ok(entries) => entries,
        Err(error) => {
            return Err(error);
        }
    };
    if let Some(first_blocked) = preflight_entries
        .iter()
        .find(|entry| entry.action == "blocked")
    {
        return Err(anyhow!(
            "init not published: {} already exists; pass --force to overwrite",
            first_blocked.path
        ));
    }
    let staging_root = stage_artifacts(parent, inventory, &nonce)?;
    let validation = validate(&staging_root);
    if let Err(error) = validation {
        let _ = init_transaction::cleanup(&[&staging_root]);
        return Err(error.context("init not published: staged validation failed"));
    }
    let backup_root = parent.join(format!(".mdp.init.backup.{nonce}"));
    match init_transaction::publish(root, &staging_root, inventory, &backup_root, force) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            if !error.to_string().contains("publication indeterminate") {
                let _ = init_transaction::cleanup(&[&staging_root, &backup_root]);
            }
            Err(error.context("init not published: publication failed"))
        }
    }
}

/// Convert a transaction dry-run plan into the legacy `write_plan`
/// array expected by existing GTM and proposal dry-run output.
fn dry_run_plan_to_legacy(
    plan: &init_transaction::DryRunPlan,
    inventory: &[GeneratedArtifact],
    root: &Path,
) -> Vec<Value> {
    let mut entries = Vec::new();
    // Directories in canonical order: pack_dir, then each generated
    // subdirectory in inventory order, deduped.
    let mut seen_dirs: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for artifact in inventory {
        let abs = artifact.absolute(root);
        if let Some(parent) = abs.parent() {
            for ancestor in parent.ancestors() {
                if !ancestor.starts_with(root) {
                    break;
                }
                if seen_dirs.insert(ancestor.to_path_buf()) {
                    entries.push(planned_directory(ancestor));
                }
            }
        }
    }
    for (artifact, entry) in inventory.iter().zip(plan.entries.iter()) {
        let abs = artifact.absolute(root);
        let kind = artifact.kind;
        let action = &entry.action;
        let would_write = matches!(action.as_str(), "create" | "overwrite");
        entries.push(json!({
            "kind": kind,
            "path": abs.display().to_string(),
            "action": action,
            "exists": entry.existed,
            "parent_exists": true,
            "would_write": would_write
        }));
    }
    entries
}

fn gtm_init_payload(
    root: &Path,
    name: &str,
    template: &str,
    target: Option<&TargetIdentity>,
) -> Value {
    let pack_dir = root.join(DEFAULT_DIR);
    let manifest_path = pack_dir.join("manifest.yaml");
    let source_ledger_path = pack_dir.join("sources.yaml");
    let cards_dir = pack_dir.join("cards");
    let evals_dir = pack_dir.join("evals");
    let prompts_dir = pack_dir.join("prompts");
    let readme_path = pack_dir.join("README.md");
    let prospect_path = root.join("examples").join(if target.is_some() {
        "prospect-row.json"
    } else {
        "clay-row.json"
    });
    let example_persona = if target.is_some() {
        "Operator"
    } else {
        "GTM Engineering"
    };
    let example_job = target
        .map(|target| format!("review evidence gaps for {}", target.name))
        .unwrap_or_else(|| "linkedin outbound copy".to_string());
    let slug = slugify(name);
    json!({
        "format": FORMAT_VERSION,
        "template": template,
        "name": name,
        "slug": slug,
        "root": root.display().to_string(),
        "pack_dir": pack_dir.display().to_string(),
        "manifest": manifest_path.display().to_string(),
        "source_ledger": source_ledger_path.display().to_string(),
        "cards_dir": cards_dir.display().to_string(),
        "evals_dir": evals_dir.display().to_string(),
        "prompts_dir": prompts_dir.display().to_string(),
        "readme": readme_path.display().to_string(),
        "example_prospect": prospect_path.display().to_string(),
        "example_prospect_kind": "synthetic-example",
        "next_commands": [
            format!("mdp --json validate --dir {}", root.display()),
            format!("mdp --json route --entries --dir {} --persona \"{}\" --job \"{}\"", root.display(), example_persona, example_job),
            format!("mdp --json fit --dir {} --prospect {}", root.display(), prospect_path.display()),
            format!("mdp --json --summary brief --dir {} --prospect {} --channel linkedin", root.display(), prospect_path.display()),
            format!("mdp --json eval --dir {}", root.display())
        ]
    })
}

fn proposal_init_payload(root: &Path, name: &str) -> Value {
    let pack_dir = root.join(DEFAULT_DIR);
    let manifest_path = pack_dir.join("manifest.yaml");
    let source_ledger_path = pack_dir.join("sources.yaml");
    let cards_dir = pack_dir.join("cards");
    let evals_dir = pack_dir.join("evals");
    let prompts_dir = pack_dir.join("prompts");
    let readme_path = pack_dir.join("README.md");
    json!({
        "format": FORMAT_VERSION,
        "template": "proposal",
        "name": name,
        "slug": slugify(name),
        "root": root.display().to_string(),
        "pack_dir": pack_dir.display().to_string(),
        "manifest": manifest_path.display().to_string(),
        "source_ledger": source_ledger_path.display().to_string(),
        "cards_dir": cards_dir.display().to_string(),
        "evals_dir": evals_dir.display().to_string(),
        "prompts_dir": prompts_dir.display().to_string(),
        "readme": readme_path.display().to_string(),
        "example_prospect": Value::Null,
        "example_prospect_kind": Value::Null,
        "next_commands": [
            format!("mdp --json validate --dir {}", root.display()),
            format!("mdp --json eval --dir {}", root.display()),
            format!("mdp --json route --entries --dir {} --persona \\\"Proposal Lead\\\" --job \\\"bid no bid review\\\"", root.display()),
            format!("mdp --json gaps --dir {}", root.display()),
            format!("mdp --json check-claims --dir {} --persona \\\"Proposal Lead\\\" --job \\\"compliance review\\\" --text \\\"The sample team is CMMC compliant.\\\"", root.display())
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_hash::pack_content_sha256;
    use crate::product_foundation::{ProductFoundationStatus, resolve_product_foundation_for_pack};
    use crate::routing::narrow_starter_route_candidates_for_tests;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generic_gtm_init_has_ready_foundations_and_orientation_readme() {
        let root = std::env::temp_dir().join(format!("mdp-foundation-gtm-{}", nonce()));
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
            .expect("generic GTM pack should initialize");

        let manifest = read_manifest(&root).expect("manifest should parse");
        for job in &manifest.jobs {
            let foundation = resolve_product_foundation_for_pack(&root, &manifest, &job.id)
                .expect("foundation should resolve");
            assert_eq!(
                foundation.status,
                ProductFoundationStatus::Ready,
                "{}",
                job.id
            );
            assert!(!foundation.selected_facets.is_empty());
            if job.id == "prospect-fit-or-brief" {
                assert!(job.model_task.is_none());
            } else {
                let binding = job.model_task.as_ref().expect("model task should be bound");
                let prompt_path = root
                    .join(".mdp/prompts")
                    .join(if binding.kind == "generation" {
                        "generate-outbound-copy.yaml"
                    } else {
                        "review-outbound-copy.yaml"
                    });
                let prompt = crate::pack_io::read_prompt(&prompt_path)
                    .expect("job-owned prompt should parse");
                assert_eq!(prompt.id, binding.prompt);
                assert_eq!(prompt.kind.as_deref(), Some(binding.kind.as_str()));
                assert_eq!(prompt.version.as_deref(), Some("3"));
                assert_eq!(
                    prompt.output_contract.output_kind.as_deref(),
                    Some("governed-artifact")
                );
            }
        }
        let readme = std::fs::read_to_string(root.join(".mdp/README.md"))
            .expect("orientation README should exist");
        for heading in [
            "## Authority",
            "## Thesis",
            "## Actors",
            "## ICP and Fit Authority",
            "## Supported Jobs",
            "## Decision Flow",
            "## Boundaries",
            "## Sources",
            "## Prompts",
            "## Commands",
            "## Gaps",
        ] {
            assert!(readme.contains(heading), "missing {heading}");
        }
        assert!(readme.contains("orientation only"));
        assert!(readme.contains("prospect-fit-or-brief"));
        assert!(readme.contains("mdp --json skills --job prospect-fit-or-brief"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn synthetic_gtm_jobs_prove_material_context_reduction() {
        let root = std::env::temp_dir().join(format!("mdp-minimal-context-{}", nonce()));
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
            .expect("generic GTM pack should initialize");
        narrow_starter_route_candidates_for_tests(&root);
        let manifest = read_manifest(&root).expect("manifest should parse");
        let total_entries = manifest
            .cards
            .iter()
            .map(|card_ref| {
                crate::pack_io::read_card(&root.join(".mdp").join(&card_ref.path))
                    .expect("card should parse")
                    .entries
                    .len()
            })
            .sum::<usize>();

        for (job_id, persona) in [
            ("prospect-fit-or-brief", "PMM"),
            ("outbound-copy-brief", "PMM"),
            // Exercise review as supplied copy for a persona with declared entry
            // selectors, not through prose overlap such as PM inside PMM.
            ("outbound-copy-review", "PMM"),
        ] {
            let job = manifest
                .jobs
                .iter()
                .find(|job| job.id == job_id)
                .expect("synthetic GTM job should exist");
            assert!(job.context_budget.is_some(), "{job_id} must be budgeted");
            if job_id == "outbound-copy-review" {
                assert_eq!(
                    job.model_task.as_ref().map(|task| task.kind.as_str()),
                    Some("review")
                );
            }
            let context = crate::routing::entry_context_scoped(
                &root,
                &manifest,
                persona,
                job_id,
                true,
                &crate::scope::ScopeResolution::default(),
            )
            .expect("context should compile");
            assert_eq!(
                context["minimality"]["status"], "ready",
                "{job_id}: minimality={} gaps={}",
                context["minimality"], context["gaps"]
            );
            assert_eq!(context["status"], "ready", "{job_id}");
            assert_eq!(context["persona"], persona, "{job_id}");
            assert_eq!(context["job"], job_id, "{job_id}");
            assert_eq!(
                context["runtime_context"]["contract"], "mdp.runtime-context.v0",
                "{job_id} must use the governed runtime route"
            );
            assert_eq!(
                context["model_context"]["contract"],
                crate::constants::ROUTED_CONTEXT_CONTRACT,
                "{job_id} must expose bounded model context"
            );
            assert_eq!(context["model_context"]["persona"], persona, "{job_id}");
            assert_eq!(context["model_context"]["job"], job_id, "{job_id}");
            let selected = context["minimality"]["selected_count"]
                .as_u64()
                .expect("selected count") as usize;
            assert!(selected > 0 && selected < total_entries, "{job_id}");
            assert!(
                context["summary"]["guardrail_entry_count"]
                    .as_u64()
                    .expect("guardrail count")
                    > 0,
                "{job_id} must retain guardrails"
            );
            assert_eq!(
                context["minimality"]["context_sha256"]
                    .as_str()
                    .expect("context digest")
                    .len(),
                64
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn targeted_gtm_init_selects_job_specific_authority_gaps() {
        let root = std::env::temp_dir().join(format!("mdp-foundation-target-{}", nonce()));
        init_pack_targeted(
            &root,
            "Company B Messaging",
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                name: Some("Company B"),
                ..TargetInitOptions::default()
            },
            true,
            false,
        )
        .expect("targeted GTM pack should initialize");

        let manifest = read_manifest(&root).expect("manifest should parse");
        assert_eq!(
            manifest.profile_eval.activation.status.as_deref(),
            Some("needs-review")
        );
        for job in &manifest.jobs {
            let foundation = resolve_product_foundation_for_pack(&root, &manifest, &job.id)
                .expect("foundation should resolve");
            assert_eq!(foundation.status, ProductFoundationStatus::Blocked);
            let gap_ids = foundation
                .selected_facets
                .iter()
                .flat_map(|facet| {
                    facet
                        .gap_refs
                        .iter()
                        .map(|reference| reference.entry_id.as_str())
                })
                .collect::<BTreeSet<_>>();
            let facet_ids = foundation
                .selected_facets
                .iter()
                .map(|facet| facet.id.as_str())
                .collect::<BTreeSet<_>>();
            assert!(gap_ids.contains("product-facts-missing"));
            assert!(gap_ids.contains("icp-actors-missing"));
            assert!(gap_ids.contains("proof-missing"));
            match job.id.as_str() {
                "prospect-fit-or-brief" => {
                    assert!(!facet_ids.contains("outcomes"));
                    assert!(!facet_ids.contains("differentiators"));
                    assert!(!facet_ids.contains("terminology"));
                    assert!(!facet_ids.contains("alternatives"));
                }
                "outbound-copy-brief" => {
                    assert!(facet_ids.contains("outcomes"));
                    assert!(facet_ids.contains("differentiators"));
                    assert!(facet_ids.contains("terminology"));
                    assert!(gap_ids.contains("outcomes-missing"));
                    assert!(gap_ids.contains("differentiators-missing"));
                    assert!(gap_ids.contains("terminology-missing"));
                    assert!(!facet_ids.contains("alternatives"));
                }
                "outbound-copy-review" => {
                    assert!(facet_ids.contains("alternatives"));
                    assert!(facet_ids.contains("terminology"));
                    assert!(gap_ids.contains("alternatives-missing"));
                    assert!(gap_ids.contains("terminology-missing"));
                    assert!(!facet_ids.contains("outcomes"));
                    assert!(!facet_ids.contains("differentiators"));
                }
                unexpected => panic!("unexpected targeted GTM job {unexpected}"),
            }
        }
        let readme = std::fs::read_to_string(root.join(".mdp/README.md"))
            .expect("orientation README should exist");
        assert!(readme.contains("Company B"));
        assert!(readme.contains("Product facts missing"));
        assert!(readme.contains("ICP and actor evidence missing"));
        assert!(readme.contains("Outcome authority missing"));
        assert!(readme.contains("Differentiator authority missing"));
        assert!(readme.contains("Terminology authority missing"));
        assert!(readme.contains("Alternative authority missing"));
        assert!(!readme.contains("Company B improves"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proposal_init_has_ready_foundations_and_public_safe_orientation() {
        let root = std::env::temp_dir().join(format!("mdp-foundation-proposal-{}", nonce()));
        init_pack(&root, PROPOSAL_TEMPLATE_NAME, "proposal", true, false)
            .expect("proposal pack should initialize");

        let manifest = read_manifest(&root).expect("manifest should parse");
        let mut shared_prompt_policy: Option<Value> = None;
        for job in &manifest.jobs {
            let foundation = resolve_product_foundation_for_pack(&root, &manifest, &job.id)
                .expect("foundation should resolve");
            assert_eq!(
                foundation.status,
                ProductFoundationStatus::Ready,
                "{}",
                job.id
            );
            let facet_ids = foundation
                .selected_facets
                .iter()
                .map(|facet| facet.id.as_str())
                .collect::<BTreeSet<_>>();
            let expected_offer = format!("{}-offer", job.id);
            let expected_motion = format!("{}-motion", job.id);
            assert!(facet_ids.contains(expected_offer.as_str()), "{}", job.id);
            assert!(facet_ids.contains(expected_motion.as_str()), "{}", job.id);
            for other_job in manifest.jobs.iter().filter(|other| other.id != job.id) {
                assert!(
                    !facet_ids.contains(format!("{}-offer", other_job.id).as_str()),
                    "{} loaded another job's offer",
                    job.id
                );
                assert!(
                    !facet_ids.contains(format!("{}-motion", other_job.id).as_str()),
                    "{} loaded another job's motion",
                    job.id
                );
            }
            let binding = job
                .model_task
                .as_ref()
                .expect("proposal job should own a prompt");
            assert_eq!(binding.kind, "review");
            let prompt_path = root
                .join(".mdp/prompts")
                .join(format!("{}.yaml", binding.prompt.trim_end_matches("-v1")));
            let prompt = crate::pack_io::read_prompt(&prompt_path)
                .expect("proposal job-owned prompt should parse");
            assert_eq!(prompt.id, binding.prompt);
            assert_eq!(prompt.kind.as_deref(), Some("review"));
            assert_eq!(prompt.version.as_deref(), Some("3"));
            let prompt_value =
                serde_json::to_value(&prompt).expect("proposal job-owned prompt should serialize");
            let input_names = prompt_value["inputs"]
                .as_array()
                .expect("proposal prompt inputs should be an array")
                .iter()
                .filter_map(|input| input["name"].as_str())
                .collect::<BTreeSet<_>>();
            assert!(
                input_names.contains("normalized_input"),
                "{} must consume the canonical neutral v3 normalized_input output",
                job.id
            );
            assert!(
                input_names.contains("routed_context")
                    && !input_names.contains("product_foundation"),
                "{} must consume only the canonical routed context authority",
                job.id
            );
            assert!(
                !input_names.contains("normalized_prospect"),
                "{} must not retain the legacy GTM-shaped normalized_prospect input",
                job.id
            );
            assert!(
                !input_names.contains("normalized_opportunity"),
                "{} must not require the legacy proposal readability alias",
                job.id
            );
            for host_input_name in ["prompt_receipt", "invocation_receipt_sha256"] {
                let host_input = prompt_value["inputs"]
                    .as_array()
                    .and_then(|inputs| inputs.iter().find(|input| input["name"] == host_input_name))
                    .unwrap_or_else(|| {
                        panic!("proposal prompt must declare {host_input_name} input")
                    });
                assert_eq!(host_input["required"], true, "{}", job.id);
                assert_eq!(host_input["producer"], "host", "{}", job.id);
                assert!(
                    prompt_value["output_contract"]["schema"]["properties"]
                        ["source_summary"]["properties"]["inputs_used"]["items"]["enum"]
                        .as_array()
                        .expect("inputs_used enum should be an array")
                        .iter()
                        .any(|name| name == host_input_name),
                    "{} must allow {host_input_name} in inputs_used",
                    job.id
                );
            }
            for required_path in [
                &prompt_value["output_contract"]["required_top_level"],
                &prompt_value["output_contract"]["schema"]["required"],
            ] {
                assert!(
                    required_path
                        .as_array()
                        .expect("governed prompt required fields should be an array")
                        .iter()
                        .any(|field| field == "invocation_receipt_sha256"),
                    "{} must require invocation_receipt_sha256",
                    job.id
                );
                assert!(
                    required_path
                        .as_array()
                        .expect("governed prompt required fields should be an array")
                        .iter()
                        .any(|field| field == "context_sha256"),
                    "{} must require context_sha256",
                    job.id
                );
            }
            assert!(
                prompt_value["output_contract"]["schema"]["properties"]
                    ["invocation_receipt_sha256"]
                    .is_object(),
                "{} must schema invocation_receipt_sha256",
                job.id
            );
            assert!(
                prompt_value["output_contract"]["example"]["invocation_receipt_sha256"]
                    .as_str()
                    .is_some_and(|hash| hash.len() == 64),
                "{} must example invocation_receipt_sha256",
                job.id
            );
            let final_checklist = prompt_value["final_checklist"]
                .as_array()
                .expect("governed proposal prompt should declare a final checklist");
            for required_check in [
                "MDP wraps and validates prompt, context, receipt, and input identities after generation.",
                "Return only semantic governed fields.",
            ] {
                assert!(
                    final_checklist.iter().any(|check| check == required_check),
                    "{} must state the host-envelope boundary exactly",
                    job.id
                );
            }
            assert!(
                final_checklist.iter().all(|check| {
                    check.as_str().is_none_or(|text| {
                        !text.contains("invocation receipt hashes exactly match")
                    })
                }),
                "{} must not imply invocation_receipt_sha256 is stored inside prompt_receipt",
                job.id
            );

            let artifact_schema =
                &prompt_value["output_contract"]["schema"]["properties"]["artifact"];
            match job.id.as_str() {
                "compliance-review" => {
                    assert_eq!(
                        artifact_schema["properties"]["human_review_required"]["const"],
                        true
                    );
                    assert_eq!(artifact_schema["properties"]["requirements"]["minItems"], 1);
                    let ready = &artifact_schema["allOf"][0]["then"]["properties"];
                    assert_eq!(ready["review_status"]["const"], "ready-for-human-review");
                    assert_eq!(ready["missing_requirements_or_sources"]["maxItems"], 0);
                    assert_eq!(ready["requirements"]["minItems"], 1);

                    let schema = &prompt_value["output_contract"]["schema"];
                    let mut ready_example = prompt_value["output_contract"]["example"].clone();
                    ready_example["artifact"]["status"] = json!("ready");
                    ready_example["artifact"]["review_status"] = json!("ready-for-human-review");
                    ready_example["artifact"]["missing_requirements_or_sources"] = json!([]);
                    ready_example["artifact"]["requirements"][0]["coverage_status"] =
                        json!("supported");
                    ready_example["artifact"]["requirements"][0]["source"] =
                        json!("synthetic-requirements");
                    ready_example["artifact"]["requirements"][0]["gap"] = json!("N/A");
                    assert!(
                        jsonschema::draft202012::validate(schema, &ready_example).is_ok(),
                        "ready compliance example should satisfy the bounded schema"
                    );
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["requirements"][0]["coverage_status"] = json!("partial");
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["missing_requirements_or_sources"] =
                        json!(["missing source"]);
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["human_review_required"] = json!(false);
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["requirements"] = json!([]);
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["requirements"][0]["source"] = json!("N/A");
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                }
                "red-team-review" => {
                    assert_eq!(
                        artifact_schema["properties"]["human_review_required"]["const"],
                        true
                    );
                    let ready = &artifact_schema["allOf"][0]["then"]["properties"];
                    assert_eq!(ready["review_status"]["const"], "ready-for-human-review");
                    assert_eq!(ready["gaps"]["maxItems"], 0);

                    let schema = &prompt_value["output_contract"]["schema"];
                    let mut ready_example = prompt_value["output_contract"]["example"].clone();
                    ready_example["artifact"]["status"] = json!("ready");
                    ready_example["artifact"]["review_status"] = json!("ready-for-human-review");
                    ready_example["artifact"]["gaps"] = json!([]);
                    assert!(
                        jsonschema::draft202012::validate(schema, &ready_example).is_ok(),
                        "ready red-team example should satisfy the bounded schema"
                    );
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["gaps"] = json!([{
                        "severity": "blocker",
                        "issue_type": "missing-source",
                        "issue": "Required review material is missing.",
                        "affected_section": "N/A",
                        "evidence": [],
                        "pack_reference": "N/A",
                        "confidence": "unknown",
                        "owner_or_question": "Who owns the missing material?",
                        "next_action": "Supply the material."
                    }]);
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["human_review_required"] = json!(false);
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                    let mut invalid = ready_example.clone();
                    invalid["artifact"]["review_status"] = json!("needs-more-info");
                    assert!(jsonschema::draft202012::validate(schema, &invalid).is_err());
                }
                _ => {}
            }

            let shared_keys = [
                "role",
                "target_card_kinds",
                "inputs",
                "instructions",
                "selection_rules",
                "ambiguity_policy",
                "provenance_policy",
                "evidence_policy",
                "negative_examples",
            ];
            let shared = Value::Object(
                shared_keys
                    .into_iter()
                    .map(|key| (key.to_string(), prompt_value[key].clone()))
                    .collect(),
            );
            if let Some(expected) = &shared_prompt_policy {
                assert_eq!(
                    &shared, expected,
                    "{} drifted from the proposal prompts' shared policy contract",
                    job.id
                );
            } else {
                shared_prompt_policy = Some(shared);
            }
        }
        let readme = std::fs::read_to_string(root.join(".mdp/README.md"))
            .expect("orientation README should exist");
        assert!(readme.contains("synthetic"));
        assert!(readme.contains("does not certify compliance"));
        for forbidden in ["raw transcript", "/Users/", "approved for CUI"] {
            assert!(!readme.contains(forbidden), "README leaked {forbidden}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn readme_is_non_authoritative_but_part_of_portable_identity() {
        let root = std::env::temp_dir().join(format!("mdp-readme-authority-{}", nonce()));
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
            .expect("generic GTM pack should initialize");
        let manifest = read_manifest(&root).expect("manifest should parse");
        let job_id = manifest.jobs[0].id.clone();
        let before = resolve_product_foundation_for_pack(&root, &manifest, &job_id)
            .expect("foundation should resolve");
        let hash_before = pack_content_sha256(&root).expect("pack should hash");
        std::fs::write(
            root.join(".mdp/README.md"),
            "# Contradiction\n\nThis prose falsely claims the pack is an AI SDR.\n",
        )
        .expect("README should be writable");
        let after = resolve_product_foundation_for_pack(&root, &manifest, &job_id)
            .expect("foundation should still resolve");
        let hash_after = pack_content_sha256(&root).expect("pack should hash");

        assert_eq!(before, after);
        assert_ne!(hash_before, hash_after);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_dry_run_and_collision_contract_include_orientation_readme() {
        let root = std::env::temp_dir().join(format!("mdp-readme-collision-{}", nonce()));
        let readme_path = root.join(".mdp/README.md");
        std::fs::create_dir_all(root.join(".mdp")).expect("pack directory should exist");
        std::fs::write(&readme_path, "# Human authored\n").expect("README should be writable");

        let dry_run = init_pack_dry_run(&root, "Basic MDP Template", "gtm", false, false)
            .expect("dry run should return plan");
        let readme_plan = dry_run["write_plan"]
            .as_array()
            .expect("write plan array")
            .iter()
            .find(|entry| entry["path"] == readme_path.display().to_string())
            .expect("README plan should be present");
        assert_eq!(readme_plan["action"], "blocked");
        assert_eq!(readme_plan["would_write"], false);

        let error = init_pack(&root, "Basic MDP Template", "gtm", false, false)
            .expect_err("normal init must preserve human-authored README");
        assert!(error.to_string().contains("README.md already exists"));
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("README should remain readable"),
            "# Human authored\n"
        );
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
            .expect("explicit force may replace README");
        assert!(
            std::fs::read_to_string(&readme_path)
                .expect("README should be readable")
                .contains("## Authority")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_gtm_authority_has_no_dangling_repo_local_evidence_locator() {
        let root = std::env::temp_dir().join(format!("mdp-readme-locators-{}", nonce()));
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
            .expect("generic GTM pack should initialize");
        let mut paths = std::fs::read_dir(root.join(".mdp/cards"))
            .expect("card directory should be readable")
            .map(|entry| entry.expect("card entry should be readable").path())
            .collect::<Vec<_>>();
        paths.push(root.join(".mdp/sources.yaml"));
        for path in paths {
            let raw = std::fs::read_to_string(&path).expect("authority file should be readable");
            for dangling in ["README.md", "AGENTS.md", "docs/", "cli/src/"] {
                assert!(
                    !raw.contains(dangling),
                    "{} contains repo-local authority locator {dangling}",
                    path.display()
                );
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_basic_starter_matches_plugin_template() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-golden-{nonce}"));
        init_pack_targeted(
            &root,
            "Basic MDP Template",
            "gtm",
            &TargetInitOptions::default(),
            true,
            false,
        )
        .expect("starter pack should initialize");
        let plugin_template =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/basic");

        let generated_files = collect_files(&root);
        let plugin_files = collect_files(&plugin_template);
        assert_eq!(generated_files, plugin_files);

        for relative in generated_files {
            let generated =
                std::fs::read(root.join(&relative)).expect("generated file should be readable");
            let checked_in = std::fs::read(plugin_template.join(&relative))
                .expect("plugin template file should be readable");
            assert_eq!(generated, checked_in, "template drift in {relative}");
        }

        let claims_prompt =
            std::fs::read_to_string(root.join(".mdp").join("prompts").join("claims-proof.yaml"))
                .expect("claims prompt should be readable");
        assert!(claims_prompt.contains("schema_ref: mdp.prompt-output.card-patches.v0"));
        assert!(claims_prompt.contains("name: runtime_context"));
        assert!(claims_prompt.contains("Use existing_pack_context as the source of truth for pack-owned personas, operator roles, card ids, claims, avoid-rules, output rules, supported channels, and value domains."));
        assert!(claims_prompt.contains("Use runtime_context.now_utc and runtime_context.date_utc only to state when this extraction ran or to compare against explicitly supplied timing metadata."));
        assert!(claims_prompt.contains("Do not infer a domain from company name."));
        assert!(!claims_prompt.contains("\n  schema:\n"));

        let normalization_prompt = std::fs::read_to_string(
            root.join(".mdp")
                .join("prompts")
                .join("normalize-prospect.yaml"),
        )
        .expect("normalization prompt should be readable");
        assert!(normalization_prompt.contains("format: mdp.prompt.v1"));
        assert!(normalization_prompt.contains("kind: normalization"));
        assert!(normalization_prompt.contains("version: gtm-prospect-context.v3"));
        assert!(normalization_prompt.contains("producer: source"));
        assert!(normalization_prompt.contains("name: decision_input_requirements"));
        assert!(normalization_prompt.contains("name: source_binding_sha256"));
        assert!(normalization_prompt.contains("mdp.normalized-decision-input.v3"));
        assert!(normalization_prompt.contains(
            "derived_from may contain only attempt_id values from observed contributor attributes."
        ));
        assert!(normalization_prompt.contains("Never emit observations, normalized_input, source prose, host hashes, outcome, fit, route, readiness, or draft permission."));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_basic_version_claim_is_distinct_and_manifest_backed() {
        let root = std::env::temp_dir().join(format!("mdp-version-claim-{}", nonce()));
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
            .expect("starter pack should initialize");

        let claims = std::fs::read_to_string(root.join(".mdp/cards/claims.yaml"))
            .expect("claims card should be readable");
        assert!(claims.contains("title: Version-declared context"));
        assert!(claims.contains("evidence:\n  - mdp-pack-manifest"));
        assert!(
            !claims
                .contains("Agents should load only the cards returned by route or brief commands.")
        );

        let sources = std::fs::read_to_string(root.join(".mdp/sources.yaml"))
            .expect("source ledger should be readable");
        assert!(sources.contains("id: mdp-pack-manifest"));
        assert!(sources.contains(
            "This MDP pack declares its version and card references in the pack manifest."
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_can_inline_prompt_output_schemas_when_requested() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-inline-schemas-{nonce}"));

        init_pack(&root, "Inline Schema Pack", "gtm", true, true)
            .expect("starter pack should initialize");

        let claims_prompt =
            std::fs::read_to_string(root.join(".mdp").join("prompts").join("claims-proof.yaml"))
                .expect("claims prompt should be readable");
        assert!(claims_prompt.contains("schema_ref: mdp.prompt-output.card-patches.v0"));
        assert!(claims_prompt.contains("\n  schema:\n"));
        assert!(claims_prompt.contains("additionalProperties: false"));
        assert!(claims_prompt.contains("runtime_context:"));

        let normalization_prompt = std::fs::read_to_string(
            root.join(".mdp")
                .join("prompts")
                .join("normalize-prospect.yaml"),
        )
        .expect("normalization prompt should be readable");
        assert!(
            normalization_prompt
                .contains("schema_ref: mdp.prompt-output.prospect-normalization.v0")
        );
        assert!(normalization_prompt.contains("\n  schema:\n"));
        assert!(normalization_prompt.contains("normalized_prospect:"));
        assert!(normalization_prompt.contains("runtime_context:"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_proposal_starter_matches_plugin_template_pack_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-proposal-golden-{nonce}"));
        let result = init_pack(&root, PROPOSAL_TEMPLATE_NAME, "proposal", true, false)
            .expect("proposal pack should initialize");
        let plugin_template =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/proposal");

        let generated_files = collect_files(&root);
        let plugin_files = collect_files(&plugin_template);
        assert_eq!(generated_files, plugin_files);
        assert!(root.join(".mdp").join("briefs").is_dir());
        assert_eq!(result["template"], "proposal");
        assert_eq!(result["example_prospect"], Value::Null);

        for relative in generated_files {
            let generated =
                std::fs::read(root.join(&relative)).expect("generated file should be readable");
            let checked_in = std::fs::read(plugin_template.join(&relative))
                .expect("plugin template file should be readable");
            assert_eq!(generated, checked_in, "template drift in {relative}");
        }

        let normalization =
            crate::pack_io::read_prompt(&root.join(".mdp/prompts/normalize-opportunity.yaml"))
                .expect("proposal normalization prompt should parse");
        assert_eq!(normalization.format, "mdp.prompt.v1");
        assert_eq!(normalization.kind.as_deref(), Some("normalization"));
        assert_eq!(
            normalization.version.as_deref(),
            Some("proposal-opportunity-context.v3")
        );
        assert!(
            normalization
                .inputs
                .iter()
                .all(|input| input.producer.is_some())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proposal_init_uses_custom_name_when_supplied() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-proposal-name-{nonce}"));

        let result = init_pack(&root, "Proposal Pack", "proposal", true, false)
            .expect("proposal pack should initialize");

        let manifest = std::fs::read_to_string(root.join(".mdp").join("manifest.yaml"))
            .expect("proposal manifest should be readable");
        assert!(manifest.contains("id: proposal-pack"));
        assert!(manifest.contains("name: Proposal Pack"));
        assert_eq!(result["name"], "Proposal Pack");
        assert_eq!(result["slug"], "proposal-pack");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proposal_dry_run_reports_template_writes_without_creating_pack() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-proposal-dry-run-{nonce}"));

        let result = init_pack_dry_run(&root, "Proposal Pack", "proposal", false, false)
            .expect("proposal dry run should return plan");

        assert_eq!(result["dry_run"], true);
        assert_eq!(result["template"], "proposal");
        assert!(!root.exists());
        assert!(
            result["write_plan"]
                .as_array()
                .expect("write plan array")
                .iter()
                .any(|entry| entry["path"]
                    == root
                        .join(".mdp")
                        .join("evals")
                        .join("proposal-gaps.yaml")
                        .display()
                        .to_string()
                    && entry["action"] == "create")
        );
    }

    #[test]
    fn unsupported_template_lists_available_templates() {
        let root = std::env::temp_dir().join(format!("mdp-unsupported-template-{}", nonce()));

        let err = init_pack(&root, "Bad Template", "unknown", true, false)
            .expect_err("unknown template should fail");

        assert_eq!(
            err.to_string(),
            "unsupported template 'unknown'; available: gtm, proposal"
        );
    }

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    }

    fn collect_files(root: &Path) -> BTreeSet<String> {
        let mut files = BTreeSet::new();
        collect_files_inner(root, root, &mut files);
        files
    }

    fn collect_files_inner(root: &Path, current: &Path, files: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(current).expect("directory should be readable") {
            let path = entry.expect("entry should be readable").path();
            if path.is_dir() {
                collect_files_inner(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .expect("path should be under root")
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }

    #[test]
    fn init_writes_source_ledger_and_marks_example_prospect_synthetic() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-source-ledger-{nonce}"));

        let result = init_pack(&root, "Source Ledger Pack", "gtm", true, false)
            .expect("starter pack should initialize");

        let source_ledger_path = root.join(".mdp").join("sources.yaml");
        let source_ledger =
            std::fs::read_to_string(&source_ledger_path).expect("source ledger should be readable");
        assert!(source_ledger.contains("mdp.sources.v0"));
        assert!(source_ledger.contains("synthetic-example"));
        assert_eq!(
            result["source_ledger"],
            source_ledger_path.display().to_string()
        );
        assert_eq!(result["example_prospect_kind"], "synthetic-example");

        let prospect_raw = std::fs::read_to_string(root.join("examples").join("clay-row.json"))
            .expect("example prospect should be readable");
        let prospect: serde_json::Value =
            serde_json::from_str(&prospect_raw).expect("example prospect should parse");
        assert_eq!(prospect["source_kind"], "synthetic-example");
        assert_eq!(prospect["synthetic"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_dry_run_reports_writes_without_creating_pack() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-dry-run-{nonce}"));

        let result = init_pack_dry_run(&root, "Dry Run Pack", "gtm", false, false)
            .expect("dry run should return plan");

        assert_eq!(result["dry_run"], true);
        assert!(!root.exists());
        assert!(
            result["write_plan"]
                .as_array()
                .expect("write plan array")
                .iter()
                .any(|entry| entry["path"]
                    == root
                        .join(".mdp")
                        .join("manifest.yaml")
                        .display()
                        .to_string()
                    && entry["action"] == "create")
        );
    }

    #[test]
    fn init_dry_run_reports_existing_example_prospect_conflict() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-dry-run-conflict-{nonce}"));
        let examples_dir = root.join("examples");
        let prospect_path = examples_dir.join("clay-row.json");
        std::fs::create_dir_all(&examples_dir).expect("examples dir should be created");
        std::fs::write(&prospect_path, "{}").expect("example prospect should be written");

        let result = init_pack_dry_run(&root, "Dry Run Pack", "gtm", false, false)
            .expect("dry run should return plan");

        let prospect_plan = result["write_plan"]
            .as_array()
            .expect("write plan array")
            .iter()
            .find(|entry| entry["path"] == prospect_path.display().to_string())
            .expect("prospect plan should be present");
        assert_eq!(prospect_plan["action"], "blocked");
        assert_eq!(prospect_plan["would_write"], false);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_dry_run_reports_existing_decision_input_scenarios_before_writes() {
        let root = std::env::temp_dir().join(format!("mdp-dry-run-scenarios-conflict-{}", nonce()));
        let examples_dir = root.join("examples");
        let scenarios_path = examples_dir.join("decision-input-scenarios.json");
        std::fs::create_dir_all(&examples_dir).expect("examples dir should be created");
        std::fs::write(&scenarios_path, "{}").expect("scenario fixture should be written");

        let result = init_pack_dry_run(&root, "Dry Run Pack", "gtm", false, false)
            .expect("dry run should return plan");
        let scenario_plan = result["write_plan"]
            .as_array()
            .expect("write plan array")
            .iter()
            .find(|entry| entry["path"] == scenarios_path.display().to_string())
            .expect("scenario plan should be present");
        assert_eq!(scenario_plan["action"], "blocked");
        assert_eq!(scenario_plan["would_write"], false);

        let err = init_pack(&root, "Dry Run Pack", "gtm", false, false)
            .expect_err("existing scenarios should fail before writing the pack");
        assert!(err.to_string().contains("decision-input-scenarios.json"));
        assert!(!root.join(".mdp").exists());
        assert_eq!(std::fs::read_to_string(&scenarios_path).unwrap(), "{}");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn custom_pack_name_requires_explicit_target_identity() {
        let root = std::env::temp_dir().join(format!("mdp-target-gate-{}", nonce()));
        let err = init_pack_targeted(
            &root,
            "Company A Messaging",
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                ..TargetInitOptions::default()
            },
            false,
            false,
        )
        .expect_err("custom pack name without target should be ambiguous");

        assert!(err.to_string().contains("target identity is ambiguous"));
        assert!(!root.exists(), "identity gate must run before authoring");
    }

    #[test]
    fn targeted_init_writes_resolved_identity() {
        let root = std::env::temp_dir().join(format!("mdp-resolved-target-{}", nonce()));
        init_pack_targeted(
            &root,
            "Company B Messaging",
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                name: Some("Company B"),
                excluded_terms: &["Company A".to_string()],
                ..TargetInitOptions::default()
            },
            true,
            false,
        )
        .expect("targeted pack should initialize");

        let manifest = std::fs::read_to_string(root.join(".mdp/manifest.yaml"))
            .expect("manifest should be readable");
        assert!(manifest.contains("name: Company B"));
        assert!(manifest.contains("- Company A"));
        let positioning = std::fs::read_to_string(root.join(".mdp/cards/positioning.yaml"))
            .expect("positioning should be readable");
        assert!(positioning.contains("Company B"));
        assert!(!positioning.contains("Company A"));
        let sample: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("examples/prospect-row.json"))
                .expect("sample row should be readable"),
        )
        .expect("sample row should parse");
        assert!(sample.get("company_domain").is_none());
        assert_eq!(sample["signals"], json!([]));
        for entry in std::fs::read_dir(root.join(".mdp/prompts"))
            .expect("prompt directory should be readable")
        {
            let path = entry.expect("prompt entry should be readable").path();
            let raw = std::fs::read_to_string(&path).expect("prompt should be readable");
            let prompt: Value = serde_yaml::from_str(&raw).expect("prompt should parse");
            let example = serde_json::to_string(&prompt["output_contract"]["example"])
                .expect("prompt example should serialize");
            for residue in [
                "PMM",
                "GTM Engineering",
                "persona-gtm-ops",
                "agent-assisted GTM",
                "local decision context",
                "Alex Rivera",
                "ExampleCo",
            ] {
                assert!(
                    !example.contains(residue),
                    "{} retained starter residue '{residue}'",
                    path.display()
                );
            }
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn targeted_init_refuses_cross_target_force_overwrite() {
        let root = std::env::temp_dir().join(format!("mdp-retarget-gate-{}", nonce()));
        init_pack_targeted(
            &root,
            "Company A Messaging",
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                name: Some("Company A"),
                ..TargetInitOptions::default()
            },
            true,
            false,
        )
        .expect("first target should initialize");

        let err = init_pack_targeted(
            &root,
            "Company B Messaging",
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                name: Some("Company B"),
                excluded_terms: &["Company A".to_string()],
                ..TargetInitOptions::default()
            },
            true,
            false,
        )
        .expect_err("cross-target force overwrite should be rejected");
        assert!(err.to_string().contains("refusing to retarget"));
        let manifest = std::fs::read_to_string(root.join(".mdp/manifest.yaml"))
            .expect("manifest should be readable");
        assert!(manifest.contains("name: Company A"));
        assert!(!manifest.contains("name: Company B"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_inventory_requires_descriptor_declared_briefs_directory() {
        let descriptor = template_registry::lookup("gtm").expect("gtm descriptor");
        let root = Path::new(".");
        let mut inventory = build_gtm_inventory(
            root,
            "Basic MDP Template",
            "gtm",
            descriptor,
            None,
            false,
            false,
            false,
        )
        .expect("canonical inventory");
        append_required_directories(descriptor, &mut inventory);
        validate_generated_inventory(descriptor, &inventory).expect("appended inventory");
        inventory.retain(|artifact| artifact.relative != ".mdp/briefs");
        assert!(validate_generated_inventory(descriptor, &inventory).is_err());
    }
}
