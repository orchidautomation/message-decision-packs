use crate::constants::{DEFAULT_DIR, FORMAT_VERSION};
use crate::pack_io::{write_json_file, write_yaml};
use crate::starter::{
    starter_cards, starter_evals, starter_manifest, starter_prompts, starter_prospect,
    starter_source_ledger,
};
use crate::utils::slugify;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(crate) fn init_pack(
    root: &Path,
    name: &str,
    template: &str,
    force: bool,
    include_output_schemas: bool,
) -> Result<Value> {
    if template != "gtm" {
        return Err(anyhow!("unsupported template '{template}'; available: gtm"));
    }
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
    write_yaml(
        &manifest_path,
        &starter_manifest(name, &slug, template),
        force,
    )?;
    let source_ledger_path = pack_dir.join("sources.yaml");
    write_yaml(&source_ledger_path, &starter_source_ledger(template), force)?;
    for (filename, card) in starter_cards(template) {
        write_yaml(&cards_dir.join(filename), &card, force)?;
    }
    for (filename, eval) in starter_evals() {
        write_yaml(&evals_dir.join(filename), &eval, force)?;
    }
    for (filename, prompt) in starter_prompts(include_output_schemas) {
        write_yaml(&prompts_dir.join(filename), &prompt, force)?;
    }
    let prospect_path = examples_dir.join("clay-row.json");
    if prospect_path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            prospect_path.display()
        ));
    }
    write_json_file(&prospect_path, &starter_prospect(template))?;
    let example_persona = "GTM Engineering";
    Ok(json!({
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
            format!("mdp --json route --entries --dir {} --persona \\\"{}\\\" --job \\\"linkedin outbound copy\\\"", root.display(), example_persona),
            format!("mdp --json fit --dir {} --prospect {}", root.display(), prospect_path.display()),
            format!("mdp --json --summary brief --dir {} --prospect {} --channel linkedin", root.display(), prospect_path.display()),
            format!("mdp --json eval --dir {}", root.display())
        ]
    }))
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
        assert!(!claims_prompt.contains("\n  schema:\n"));

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

        let _ = std::fs::remove_dir_all(root);
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
}
