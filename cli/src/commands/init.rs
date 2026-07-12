use crate::constants::{DEFAULT_DIR, FORMAT_VERSION};
use crate::models::TargetIdentity;
use crate::pack_io::{
    planned_directory, planned_json_write_after_dirs, planned_yaml_write_after_dirs, read_manifest,
    write_json_file, write_yaml,
};
use crate::starter::{
    starter_cards, starter_evals, starter_manifest, starter_prompts, starter_prospect,
    starter_source_ledger,
};
use crate::target_starter::{
    target_cards, target_evals, target_manifest, target_prompts, target_prospect,
    target_source_ledger,
};
use crate::utils::slugify;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use serde_yaml::Value as YamlValue;
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

const AVAILABLE_TEMPLATES: &str = "gtm, proposal";
const PROPOSAL_TEMPLATE_NAME: &str = "Proposal Reference Profile Sample";

pub(crate) struct TargetInitOptions<'a> {
    pub(crate) custom_name: bool,
    pub(crate) name: Option<&'a str>,
    pub(crate) kind: &'a str,
    pub(crate) aliases: &'a [String],
    pub(crate) terms: &'a [String],
    pub(crate) excluded_terms: &'a [String],
}

impl Default for TargetInitOptions<'_> {
    fn default() -> Self {
        Self {
            custom_name: false,
            name: None,
            kind: "company",
            aliases: &[],
            terms: &[],
            excluded_terms: &[],
        }
    }
}

const PROPOSAL_TEMPLATE_FILES: &[(&str, &str)] = &[
    (
        ".mdp/manifest.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/manifest.yaml"),
    ),
    (
        ".mdp/sources.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/sources.yaml"),
    ),
    (
        ".mdp/cards/bid-no-bid-rules.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/cards/bid-no-bid-rules.yaml"),
    ),
    (
        ".mdp/cards/compliance-boundaries.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/compliance-boundaries.yaml"
        ),
    ),
    (
        ".mdp/cards/evaluation-criteria.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/evaluation-criteria.yaml"
        ),
    ),
    (
        ".mdp/cards/gaps.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/cards/gaps.yaml"),
    ),
    (
        ".mdp/cards/opportunity-context.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/opportunity-context.yaml"
        ),
    ),
    (
        ".mdp/cards/proof-library.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/cards/proof-library.yaml"),
    ),
    (
        ".mdp/cards/proposal-boundaries.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/proposal-boundaries.yaml"
        ),
    ),
    (
        ".mdp/cards/proposal-output-rules.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/proposal-output-rules.yaml"
        ),
    ),
    (
        ".mdp/cards/proposal-roles.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/cards/proposal-roles.yaml"),
    ),
    (
        ".mdp/cards/requirement-signals.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/requirement-signals.yaml"
        ),
    ),
    (
        ".mdp/cards/requirements-matrix.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/cards/requirements-matrix.yaml"
        ),
    ),
    (
        ".mdp/cards/review-gates.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/cards/review-gates.yaml"),
    ),
    (
        ".mdp/cards/review-outputs.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/cards/review-outputs.yaml"),
    ),
    (
        ".mdp/evals/bid-no-bid-route.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/evals/bid-no-bid-route.yaml"),
    ),
    (
        ".mdp/evals/compliance-route.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/evals/compliance-route.yaml"),
    ),
    (
        ".mdp/evals/compliance-unsupported-claim.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/compliance-unsupported-claim.yaml"
        ),
    ),
    (
        ".mdp/evals/fit-insufficient-context.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/fit-insufficient-context.yaml"
        ),
    ),
    (
        ".mdp/evals/fit-policy-bypass-disqualified.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/fit-policy-bypass-disqualified.yaml"
        ),
    ),
    (
        ".mdp/evals/invented-proof-guardrail.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/invented-proof-guardrail.yaml"
        ),
    ),
    (
        ".mdp/evals/normalize-opportunity-insufficient-context.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/normalize-opportunity-insufficient-context.yaml"
        ),
    ),
    (
        ".mdp/evals/normalize-opportunity-invalid-source-kind.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/normalize-opportunity-invalid-source-kind.yaml"
        ),
    ),
    (
        ".mdp/evals/normalize-opportunity-missing-required-attribute.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/normalize-opportunity-missing-required-attribute.yaml"
        ),
    ),
    (
        ".mdp/evals/normalize-opportunity-missing-required-signal.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/normalize-opportunity-missing-required-signal.yaml"
        ),
    ),
    (
        ".mdp/evals/normalize-opportunity-output.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/normalize-opportunity-output.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-claim-source-ref-missing.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-claim-source-ref-missing.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-connective-text.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-connective-text.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-connective-too-long.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-connective-too-long.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-fake-id.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-fake-id.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-gap-only-safe.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-gap-only-safe.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-malformed-artifact.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-malformed-artifact.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-min-segments-violation.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-min-segments-violation.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-missing-binding.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-missing-binding.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-missing-required-segment.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-missing-required-segment.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-unsupported-claim.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-unsupported-claim.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-output-valid-binding.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-output-valid-binding.yaml"
        ),
    ),
    (
        ".mdp/evals/proof-review-route.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/evals/proof-review-route.yaml"
        ),
    ),
    (
        ".mdp/evals/proposal-gaps.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/evals/proposal-gaps.yaml"),
    ),
    (
        ".mdp/evals/red-team-route.yaml",
        include_str!("../../../plugin/assets/templates/proposal/.mdp/evals/red-team-route.yaml"),
    ),
    (
        ".mdp/prompts/normalize-opportunity.yaml",
        include_str!(
            "../../../plugin/assets/templates/proposal/.mdp/prompts/normalize-opportunity.yaml"
        ),
    ),
    (
        "examples/proof-output/claim-source-ref-missing.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/claim-source-ref-missing.json"
        ),
    ),
    (
        "examples/proof-output/connective-text.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/connective-text.json"
        ),
    ),
    (
        "examples/proof-output/connective-too-long.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/connective-too-long.json"
        ),
    ),
    (
        "examples/proof-output/fake-id.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/fake-id.json"
        ),
    ),
    (
        "examples/proof-output/gap-only-safe.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/gap-only-safe.json"
        ),
    ),
    (
        "examples/proof-output/malformed-artifact.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/malformed-artifact.json"
        ),
    ),
    (
        "examples/proof-output/min-segments-violation.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/min-segments-violation.json"
        ),
    ),
    (
        "examples/proof-output/missing-binding.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/missing-binding.json"
        ),
    ),
    (
        "examples/proof-output/missing-required-segment.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/missing-required-segment.json"
        ),
    ),
    (
        "examples/proof-output/unsupported-claim.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/unsupported-claim.json"
        ),
    ),
    (
        "examples/proof-output/valid-binding.json",
        include_str!(
            "../../../plugin/assets/templates/proposal/examples/proof-output/valid-binding.json"
        ),
    ),
];

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn init_pack(
    root: &Path,
    name: &str,
    template: &str,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    init_pack_targeted(
        root,
        name,
        template,
        &TargetInitOptions::default(),
        force,
        include_output_schemas,
    )
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
        target_options.terms,
        target_options.excluded_terms,
    )?;
    match template {
        "gtm" => init_gtm_pack(
            root,
            name,
            template,
            target.as_ref(),
            force,
            include_output_schemas,
        ),
        "proposal" => init_proposal_pack(root, name, force),
        _ => Err(unsupported_template(template)),
    }
}

