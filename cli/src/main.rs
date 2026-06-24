use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const FORMAT_NAME: &str = "Message Decision Pack";
const FORMAT_VERSION: &str = "mdp.v0";
const DEFAULT_DIR: &str = ".mdp";

#[derive(Parser)]
#[command(name = "mdp")]
#[command(about = "Author and route modular message decision packs for GTM agents")]
#[command(version)]
struct Cli {
    #[arg(long, global = true, help = "Emit stable machine-readable JSON")]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a starter MDP package")]
    Init {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value = "gtm")]
        template: String,
        #[arg(long, help = "Overwrite existing starter files")]
        force: bool,
    },
    #[command(about = "Report local setup and pack health")]
    Doctor {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    #[command(about = "Validate manifest and card references")]
    Validate {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    #[command(about = "Explain what an agent should load")]
    Explain {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        persona: Option<String>,
    },
    #[command(about = "Route a job to the minimal cards an agent should load")]
    Route {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        persona: String,
        #[arg(long)]
        job: String,
    },
    #[command(about = "Build a message brief from a pack and enriched prospect JSON")]
    Brief {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        prospect: PathBuf,
        #[arg(long, default_value = "linkedin")]
        channel: String,
        #[arg(long)]
        job: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    #[command(about = "Generate deterministic demo copy from a pack and prospect JSON")]
    Copy {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        prospect: PathBuf,
        #[arg(long, default_value = "linkedin")]
        channel: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    #[command(about = "Emit an agent-readable copy or decision brief")]
    EmitBrief {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        persona: String,
        #[arg(long)]
        motion: Option<String>,
        #[arg(long)]
        job: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    #[command(about = "Compile a bounded portable representation with card hashes")]
    Pack {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    #[command(about = "Print a schema contract")]
    Schema {
        #[arg(value_enum)]
        target: SchemaTarget,
    },
}

#[derive(Clone, ValueEnum)]
enum SchemaTarget {
    Manifest,
    Card,
    Brief,
    Prospect,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    id: String,
    name: String,
    version: String,
    description: Option<String>,
    personas: Vec<String>,
    cards: Vec<CardRef>,
    policy: Policy,
    provenance: Provenance,
}

#[derive(Debug, Serialize, Deserialize)]
struct CardRef {
    id: String,
    path: String,
    kind: CardKind,
    description: String,
    #[serde(default)]
    personas: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
enum CardKind {
    Personas,
    Pains,
    Motions,
    Hooks,
    AvoidRules,
    CopyPatterns,
    Ctas,
}

#[derive(Debug, Serialize, Deserialize)]
struct Policy {
    progressive_disclosure: bool,
    load_manifest_first: bool,
    max_cards_per_route: usize,
    json_contract: String,
    no_auth_required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Provenance {
    owner: String,
    created_by: String,
    notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Card {
    id: String,
    kind: CardKind,
    title: String,
    description: String,
    #[serde(default)]
    personas: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    id: String,
    title: String,
    body: String,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    avoid: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Prospect {
    name: String,
    title: String,
    company: String,
    #[serde(default)]
    linkedin_url: Option<String>,
    #[serde(default)]
    company_url: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    trigger: Option<String>,
    #[serde(default)]
    persona: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    if let Err(err) = run(cli) {
        let _ = print_error(json_mode, err);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init {
            name,
            dir,
            template,
            force,
        } => {
            let data = init_pack(&dir, &name, &template, force)?;
            print_output(cli.json, "init", data)
        }
        Commands::Doctor { dir } => print_output(cli.json, "doctor", doctor(&dir)),
        Commands::Validate { dir } => print_output(cli.json, "validate", validate_pack(&dir)?),
        Commands::Explain { dir, persona } => {
            print_output(cli.json, "explain", explain(&dir, persona.as_deref())?)
        }
        Commands::Route { dir, persona, job } => {
            print_output(cli.json, "route", route(&dir, &persona, &job)?)
        }
        Commands::Brief {
            dir,
            prospect,
            channel,
            job,
            out,
        } => {
            let data = prospect_brief(&dir, &prospect, &channel, job.as_deref())?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "brief", data)
        }
        Commands::Copy {
            dir,
            prospect,
            channel,
            out,
        } => {
            let data = demo_copy(&dir, &prospect, &channel)?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "copy", data)
        }
        Commands::EmitBrief {
            dir,
            persona,
            motion,
            job,
            out,
        } => {
            let data = emit_brief(&dir, &persona, motion.as_deref(), job.as_deref())?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "emit-brief", data)
        }
        Commands::Pack { dir, out } => {
            let data = pack(&dir)?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "pack", data)
        }
        Commands::Schema { target } => print_output(cli.json, "schema", schema(target)),
    }
}

fn print_output(json_mode: bool, command: &str, data: Value) -> Result<()> {
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"ok": true, "command": command, "data": data}))?
        );
    } else {
        print_human(command, &data)?;
    }
    Ok(())
}

fn print_error(json_mode: bool, err: anyhow::Error) -> Result<()> {
    let payload = json!({"ok": false, "error": {"code": "mdp_error", "message": err.to_string(), "details": []}});
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        eprintln!("error: {}", err);
    }
    Ok(())
}

