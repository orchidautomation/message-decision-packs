use crate::constants::{DEFAULT_DIR, FORMAT_VERSION};
use crate::pack_io::{write_json_file, write_yaml};
use crate::starter::{starter_cards, starter_eval, starter_manifest, starter_prospect};
use crate::utils::slugify;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(crate) fn init_pack(root: &Path, name: &str, template: &str, force: bool) -> Result<Value> {
    if template != "gtm" {
        return Err(anyhow!("unsupported template '{template}'; available: gtm"));
    }
    let pack_dir = root.join(DEFAULT_DIR);
    let cards_dir = pack_dir.join("cards");
    let briefs_dir = pack_dir.join("briefs");
    let evals_dir = pack_dir.join("evals");
    let examples_dir = root.join("examples");
    fs::create_dir_all(&cards_dir).with_context(|| format!("creating {}", cards_dir.display()))?;
    fs::create_dir_all(&briefs_dir)
        .with_context(|| format!("creating {}", briefs_dir.display()))?;
    fs::create_dir_all(&evals_dir).with_context(|| format!("creating {}", evals_dir.display()))?;
    fs::create_dir_all(&examples_dir)
        .with_context(|| format!("creating {}", examples_dir.display()))?;
    let slug = slugify(name);
    let manifest_path = pack_dir.join("manifest.yaml");
    write_yaml(
        &manifest_path,
        &starter_manifest(name, &slug, template),
        force,
    )?;
    for (filename, card) in starter_cards(template) {
        write_yaml(&cards_dir.join(filename), &card, force)?;
    }
    write_yaml(
        &evals_dir.join("linkedin-copy-route.yaml"),
        &starter_eval(),
        force,
    )?;
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
        "cards_dir": cards_dir.display().to_string(),
        "evals_dir": evals_dir.display().to_string(),
        "example_prospect": prospect_path.display().to_string(),
        "next_commands": [
            format!("mdp --json validate --dir {}", root.display()),
            format!("mdp --json route --entries --dir {} --persona \\\"{}\\\" --job \\\"linkedin outbound copy\\\"", root.display(), example_persona),
            format!("mdp --json fit --dir {} --prospect {}", root.display(), prospect_path.display()),
            format!("mdp --json brief --dir {} --prospect {} --channel linkedin", root.display(), prospect_path.display()),
            format!("mdp --json eval --dir {}", root.display())
        ]
    }))
}