fn init_gtm_pack(
    root: &Path,
    name: &str,
    template: &str,
    target: Option<&TargetIdentity>,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    validate_target_destination(root, target)?;
    let pack_dir = root.join(DEFAULT_DIR);
    let cards_dir = pack_dir.join("cards");
    let briefs_dir = pack_dir.join("briefs");
    let evals_dir = pack_dir.join("evals");
    let prompts_dir = pack_dir.join("prompts");
    let examples_dir = root.join("examples");
    fs::create_dir_all(&cards_dir).with_context(|| format!("creating {}", cards_dir.display()))?;
    fs::create_dir_all(&briefs_dir)
        .with_context(|| format!("creating {}", briefs_dir.display()))?;
    fs::create_dir_all(&evals_dir).with_context(|| format!("creating {}", evals_dir.display()))?;
    fs::create_dir_all(&prompts_dir)
        .with_context(|| format!("creating {}", prompts_dir.display()))?;
    fs::create_dir_all(&examples_dir)
        .with_context(|| format!("creating {}", examples_dir.display()))?;
    let slug = slugify(name);
    let manifest_path = pack_dir.join("manifest.yaml");
    if let Some(target) = target {
        write_yaml(
            &manifest_path,
            &target_manifest(name, &slug, template, target),
            force,
        )?;
    } else {
        write_yaml(
            &manifest_path,
            &starter_manifest(name, &slug, template),
            force,
        )?;
    }
    let source_ledger_path = pack_dir.join("sources.yaml");
    let source_ledger = target
        .map(target_source_ledger)
        .unwrap_or_else(|| starter_source_ledger(template));
    write_yaml(&source_ledger_path, &source_ledger, force)?;
    let cards = target
        .map(target_cards)
        .unwrap_or_else(|| starter_cards(template));
    for (filename, card) in cards {
        write_yaml(&cards_dir.join(filename), &card, force)?;
    }
    let evals = target.map(target_evals).unwrap_or_else(starter_evals);
    for (filename, eval) in evals {
        write_yaml(&evals_dir.join(filename), &eval, force)?;
    }
    let prompts = target
        .map(|target| target_prompts(target, include_output_schemas))
        .unwrap_or_else(|| starter_prompts(include_output_schemas));
    for (filename, prompt) in prompts {
        write_yaml(&prompts_dir.join(filename), &prompt, force)?;
    }
    let prospect_path = examples_dir.join(if target.is_some() {
        "prospect-row.json"
    } else {
        "clay-row.json"
    });
    if prospect_path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            prospect_path.display()
        ));
    }
    let prospect = target
        .map(target_prospect)
        .unwrap_or_else(|| starter_prospect(template));
    write_json_file(&prospect_path, &prospect)?;
    Ok(init_payload(
        root,
        &pack_dir,
        &manifest_path,
        &source_ledger_path,
        &cards_dir,
        &evals_dir,
        &prompts_dir,
        &prospect_path,
        target,
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
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
        target_options.terms,
        target_options.excluded_terms,
    )?;
    match template {
        "gtm" => init_gtm_pack_dry_run(
            root,
            name,
            template,
            target.as_ref(),
            force,
            include_output_schemas,
        ),
        "proposal" => init_proposal_pack_dry_run(root, name, force),
        _ => Err(unsupported_template(template)),
    }
}