fn print_human(command: &str, data: &Value) -> Result<()> {
    match command {
        "init" => {
            println!(
                "Created MDP package at {}",
                data["pack_dir"].as_str().unwrap_or("")
            );
            println!(
                "Next: mdp validate --dir {}",
                data["root"].as_str().unwrap_or(".")
            );
        }
        "doctor" | "validate" => {
            println!(
                "{}: {}",
                command,
                if data["valid"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "needs attention"
                }
            );
            if let Some(items) = data["issues"].as_array() {
                for item in items {
                    println!("- {}", item.as_str().unwrap_or(""));
                }
            }
        }
        _ => println!("{}", serde_json::to_string_pretty(data)?),
    }
    Ok(())
}

fn init_pack(root: &Path, name: &str, template: &str, force: bool) -> Result<Value> {
    if !matches!(template, "gtm" | "rillet") {
        return Err(anyhow!(
            "unsupported template '{template}'; available: gtm, rillet"
        ));
    }
    let pack_dir = root.join(DEFAULT_DIR);
    let cards_dir = pack_dir.join("cards");
    let briefs_dir = pack_dir.join("briefs");
    let examples_dir = root.join("examples");
    fs::create_dir_all(&cards_dir).with_context(|| format!("creating {}", cards_dir.display()))?;
    fs::create_dir_all(&briefs_dir)
        .with_context(|| format!("creating {}", briefs_dir.display()))?;
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
    let prospect_path = examples_dir.join("clay-row.json");
    if prospect_path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            prospect_path.display()
        ));
    }
    write_json_file(&prospect_path, &starter_prospect(template))?;
    let example_persona = if template == "rillet" {
        "VP Finance"
    } else {
        "GTM Engineering"
    };
    Ok(json!({
        "format": FORMAT_VERSION,
        "root": root.display().to_string(),
        "pack_dir": pack_dir.display().to_string(),
        "manifest": manifest_path.display().to_string(),
        "cards_dir": cards_dir.display().to_string(),
        "example_prospect": prospect_path.display().to_string(),
        "next_commands": [
            format!("mdp --json validate --dir {}", root.display()),
            format!("mdp --json route --dir {} --persona \\\"{}\\\" --job \\\"linkedin outbound\\\"", root.display(), example_persona),
            format!("mdp --json copy --dir {} --prospect {}", root.display(), prospect_path.display())
        ]
    }))
}

fn doctor(root: &Path) -> Value {
    let pack_dir = root.join(DEFAULT_DIR);
    let manifest_path = pack_dir.join("manifest.yaml");
    let mut issues = Vec::new();
    let mut checks = BTreeMap::new();
    checks.insert("auth_required", json!(false));
    checks.insert("offline_mode", json!(true));
    checks.insert("pack_dir_exists", json!(pack_dir.exists()));
    checks.insert("manifest_exists", json!(manifest_path.exists()));
    if !pack_dir.exists() {
        issues.push(format!("missing {}", pack_dir.display()));
    }
    if !manifest_path.exists() {
        issues.push(format!("missing {}", manifest_path.display()));
    }
    if manifest_path.exists() {
        match read_manifest(root) {
            Ok(manifest) => {
                checks.insert("format", json!(manifest.format));
                checks.insert("manifest_parseable", json!(true));
            }
            Err(err) => {
                checks.insert("manifest_parseable", json!(false));
                issues.push(err.to_string());
            }
        }
    }
    json!({
        "tool": "mdp",
        "format_name": FORMAT_NAME,
        "expected_format": FORMAT_VERSION,
        "valid": issues.is_empty(),
        "checks": checks,
        "issues": issues,
        "setup": if issues.is_empty() { Value::Null } else { json!("Run `mdp init --name <name>` from the repo or workspace root.") }
    })
}

fn validate_pack(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut issues = Vec::new();
    let mut card_ids = BTreeSet::new();
    let mut loaded_cards = Vec::new();
    if manifest.format != FORMAT_VERSION {
        issues.push(format!(
            "manifest format must be {FORMAT_VERSION}, found {}",
            manifest.format
        ));
    }
    if manifest.personas.is_empty() {
        issues.push("manifest personas must not be empty".to_string());
    }
    if manifest.cards.is_empty() {
        issues.push("manifest cards must not be empty".to_string());
    }
    if !manifest.policy.progressive_disclosure {
        issues.push("policy.progressive_disclosure should be true".to_string());
    }
    for card_ref in &manifest.cards {
        if !card_ids.insert(card_ref.id.clone()) {
            issues.push(format!("duplicate card id {}", card_ref.id));
        }
        let path = root.join(DEFAULT_DIR).join(&card_ref.path);
        match read_card(&path) {
            Ok(card) => {
                if card.id != card_ref.id {
                    issues.push(format!(
                        "{} id mismatch: manifest has {}, card has {}",
                        path.display(),
                        card_ref.id,
                        card.id
                    ));
                }
                if card.kind != card_ref.kind {
                    issues.push(format!("{} kind mismatch", path.display()));
                }
                if card.entries.is_empty() {
                    issues.push(format!("{} has no entries", path.display()));
                }
                loaded_cards.push(json!({"id": card.id, "kind": card_ref.kind, "path": path.display().to_string(), "entries": card.entries.len()}));
            }
            Err(err) => issues.push(err.to_string()),
        }
    }
    Ok(
        json!({"valid": issues.is_empty(), "manifest": root.join(DEFAULT_DIR).join("manifest.yaml").display().to_string(), "cards": loaded_cards, "issues": issues}),
    )
}