fn init_gtm_pack_dry_run(
    root: &Path,
    name: &str,
    template: &str,
    target: Option<&TargetIdentity>,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    validate_target_destination(root, target)?;
    let pack_dir = root.join(DEFAULT_DIR);
    let cards_dir = pack_dir.join("cards");
    let briefs_dir = pack_dir.join("briefs");
    let evals_dir = pack_dir.join("evals");
    let prompts_dir = pack_dir.join("prompts");
    let examples_dir = root.join("examples");
    let manifest_path = pack_dir.join("manifest.yaml");
    let source_ledger_path = pack_dir.join("sources.yaml");
    let prospect_path = examples_dir.join(if target.is_some() {
        "prospect-row.json"
    } else {
        "clay-row.json"
    });
    let mut payload = init_payload(
        root,
        &pack_dir,
        &manifest_path,
        &source_ledger_path,
        &cards_dir,
        &evals_dir,
        &prompts_dir,
        &prospect_path,
        target,
    );
    let slug = slugify(name);
    let mut write_plan = vec![
        planned_directory(&pack_dir),
        planned_directory(&cards_dir),
        planned_directory(&briefs_dir),
        planned_directory(&evals_dir),
        planned_directory(&prompts_dir),
        planned_directory(&examples_dir),
        planned_yaml_write_after_dirs(&manifest_path, force),
        planned_yaml_write_after_dirs(&source_ledger_path, force),
    ];
    let cards = target
        .map(target_cards)
        .unwrap_or_else(|| starter_cards(template));
    for (filename, _) in cards {
        write_plan.push(planned_yaml_write_after_dirs(
            &cards_dir.join(filename),
            force,
        ));
    }
    let evals = target.map(target_evals).unwrap_or_else(starter_evals);
    for (filename, _) in evals {
        write_plan.push(planned_yaml_write_after_dirs(
            &evals_dir.join(filename),
            force,
        ));
    }
    let prompts = target
        .map(|target| target_prompts(target, include_output_schemas))
        .unwrap_or_else(|| starter_prompts(include_output_schemas));
    for (filename, _) in prompts {
        write_plan.push(planned_yaml_write_after_dirs(
            &prompts_dir.join(filename),
            force,
        ));
    }
    write_plan.push(planned_json_write_after_dirs(&prospect_path, force));
    if let Some(object) = payload.as_object_mut() {
        object.insert("dry_run".to_string(), json!(true));
        object.insert("template".to_string(), json!(template));
        object.insert("slug".to_string(), json!(slug));
        object.insert("force".to_string(), json!(force));
        object.insert("write_plan".to_string(), Value::Array(write_plan));
    }
    Ok(payload)
}

fn resolve_target_identity(
    custom_name: bool,
    template: &str,
    target_name: Option<&str>,
    target_kind: &str,
    target_aliases: &[String],
    target_terms: &[String],
    exclude_terms: &[String],
) -> Result<Option<TargetIdentity>> {
    let has_target_details = !target_aliases.is_empty()
        || !target_terms.is_empty()
        || !exclude_terms.is_empty()
        || target_kind != "company";
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
    let mut external_terms = vec![target_name.to_string()];
    extend_unique(&mut external_terms, target_terms);
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

fn init_proposal_pack(root: &Path, name: &str, force: bool) -> Result<Value> {
    for directory in proposal_template_dirs(root) {
        fs::create_dir_all(&directory)
            .with_context(|| format!("creating {}", directory.display()))?;
    }
    for (relative_path, contents) in PROPOSAL_TEMPLATE_FILES {
        let contents = proposal_template_contents(relative_path, contents, name)?;
        write_embedded_text(root, relative_path, contents.as_ref(), force)?;
    }
    Ok(proposal_init_payload(root, name))
}

fn init_proposal_pack_dry_run(root: &Path, name: &str, force: bool) -> Result<Value> {
    let mut payload = proposal_init_payload(root, name);
    let mut write_plan = proposal_template_dirs(root)
        .into_iter()
        .map(|path| planned_directory(&path))
        .collect::<Vec<_>>();
    for (relative_path, _) in PROPOSAL_TEMPLATE_FILES {
        let target = root.join(relative_path);
        let planned_write = if relative_path.ends_with(".json") {
            planned_json_write_after_dirs(&target, force)
        } else {
            planned_yaml_write_after_dirs(&target, force)
        };
        write_plan.push(planned_write);
    }
    if let Some(object) = payload.as_object_mut() {
        object.insert("dry_run".to_string(), json!(true));
        object.insert("template".to_string(), json!("proposal"));
        object.insert("slug".to_string(), json!(slugify(name)));
        object.insert("force".to_string(), json!(force));
        object.insert("write_plan".to_string(), Value::Array(write_plan));
    }
    Ok(payload)
}

fn proposal_template_dirs(root: &Path) -> Vec<PathBuf> {
    let pack_dir = root.join(DEFAULT_DIR);
    vec![
        pack_dir.clone(),
        pack_dir.join("briefs"),
        pack_dir.join("cards"),
        pack_dir.join("evals"),
        pack_dir.join("prompts"),
        root.join("examples").join("proof-output"),
    ]
}

fn write_embedded_text(
    root: &Path,
    relative_path: &str,
    contents: &str,
    force: bool,
) -> Result<()> {
    let path = root.join(relative_path);
    if path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))
}

fn proposal_template_contents(
    relative_path: &str,
    contents: &'static str,
    name: &str,
) -> Result<Cow<'static, str>> {
    if relative_path != ".mdp/manifest.yaml" || name == PROPOSAL_TEMPLATE_NAME {
        return Ok(Cow::Borrowed(contents));
    }
    let mut manifest: YamlValue =
        serde_yaml::from_str(contents).context("parsing embedded proposal manifest")?;
    let map = manifest
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("embedded proposal manifest must be a mapping"))?;
    map.insert(
        YamlValue::String("id".to_string()),
        YamlValue::String(slugify(name)),
    );
    map.insert(
        YamlValue::String("name".to_string()),
        YamlValue::String(name.to_string()),
    );
    Ok(Cow::Owned(
        serde_yaml::to_string(&manifest).context("serializing embedded proposal manifest")?,
    ))
}

fn unsupported_template(template: &str) -> anyhow::Error {
    anyhow!("unsupported template '{template}'; available: {AVAILABLE_TEMPLATES}")
}

fn init_payload(
    root: &Path,
    pack_dir: &Path,
    manifest_path: &Path,
    source_ledger_path: &Path,
    cards_dir: &Path,
    evals_dir: &Path,
    prompts_dir: &Path,
    prospect_path: &Path,
    target: Option<&TargetIdentity>,
) -> Value {
    let example_persona = if target.is_some() {
        "Operator"
    } else {
        "GTM Engineering"
    };
    let example_job = target
        .map(|target| format!("review evidence gaps for {}", target.name))
        .unwrap_or_else(|| "linkedin outbound copy".to_string());
    json!({
        "format": FORMAT_VERSION,
        "root": root.display().to_string(),
        "pack_dir": pack_dir.display().to_string(),
        "manifest": manifest_path.display().to_string(),
        "source_ledger": source_ledger_path.display().to_string(),
        "cards_dir": cards_dir.display().to_string(),
        "evals_dir": evals_dir.display().to_string(),
        "prompts_dir": prompts_dir.display().to_string(),
        "example_prospect": prospect_path.display().to_string(),
        "example_prospect_kind": "synthetic-example",
        "next_commands": [
            format!("mdp --json validate --dir {}", root.display()),
            format!("mdp --json route --entries --dir {} --persona \\\"{}\\\" --job \\\"{}\\\"", root.display(), example_persona, example_job),
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
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generated_basic_starter_matches_plugin_template() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-golden-{nonce}"));
        init_pack(&root, "Basic MDP Template", "gtm", true, false)
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
        assert!(normalization_prompt.contains("lead_input_requirements.value_contracts"));
        assert!(normalization_prompt.contains("lead_input_requirements.attribute_definitions"));
        assert!(normalization_prompt.contains("name: runtime_context"));
        assert!(normalization_prompt.contains("Do not hardcode fiscal year or infer customer-specific calendars from the current date."));
        assert!(normalization_prompt.contains("Invalid-value example:"));

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
        let mut plugin_files = collect_files(&plugin_template);
        plugin_files.remove("README.md");
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
}