fn explain(root: &Path, persona: Option<&str>) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let selected = select_cards(&manifest, persona, None);
    Ok(json!({
        "format": manifest.format,
        "name": manifest.name,
        "principle": "Load the manifest first, then load only the card paths returned for the persona/job.",
        "persona": persona,
        "cards_to_consider": selected,
        "do_not": [
            "Do not ingest every card unless route says the task needs it.",
            "Do not treat the decision pack as a sequencer, CRM, enrichment tool, or execution agent.",
            "Do not invent missing GTM facts; surface gaps in the brief."
        ]
    }))
}

fn route(root: &Path, persona: &str, job: &str) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let selected = select_cards(&manifest, Some(persona), Some(job));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    Ok(json!({
        "persona": persona,
        "job": job,
        "route": selected,
        "decision_trace": [
            "manifest loaded",
            "persona matched against card metadata",
            "job keywords matched against card descriptions and tags",
            "base policy cards included for guardrails"
        ],
        "load_order": load_order
    }))
}

fn emit_brief(
    root: &Path,
    persona: &str,
    motion: Option<&str>,
    job: Option<&str>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let job_text = job.unwrap_or("unspecified GTM decision task");
    let selected = select_cards(&manifest, Some(persona), Some(job_text));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    Ok(json!({
        "contract": "mdp.brief.v0",
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "inputs": {"persona": persona, "motion": motion, "job": job_text},
        "required_load_order": load_order,
        "decision_trace": [
            {"step": "load_manifest", "reason": "discover pack metadata and card index"},
            {"step": "route_cards", "reason": "preserve progressive disclosure"},
            {"step": "apply_avoid_rules", "reason": "prevent category drift and unsupported claims"},
            {"step": "draft_or_decide", "reason": "use only loaded card evidence and cite gaps"}
        ],
        "output_requirements": {"state_assumptions": true, "cite_loaded_cards": true, "surface_gaps": true, "avoid_execution_claims": true, "use_loaded_cta_policy": true}
    }))
}

fn prospect_brief(
    root: &Path,
    prospect_path: &Path,
    channel: &str,
    job: Option<&str>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let prospect = read_prospect(prospect_path)?;
    let persona = prospect
        .persona
        .as_deref()
        .unwrap_or_else(|| infer_persona(&prospect.title));
    let job_text = job.unwrap_or("write outbound message");
    let route = select_cards(&manifest, Some(persona), Some(job_text));
    let load_order: Vec<String> = route
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    Ok(json!({
        "contract": "mdp.message-brief.v0",
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "channel": channel,
        "prospect": prospect,
        "persona": persona,
        "job": job_text,
        "required_load_order": load_order,
        "route": route,
        "decision_trace": [
            {"step": "read_prospect", "reason": "use enriched Clay/Deepline-style row as task input"},
            {"step": "infer_or_use_persona", "reason": "map person title to pack persona"},
            {"step": "route_cards", "reason": "load only relevant message decision cards"},
            {"step": "generate_or_handoff", "reason": "use the brief as the agent/model input contract"}
        ],
        "agent_instruction": "Read only required_load_order card files, combine them with prospect, then draft copy. Use the routed CTA policy when present. Do not invent claims outside the loaded cards."
    }))
}

fn demo_copy(root: &Path, prospect_path: &Path, channel: &str) -> Result<Value> {
    let brief = prospect_brief(
        root,
        prospect_path,
        channel,
        Some("write linkedin outbound copy"),
    )?;
    let prospect: Prospect = serde_json::from_value(brief["prospect"].clone())?;
    let persona = brief["persona"].as_str().unwrap_or("finance leader");
    let pack_name = brief["pack"]["name"].as_str().unwrap_or("Message Pack");
    let pack_id = brief["pack"]["id"].as_str().unwrap_or("");
    let is_rillet_pack =
        pack_name.to_lowercase().contains("rillet") || pack_id.to_lowercase().contains("rillet");
    let trigger = prospect
        .trigger
        .as_deref()
        .unwrap_or("scaling finance operations");
    let background = prospect
        .background
        .as_deref()
        .unwrap_or("working on finance systems");
    let first_name = prospect
        .name
        .split_whitespace()
        .next()
        .unwrap_or(&prospect.name);

    let (recommended, shorter, proof_led) = if is_rillet_pack {
        (
            format!(
                "Hey {first_name} - saw you're {background}. Rillet's AI-native ERP helps finance teams move toward continuous close with traceable entries and real-time reporting. Worth comparing notes?"
            ),
            format!(
                "Hey {first_name} - noticed {company} is {trigger}. Rillet helps modern finance teams close faster without spreadsheet-heavy workarounds. Open to a quick compare?",
                company = prospect.company
            ),
            format!(
                "Hey {first_name} - Rillet works with scaling teams that need cleaner close, revenue recognition, and reporting. Given your {title} role, thought this might be relevant.",
                title = prospect.title
            ),
        )
    } else {
        (
            format!(
                "Hey {first_name} - saw you're {background}. If {company} is {trigger}, a Message Decision Pack can keep persona, pain, hooks, CTA rules, and avoid-rules consistent across agents. Worth comparing notes?",
                company = prospect.company
            ),
            format!(
                "Hey {first_name} - noticed {company} is {trigger}. MDP helps teams version their GTM message context so agents draft from the same approved source. Open to a quick compare?",
                company = prospect.company
            ),
            format!(
                "Hey {first_name} - given your {title} role, thought a lightweight message decision layer could be relevant for keeping agent-generated GTM copy consistent.",
                title = prospect.title
            ),
        )
    };

    Ok(json!({
        "contract": "mdp.copy-demo.v0",
        "channel": channel,
        "persona": persona,
        "prospect": {
            "name": prospect.name,
            "title": prospect.title,
            "company": prospect.company,
            "linkedin_url": prospect.linkedin_url
        },
        "recommended": recommended,
        "alternates": [shorter, proof_led],
        "decision_trace": brief["decision_trace"].clone(),
        "cards_used": brief["required_load_order"].clone(),
        "notes": [
            "Deterministic demo copy only; production should pass the brief to a model.",
            "No LinkedIn, Clay, CRM, or sequencer write was performed."
        ]
    }))
}

fn pack(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut packed_cards = Vec::new();
    for card_ref in &manifest.cards {
        let path = root.join(DEFAULT_DIR).join(&card_ref.path);
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let hash = Sha256::digest(&bytes);
        packed_cards.push(json!({"id": card_ref.id, "kind": card_ref.kind, "path": card_ref.path, "sha256": format!("{hash:x}"), "bytes": bytes.len()}));
    }
    Ok(json!({"format": "mdp.pack.v0", "manifest": manifest, "cards": packed_cards}))
}

fn schema(target: SchemaTarget) -> Value {
    match target {
        SchemaTarget::Manifest => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Manifest v0", "type": "object", "required": ["format", "id", "name", "version", "personas", "cards", "policy", "provenance"], "properties": {"format": {"const": FORMAT_VERSION}, "id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "cards": {"type": "array"}}})
        }
        SchemaTarget::Card => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Card v0", "type": "object", "required": ["id", "kind", "title", "description", "entries"], "properties": {"id": {"type": "string"}, "kind": {"enum": ["personas", "pains", "motions", "hooks", "avoid-rules", "copy-patterns", "ctas"]}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}, "entries": {"type": "array"}}})
        }
        SchemaTarget::Brief => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Brief v0", "type": "object", "required": ["contract", "pack", "inputs", "required_load_order", "decision_trace", "output_requirements"], "properties": {"contract": {"const": "mdp.brief.v0"}, "required_load_order": {"type": "array", "items": {"type": "string"}}, "decision_trace": {"type": "array"}}})
        }
        SchemaTarget::Prospect => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Prospect Input v0", "type": "object", "required": ["name", "title", "company"], "properties": {"name": {"type": "string"}, "title": {"type": "string"}, "company": {"type": "string"}, "linkedin_url": {"type": "string"}, "company_url": {"type": "string"}, "background": {"type": "string"}, "trigger": {"type": "string"}, "persona": {"type": "string"}}})
        }
    }
}

fn select_cards(manifest: &Manifest, persona: Option<&str>, job: Option<&str>) -> Vec<Value> {
    let persona_lower = persona.map(|p| p.to_lowercase());
    let job_lower = job.unwrap_or("").to_lowercase();
    let is_message_job = is_message_job(&job_lower);
    let mut selected = Vec::new();
    let mut candidates = Vec::new();

    for card in &manifest.cards {
        if matches!(card.kind, CardKind::Personas | CardKind::AvoidRules) {
            selected.push(json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": "base guardrail", "description": card.description}));
        }
    }

    for (index, card) in manifest.cards.iter().enumerate() {
        if matches!(card.kind, CardKind::Personas | CardKind::AvoidRules) {
            continue;
        }
        let persona_match = persona_lower
            .as_ref()
            .map(|p| {
                card.personas
                    .iter()
                    .any(|candidate| candidate.to_lowercase() == *p)
                    || card.description.to_lowercase().contains(p)
            })
            .unwrap_or(false);
        let job_match = !job_lower.is_empty()
            && (card.description.to_lowercase().contains(&job_lower)
                || card.tags.iter().any(|tag| {
                    job_lower.contains(&tag.to_lowercase())
                        || tag.to_lowercase().contains(&job_lower)
                }));
        if persona_match || job_match {
            let reason = match (persona_match, job_match) {
                (true, true) => "persona and job/tag match",
                (true, false) => "persona match",
                (false, true) => "job/tag match",
                (false, false) => "matched",
            };
            candidates.push((
                card_priority(&card.kind, is_message_job),
                index,
                json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": reason, "description": card.description}),
            ));
        }
    }

    candidates.sort_by_key(|(priority, index, _)| (*priority, *index));
    for (_, _, card) in candidates {
        if selected.len() >= manifest.policy.max_cards_per_route {
            break;
        }
        selected.push(card);
    }
    selected
}

fn is_message_job(job_lower: &str) -> bool {
    [
        "copy", "outbound", "linkedin", "email", "message", "brief", "cta", "ask", "reply",
    ]
    .iter()
    .any(|token| job_lower.contains(token))
}

fn card_priority(kind: &CardKind, is_message_job: bool) -> usize {
    if is_message_job {
        match kind {
            CardKind::Pains => 10,
            CardKind::Hooks => 20,
            CardKind::CopyPatterns => 30,
            CardKind::Ctas => 40,
            CardKind::Motions => 50,
            CardKind::Personas | CardKind::AvoidRules => 0,
        }
    } else {
        match kind {
            CardKind::Motions => 10,
            CardKind::Pains => 20,
            CardKind::Hooks => 30,
            CardKind::CopyPatterns => 40,
            CardKind::Ctas => 50,
            CardKind::Personas | CardKind::AvoidRules => 0,
        }
    }
}

fn read_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join(DEFAULT_DIR).join("manifest.yaml");
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn read_card(path: &Path) -> Result<Card> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn read_prospect(path: &Path) -> Result<Prospect> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

fn infer_persona(title: &str) -> &str {
    let lower = title.to_lowercase();
    if lower.contains("cfo")
        || lower.contains("controller")
        || lower.contains("finance")
        || lower.contains("accounting")
    {
        "VP Finance"
    } else if lower.contains("revops") || lower.contains("gtm") || lower.contains("growth") {
        "GTM Engineering"
    } else {
        "Operator"
    }
}

fn write_yaml<T: Serialize>(path: &Path, value: &T, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            path.display()
        ));
    }
    let raw = serde_yaml::to_string(value)?;
    fs::write(path, raw).with_context(|| format!("writing {}", path.display()))
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    let mut file =
        fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn starter_manifest(name: &str, slug: &str, template: &str) -> Manifest {
    let personas = if template == "rillet" {
        vec![
            "VP Finance".to_string(),
            "Controller".to_string(),
            "Accounting Leader".to_string(),
            "GTM Engineering".to_string(),
        ]
    } else {
        vec![
            "GTM Engineering".to_string(),
            "PMM".to_string(),
            "PM".to_string(),
        ]
    };
    Manifest {
        format: FORMAT_VERSION.to_string(),
        id: slug.to_string(),
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: Some("A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, and copy guidance.".to_string()),
        personas,
        cards: if template == "rillet" {
            vec![
                card_ref("personas", "cards/personas.yaml", CardKind::Personas, "Finance and accounting personas for Rillet messaging.", &["VP Finance", "Controller", "Accounting Leader", "GTM Engineering"], &["persona", "finance"]),
                card_ref("pains", "cards/pains.yaml", CardKind::Pains, "Finance pains, buying triggers, and evidence requirements.", &["VP Finance", "Controller", "Accounting Leader"], &["pain", "trigger", "close"]),
                card_ref("motions", "cards/motions.yaml", CardKind::Motions, "Approved message motions for Rillet outbound and follow-up.", &["VP Finance", "Controller", "GTM Engineering"], &["motion", "outbound", "workflow"]),
                card_ref("hooks", "cards/hooks.yaml", CardKind::Hooks, "Rillet hooks grounded in public positioning and product claims.", &["VP Finance", "Controller", "Accounting Leader"], &["hook", "copy", "message", "erp"]),
                card_ref("avoid-rules", "cards/avoid-rules.yaml", CardKind::AvoidRules, "Claims and categories the agent must avoid.", &["VP Finance", "Controller", "Accounting Leader", "GTM Engineering"], &["guardrail", "avoid"]),
                card_ref("copy-patterns", "cards/copy-patterns.yaml", CardKind::CopyPatterns, "LinkedIn and email copy structures for finance buyers.", &["VP Finance", "Controller", "Accounting Leader"], &["copy", "brief", "linkedin", "outbound", "message"]),
                card_ref("ctas", "cards/ctas.yaml", CardKind::Ctas, "CTA rules, reply paths, and ask boundaries for finance buyer messages.", &["VP Finance", "Controller", "Accounting Leader"], &["cta", "ask", "reply", "copy", "outbound", "message"]),
            ]
        } else {
            vec![
                card_ref("personas", "cards/personas.yaml", CardKind::Personas, "Who the decision pack serves and what each persona needs.", &["GTM Engineering", "PMM", "PM"], &["persona"]),
                card_ref("pains", "cards/pains.yaml", CardKind::Pains, "Buyer pains, triggers, and evidence requirements.", &["PMM", "PM"], &["pain", "trigger"]),
                card_ref("motions", "cards/motions.yaml", CardKind::Motions, "Approved GTM motions and motion boundaries.", &["GTM Engineering", "PMM"], &["motion", "workflow"]),
                card_ref("hooks", "cards/hooks.yaml", CardKind::Hooks, "Messaging hooks that can be reused after evidence checks.", &["PMM"], &["hook", "copy", "message"]),
                card_ref("avoid-rules", "cards/avoid-rules.yaml", CardKind::AvoidRules, "Claims and categories the agent must avoid.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"]),
                card_ref("copy-patterns", "cards/copy-patterns.yaml", CardKind::CopyPatterns, "Copy structures and brief patterns for GTM outputs.", &["PMM"], &["copy", "brief", "outbound", "message"]),
                card_ref("ctas", "cards/ctas.yaml", CardKind::Ctas, "CTA rules, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"]),
            ]
        },
        policy: Policy { progressive_disclosure: true, load_manifest_first: true, max_cards_per_route: if template == "rillet" { 7 } else { 6 }, json_contract: "mdp.cli.v0".to_string(), no_auth_required: true },
        provenance: Provenance { owner: "local".to_string(), created_by: "mdp init".to_string(), notes: vec!["This pack is guidance and evidence context, not an execution system.".to_string(), "Agents should load only routed cards unless the user asks for a full audit.".to_string()] },
    }
}

fn card_ref(
    id: &str,
    path: &str,
    kind: CardKind,
    description: &str,
    personas: &[&str],
    tags: &[&str],
) -> CardRef {
    CardRef {
        id: id.to_string(),
        path: path.to_string(),
        kind,
        description: description.to_string(),
        personas: personas.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

fn starter_cards(template: &str) -> Vec<(&'static str, Card)> {
    if template == "rillet" {
        return vec![
            ("personas.yaml", card("personas", CardKind::Personas, "Rillet personas", "Finance, accounting, and GTM engineering personas for a Rillet message decision pack.", &["VP Finance", "Controller", "Accounting Leader", "GTM Engineering"], &["persona", "finance"], vec![
                entry_with_evidence("vp-finance", "VP Finance", "Cares about close speed, board-ready reporting, real-time visibility, and replacing spreadsheet-heavy finance workflows as the company scales.", &["VP Finance"], &["https://www.rillet.com/"]),
                entry_with_evidence("controller", "Controller", "Cares about auditability, traceable entries, approvals, close management, and reducing manual reconciliation work.", &["Controller"], &["https://www.rillet.com/"]),
                entry_with_evidence("accounting-leader", "Accounting Leader", "Cares about revenue recognition, ASC 606 analysis, contracts, schedules, reconciliations, and keeping accounting logic consistent.", &["Accounting Leader"], &["https://www.rillet.com/"]),
                entry("gtm-engineering", "GTM Engineering", "Maintains this pack so agents can use the same approved ICP, claims, triggers, and copy constraints across Clay, Deepline, Codex, Claude Code, or OpenCode.", &["GTM Engineering"]),
            ])),
            ("pains.yaml", card("pains", CardKind::Pains, "Rillet pains and triggers", "Finance pains, buying triggers, and evidence expectations for Rillet outbound.", &["VP Finance", "Controller", "Accounting Leader"], &["pain", "trigger", "close", "revenue"], vec![
                entry_with_evidence("close-lag", "Month-end lag", "The buyer is stuck in a period-end close loop and wants books, metrics, and exceptions updated continuously instead of waiting on spreadsheet cleanup.", &["VP Finance", "Controller"], &["https://www.rillet.com/"]),
                entry_with_evidence("rev-rec-complexity", "Revenue recognition complexity", "The company has SLG, PLG, usage-based, or contract-driven revenue patterns where manual schedules and ASC 606 review slow the team down.", &["Controller", "Accounting Leader"], &["https://www.rillet.com/"]),
                entry_with_evidence("data-spread", "Finance data spread across tools", "The team is reconciling CRM, billing, payments, payroll, AP, and bank data across too many systems before the GL can be trusted.", &["VP Finance", "Controller"], &["https://www.rillet.com/"]),
                entry_with_evidence("outgrowing-smb-ledger", "Outgrowing QBO or Xero", "The company needs stronger multi-entity, investor metrics, GAAP reporting, revenue, and controls than a small-business ledger plus spreadsheets can provide.", &["VP Finance", "Accounting Leader"], &["https://www.rillet.com/"]),
            ])),
            ("motions.yaml", card("motions", CardKind::Motions, "Approved Rillet motions", "Message motions this pack can brief for an agent. This does not send messages or sequence people.", &["VP Finance", "Controller", "GTM Engineering"], &["motion", "outbound", "linkedin", "email", "workflow"], vec![
                entry("linkedin-opener", "LinkedIn opener", "Use a short trigger-led note: observed finance scaling signal, likely close or reporting pain, Rillet angle, low-pressure compare-notes ask.", &["VP Finance", "Controller"]),
                entry("finance-follow-up", "Finance follow-up", "Reference the original hypothesis, add one concrete Rillet capability, and ask whether the issue is owned by finance, accounting, or systems.", &["VP Finance", "Controller"]),
                entry("call-prep", "Call prep", "Create a pre-call brief with persona, likely pains, claims allowed, claims avoided, open questions, and the exact cards loaded.", &["GTM Engineering", "VP Finance"]),
                entry("clay-row-to-brief", "Clay row to brief", "Convert an enriched person/company row into a message brief before drafting. Keep LinkedIn URL and source fields as inputs, not as proof of claims.", &["GTM Engineering"]),
            ])),
            ("hooks.yaml", card("hooks", CardKind::Hooks, "Rillet hooks", "Public Rillet positioning and capability hooks that can be used when evidence supports the prospect fit.", &["VP Finance", "Controller", "Accounting Leader"], &["hook", "copy", "message", "erp", "close"], vec![
                entry_with_evidence("ai-native-erp", "AI-native ERP", "Frame Rillet as an AI-native ERP for scaling companies, not an AI SDR, copywriter, CRM, sequencer, or generic automation product.", &["VP Finance", "Controller"], &["https://www.rillet.com/"]),
                entry_with_evidence("continuous-close", "Continuous close", "Use continuous close and traceable entries when the prospect signal points to close, reporting, or controls pain.", &["VP Finance", "Controller"], &["https://www.rillet.com/"]),
                entry_with_evidence("real-time-reporting", "Real-time reporting", "Use real-time reporting, GAAP metrics, and operator/investor metrics when the prospect likely cares about leadership visibility.", &["VP Finance"], &["https://www.rillet.com/"]),
                entry_with_evidence("advanced-rev-rec", "Advanced revenue recognition", "Use advanced revenue recognition, contracts, usage-based models, and ASC 606 language when the prospect has complex revenue patterns.", &["Controller", "Accounting Leader"], &["https://www.rillet.com/"]),
            ])),
            ("avoid-rules.yaml", card("avoid-rules", CardKind::AvoidRules, "Rillet avoid rules", "Claims and categories agents must avoid when using this pack.", &["VP Finance", "Controller", "Accounting Leader", "GTM Engineering"], &["guardrail", "avoid"], vec![
                Entry { id: "do-not-replace-accountants".to_string(), title: "Do not say AI replaces accountants".to_string(), body: "Do not imply Rillet removes the need for finance or accounting teams. Keep the framing on up-leveling, automation with controls, and accountants spending less time pushing paper.".to_string(), applies_to: vec!["VP Finance".to_string(), "Controller".to_string(), "Accounting Leader".to_string()], evidence: vec!["https://www.rillet.com/".to_string()], avoid: vec!["replaces accountants".to_string(), "fully autonomous accounting".to_string()] },
                Entry { id: "no-unverified-guarantees".to_string(), title: "Do not guarantee outcomes".to_string(), body: "Do not guarantee zero-day close, exact implementation time, exact savings, or compliance outcomes for a specific prospect unless that proof is separately provided.".to_string(), applies_to: vec!["VP Finance".to_string(), "Controller".to_string()], evidence: vec![], avoid: vec!["guaranteed zero-day close".to_string(), "guaranteed ASC 606 compliance".to_string()] },
                Entry { id: "not-execution-system".to_string(), title: "Do not blur MDP into execution".to_string(), body: "The MDP pack routes message decisions and creates briefs. It does not enrich leads, send LinkedIn messages, update CRM records, run sequences, or perform finance operations.".to_string(), applies_to: vec!["GTM Engineering".to_string()], evidence: vec![], avoid: vec!["auto-send".to_string(), "sequencer".to_string(), "CRM replacement".to_string()] },
            ])),
            ("copy-patterns.yaml", card("copy-patterns", CardKind::CopyPatterns, "Rillet copy patterns", "Agent-readable structures for LinkedIn and email copy that stay grounded in the routed cards.", &["VP Finance", "Controller", "Accounting Leader"], &["copy", "brief", "linkedin", "email", "outbound", "message"], vec![
                entry("linkedin-short", "LinkedIn opener", "Keep it under roughly 300 characters when possible: first-name context, likely finance trigger, one Rillet angle, one low-pressure ask.", &["VP Finance", "Controller"]),
                entry("trigger-pain-angle-ask", "Trigger -> pain -> angle -> ask", "Structure the message as: observed trigger, likely pain, Rillet angle, soft ask. State the trigger as a hypothesis, not a fact, unless sourced.", &["VP Finance", "Controller", "Accounting Leader"]),
                entry("no-hype", "No hype", "Use practical finance language. Avoid breathless AI claims, vague transformation language, and unsupported customer-specific claims.", &["VP Finance", "Controller", "Accounting Leader"]),
            ])),
            ("ctas.yaml", card("ctas", CardKind::Ctas, "Rillet CTA rules", "Calls to action, reply paths, and ask boundaries for finance buyer messages.", &["VP Finance", "Controller", "Accounting Leader"], &["cta", "ask", "reply", "copy", "linkedin", "email", "outbound", "message"], vec![
                entry("compare-notes", "Compare notes", "Default to a low-pressure compare-notes ask when the source signal is directional and not yet confirmed.", &["VP Finance", "Controller"]),
                entry("route-owner", "Route to owner", "When ownership is unclear, ask whether finance, accounting, or systems owns the problem instead of forcing a meeting ask.", &["VP Finance", "Controller", "Accounting Leader"]),
                entry("avoid-hard-calendar-push", "Avoid hard calendar push", "Do not open with aggressive calendar language. Use meeting language only after the message establishes relevant context.", &["VP Finance", "Controller", "Accounting Leader"]),
            ])),
        ];
    }

    vec![
        ("personas.yaml", card("personas", CardKind::Personas, "Core personas", "The users who author, maintain, and consume the decision pack.", &["GTM Engineering", "PMM", "PM"], &["persona"], vec![
            entry("gtm-engineering", "GTM Engineering", "Needs precise contracts, data boundaries, approved workflows, and machine-readable routing.", &["GTM Engineering"]),
            entry("pmm", "PMM", "Needs pains, triggers, hooks, proof points, and copy constraints without losing source fidelity.", &["PMM"]),
            entry("pm", "PM", "Needs product boundaries, roadmap-relevant pain evidence, and clear decisions about what the product is not.", &["PM"]),
        ])),
        ("pains.yaml", card("pains", CardKind::Pains, "Pains and triggers", "Reusable buyer pains with evidence expectations.", &["PMM", "PM"], &["pain", "trigger"], vec![
            entry("agent-context-drift", "Agent context drift", "Agents working on GTM tasks lose product truth when source context, contracts, and approved claims are scattered.", &["PMM", "PM"]),
            entry("handoff-friction", "Handoff friction", "Teams need a way to give agents enough context to draft or decide without dumping a giant doc into every prompt.", &["GTM Engineering", "PMM"]),
        ])),
        ("motions.yaml", card("motions", CardKind::Motions, "Approved motions", "GTM workflows this pack can support as context.", &["GTM Engineering", "PMM"], &["motion", "workflow"], vec![
            entry("copy-brief", "Copy brief", "Route persona, pain, hook, avoid-rules, and copy-pattern cards to produce a grounded brief, not final unsupervised sending.", &["PMM"]),
            entry("agent-preflight", "Agent preflight", "Let an agent inspect the pack before doing GTM work and report missing evidence or unsupported claims.", &["GTM Engineering"]),
        ])),
        ("hooks.yaml", card("hooks", CardKind::Hooks, "Hooks", "Starter hook patterns that require local evidence before use.", &["PMM"], &["hook", "copy", "message"], vec![
            entry("manifest-not-monolith", "Manifest, not monolith", "Position the pack as a small manifest plus task-specific cards so agents load the minimum needed context.", &["PMM"]),
            entry("evidence-before-action", "Evidence before action", "Emphasize that GTM execution should start with source context, contracts, and approval boundaries.", &["PMM"]),
        ])),
        ("avoid-rules.yaml", card("avoid-rules", CardKind::AvoidRules, "Avoid rules", "Category and claim boundaries agents must keep intact.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"], vec![Entry { id: "not-execution".to_string(), title: "Do not claim execution".to_string(), body: "Do not describe the decision pack as an AI SDR, sequencer, CRM, enrichment provider, BI tool, or generic RevOps automation system.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], evidence: vec![], avoid: vec!["AI SDR".to_string(), "sequencer".to_string(), "CRM replacement".to_string(), "generic automation".to_string()] }])),
        ("copy-patterns.yaml", card("copy-patterns", CardKind::CopyPatterns, "Copy patterns", "Reusable structures for brief and copy outputs.", &["PMM"], &["copy", "brief", "outbound", "message"], vec![
            entry("brief-contract", "Brief contract", "Return audience, job, loaded cards, decision trace, approved claims, avoid rules, open questions, and draft direction.", &["PMM"]),
            entry("claim-gap", "Claim gap", "When evidence is missing, write the gap explicitly instead of smoothing over it with generic GTM language.", &["PMM", "PM"]),
        ])),
        ("ctas.yaml", card("ctas", CardKind::Ctas, "CTA rules", "Calls to action, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"], vec![
            entry("soft-ask", "Soft ask", "Default to a low-friction ask such as comparing notes, sanity-checking the hypothesis, or asking who owns the problem.", &["PMM", "GTM Engineering"]),
            entry("no-false-urgency", "No false urgency", "Do not manufacture urgency or imply the prospect has asked for help unless the source row says so.", &["PMM"]),
            entry("reply-path", "Reply path", "When the best next step is not a meeting, ask a routing question that helps identify the owner, priority, or current workflow.", &["PMM", "GTM Engineering"]),
        ])),
    ]
}

fn starter_prospect(template: &str) -> Value {
    if template == "rillet" {
        return json!({
            "name": "Jordan Lee",
            "title": "VP Finance",
            "company": "Northstar AI",
            "linkedin_url": "https://www.linkedin.com/in/example-rillet-demo",
            "company_url": "https://example.com",
            "background": "leading finance operations at a fast-growing AI company",
            "trigger": "scaling reporting, controls, and multi-entity finance operations",
            "persona": "VP Finance"
        });
    }

    json!({
        "name": "Alex Rivera",
        "title": "GTM Engineering Lead",
        "company": "ExampleCo",
        "linkedin_url": "https://www.linkedin.com/in/example-mdp-demo",
        "company_url": "https://example.com",
        "background": "building repeatable agent-assisted GTM workflows",
        "trigger": "standardizing outbound context across agents and systems",
        "persona": "GTM Engineering"
    })
}

fn card(
    id: &str,
    kind: CardKind,
    title: &str,
    description: &str,
    personas: &[&str],
    tags: &[&str],
    entries: Vec<Entry>,
) -> Card {
    Card {
        id: id.to_string(),
        kind,
        title: title.to_string(),
        description: description.to_string(),
        personas: personas.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        entries,
    }
}

fn entry(id: &str, title: &str, body: &str, applies_to: &[&str]) -> Entry {
    Entry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        applies_to: applies_to.iter().map(|s| s.to_string()).collect(),
        evidence: vec![],
        avoid: vec![],
    }
}

fn entry_with_evidence(
    id: &str,
    title: &str,
    body: &str,
    applies_to: &[&str],
    evidence: &[&str],
) -> Entry {
    Entry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        applies_to: applies_to.iter().map(|s| s.to_string()).collect(),
        evidence: evidence.iter().map(|s| s.to_string()).collect(),
        avoid: vec![],
    }
}
