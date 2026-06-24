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
        #[arg(long, help = "Include entry-level route matches and gaps")]
        entries: bool,
    },
    #[command(about = "Evaluate prospect/account fit against pack fit rules")]
    Fit {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        prospect: PathBuf,
    },
    #[command(about = "Check draft copy or text against approved claims and guardrails")]
    CheckClaims {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    #[command(about = "List durable gaps and open questions from a pack")]
    Gaps {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    #[command(about = "Run pack eval fixtures when .mdp/evals exists")]
    Eval {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
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
    Eval,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    id: String,
    name: String,
    version: String,
    description: Option<String>,
    personas: Vec<String>,
    #[serde(default)]
    target_personas: Vec<String>,
    #[serde(default)]
    operator_roles: Vec<String>,
    #[serde(default)]
    supported_channels: Vec<String>,
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
    FitRules,
    Claims,
    Signals,
    Positioning,
    ChannelPolicies,
    Objections,
    Gaps,
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
    #[serde(default)]
    segment: Option<String>,
    #[serde(default)]
    signals: Vec<Signal>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Signal {
    id: String,
    title: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    freshness: Option<String>,
    #[serde(default)]
    state_as: Option<String>,
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
        Commands::Route {
            dir,
            persona,
            job,
            entries,
        } => print_output(cli.json, "route", route(&dir, &persona, &job, entries)?),
        Commands::Fit { dir, prospect } => print_output(cli.json, "fit", fit(&dir, &prospect)?),
        Commands::CheckClaims { dir, text, file } => print_output(
            cli.json,
            "check-claims",
            check_claims(&dir, text.as_deref(), file.as_deref())?,
        ),
        Commands::Gaps { dir } => print_output(cli.json, "gaps", gaps(&dir)?),
        Commands::Eval { dir } => print_output(cli.json, "eval", eval_pack(&dir)?),
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

fn route(root: &Path, persona: &str, job: &str, include_entries: bool) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let selected = select_cards(&manifest, Some(persona), Some(job));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    let mut payload = json!({
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
    });
    if include_entries {
        payload["entry_route"] = json!(entry_route(root, &manifest, persona, job)?);
    }
    Ok(payload)
}

fn fit(root: &Path, prospect_path: &Path) -> Result<Value> {
    let prospect = read_prospect(prospect_path)?;
    let fit_card = read_card_by_id(root, "fit-rules")?;
    let mut matches = Vec::new();
    let mut disqualifiers = Vec::new();
    let haystack = prospect_haystack(&prospect);

    for entry in &fit_card.entries {
        let entry_text = format!("{} {}", entry.title, entry.body).to_lowercase();
        let applies = entry.applies_to.iter().any(|candidate| {
            haystack.contains(&candidate.to_lowercase())
                || prospect
                    .segment
                    .as_ref()
                    .map(|s| s.eq_ignore_ascii_case(candidate))
                    .unwrap_or(false)
        });
        let keyword_match = entry_text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|token| token.len() >= 5)
            .any(|token| haystack.contains(token));
        if entry.avoid.is_empty() && (applies || keyword_match) {
            matches.push(json!({"id": entry.id, "title": entry.title, "reason": if applies { "segment/persona match" } else { "keyword match" }}));
        }
        for avoid in &entry.avoid {
            if haystack.contains(&avoid.to_lowercase()) {
                disqualifiers
                    .push(json!({"entry_id": entry.id, "term": avoid, "title": entry.title}));
            }
        }
    }

    let status = if !disqualifiers.is_empty() {
        "disqualified"
    } else if !matches.is_empty() {
        "fit"
    } else {
        "insufficient-context"
    };
    Ok(json!({
        "contract": "mdp.fit.v0",
        "prospect": prospect,
        "status": status,
        "matches": matches,
        "disqualifiers": disqualifiers,
        "decision": match status {
            "fit" => "Proceed to route/brief with stated assumptions.",
            "disqualified" => "Do not draft outbound copy unless the user overrides the disqualifier.",
            _ => "Ask for more context before drafting.",
        }
    }))
}

fn check_claims(root: &Path, text: Option<&str>, file: Option<&Path>) -> Result<Value> {
    let raw = match (text, file) {
        (Some(value), None) => value.to_string(),
        (None, Some(path)) => {
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
        }
        (Some(_), Some(_)) => return Err(anyhow!("pass either --text or --file, not both")),
        (None, None) => return Err(anyhow!("pass --text or --file")),
    };
    let lower = raw.to_lowercase();
    let claims_card = read_card_by_id(root, "claims")?;
    let avoid_card = read_card_by_id(root, "avoid-rules")?;
    let mut matched_claims = Vec::new();
    let mut claim_gaps = Vec::new();
    let mut guardrail_hits = Vec::new();

    for entry in &claims_card.entries {
        let title = entry.title.to_lowercase();
        let title_match = title.len() > 4 && lower.contains(&title);
        let evidence_missing = entry.evidence.is_empty();
        if title_match {
            matched_claims.push(json!({"id": entry.id, "title": entry.title, "evidence": entry.evidence, "evidence_missing": evidence_missing}));
            if evidence_missing {
                claim_gaps.push(json!({"id": entry.id, "title": entry.title, "reason": "matched claim has no evidence"}));
            }
        }
    }
    for entry in &avoid_card.entries {
        for term in &entry.avoid {
            if lower.contains(&term.to_lowercase()) {
                guardrail_hits
                    .push(json!({"entry_id": entry.id, "term": term, "title": entry.title}));
            }
        }
    }
    Ok(json!({
        "contract": "mdp.claim-check.v0",
        "valid": guardrail_hits.is_empty() && claim_gaps.is_empty(),
        "matched_claims": matched_claims,
        "claim_gaps": claim_gaps,
        "guardrail_hits": guardrail_hits,
        "decision": if guardrail_hits.is_empty() && claim_gaps.is_empty() { "claim-safe" } else { "needs-revision" }
    }))
}

fn gaps(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut durable_gaps = Vec::new();
    let mut evidence_gaps = Vec::new();
    if let Ok(card) = read_card_by_id(root, "gaps") {
        for entry in card.entries {
            durable_gaps.push(json!({"id": entry.id, "title": entry.title, "body": entry.body, "applies_to": entry.applies_to}));
        }
    }
    for card_ref in &manifest.cards {
        let card = read_card(&root.join(DEFAULT_DIR).join(&card_ref.path))?;
        for entry in &card.entries {
            if entry.evidence.is_empty()
                && !matches!(
                    card.kind,
                    CardKind::AvoidRules | CardKind::Gaps | CardKind::Ctas
                )
            {
                evidence_gaps.push(json!({"card_id": card.id, "entry_id": entry.id, "title": entry.title, "reason": "missing evidence"}));
            }
        }
    }
    let durable_count = durable_gaps.len();
    let evidence_count = evidence_gaps.len();
    Ok(json!({
        "contract": "mdp.gaps.v0",
        "durable_gaps": durable_gaps,
        "evidence_gaps": evidence_gaps,
        "summary": {"durable": durable_count, "evidence": evidence_count}
    }))
}

fn eval_pack(root: &Path) -> Result<Value> {
    let eval_dir = root.join(DEFAULT_DIR).join("evals");
    if !eval_dir.exists() {
        return Ok(json!({
            "contract": "mdp.eval.v0",
            "valid": true,
            "fixtures": [],
            "issues": [],
            "note": "No .mdp/evals directory found. Add YAML fixtures to make route behavior testable."
        }));
    }
    let mut fixtures = Vec::new();
    let mut issues = Vec::new();
    for entry in
        fs::read_dir(&eval_dir).with_context(|| format!("reading {}", eval_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let fixture: Value =
            serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        let persona = fixture["persona"].as_str().unwrap_or("GTM Engineering");
        let job = fixture["job"].as_str().unwrap_or("linkedin outbound copy");
        let result = route(root, persona, job, false)?;
        let load_order = result["load_order"].as_array().cloned().unwrap_or_default();
        let expected = fixture["expect_load_order_contains"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let missing: Vec<Value> = expected
            .into_iter()
            .filter(|expected_path| !load_order.iter().any(|actual| actual == expected_path))
            .collect();
        if !missing.is_empty() {
            issues.push(json!({"fixture": path.display().to_string(), "missing": missing}));
        }
        fixtures.push(json!({"path": path.display().to_string(), "persona": persona, "job": job, "load_order": load_order}));
    }
    Ok(
        json!({"contract": "mdp.eval.v0", "valid": issues.is_empty(), "fixtures": fixtures, "issues": issues}),
    )
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

    let (recommended, shorter, proof_led) = (
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
    );

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
    let card_kinds = [
        "personas",
        "pains",
        "motions",
        "hooks",
        "avoid-rules",
        "copy-patterns",
        "ctas",
        "fit-rules",
        "claims",
        "signals",
        "positioning",
        "channel-policies",
        "objections",
        "gaps",
    ];
    match target {
        SchemaTarget::Manifest => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Manifest v0", "type": "object", "required": ["format", "id", "name", "version", "personas", "cards", "policy", "provenance"], "properties": {"format": {"const": FORMAT_VERSION}, "id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "target_personas": {"type": "array", "items": {"type": "string"}}, "operator_roles": {"type": "array", "items": {"type": "string"}}, "supported_channels": {"type": "array", "items": {"type": "string"}}, "cards": {"type": "array"}}})
        }
        SchemaTarget::Card => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Card v0", "type": "object", "required": ["id", "kind", "title", "description", "entries"], "properties": {"id": {"type": "string"}, "kind": {"enum": card_kinds}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}, "entries": {"type": "array", "items": {"type": "object", "required": ["id", "title", "body"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": {"type": "array", "items": {"type": "string"}}, "evidence": {"type": "array", "items": {"type": "string"}}, "avoid": {"type": "array", "items": {"type": "string"}}}}}}})
        }
        SchemaTarget::Brief => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Brief v0", "type": "object", "required": ["contract", "pack", "inputs", "required_load_order", "decision_trace", "output_requirements"], "properties": {"contract": {"const": "mdp.brief.v0"}, "required_load_order": {"type": "array", "items": {"type": "string"}}, "decision_trace": {"type": "array"}}})
        }
        SchemaTarget::Prospect => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Prospect Input v0", "type": "object", "required": ["name", "title", "company"], "properties": {"name": {"type": "string"}, "title": {"type": "string"}, "company": {"type": "string"}, "linkedin_url": {"type": "string"}, "company_url": {"type": "string"}, "background": {"type": "string"}, "trigger": {"type": "string"}, "persona": {"type": "string"}, "segment": {"type": "string"}, "signals": {"type": "array", "items": {"type": "object", "required": ["id", "title"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "source": {"type": "string"}, "confidence": {"type": "string"}, "freshness": {"type": "string"}, "state_as": {"type": "string"}}}}}})
        }
        SchemaTarget::Eval => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Eval Fixture v0", "type": "object", "required": ["id", "persona", "job", "expect_load_order_contains"], "properties": {"id": {"type": "string"}, "persona": {"type": "string"}, "job": {"type": "string"}, "expect_load_order_contains": {"type": "array", "items": {"type": "string"}}}})
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
            CardKind::Personas | CardKind::AvoidRules => 0,
            CardKind::FitRules => 5,
            CardKind::Positioning => 10,
            CardKind::Pains => 20,
            CardKind::Signals => 25,
            CardKind::Hooks => 30,
            CardKind::Claims => 35,
            CardKind::CopyPatterns => 40,
            CardKind::Ctas => 45,
            CardKind::ChannelPolicies => 50,
            CardKind::Objections => 60,
            CardKind::Motions => 70,
            CardKind::Gaps => 80,
        }
    } else {
        match kind {
            CardKind::Personas | CardKind::AvoidRules => 0,
            CardKind::FitRules => 5,
            CardKind::Positioning => 10,
            CardKind::Motions => 20,
            CardKind::Signals => 30,
            CardKind::Pains => 40,
            CardKind::Claims => 50,
            CardKind::ChannelPolicies => 60,
            CardKind::Objections => 70,
            CardKind::Hooks => 80,
            CardKind::CopyPatterns => 90,
            CardKind::Ctas => 100,
            CardKind::Gaps => 110,
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

fn read_card_by_id(root: &Path, id: &str) -> Result<Card> {
    let manifest = read_manifest(root)?;
    let card_ref = manifest
        .cards
        .iter()
        .find(|card_ref| card_ref.id == id)
        .ok_or_else(|| anyhow!("missing card id {id}"))?;
    read_card(&root.join(DEFAULT_DIR).join(&card_ref.path))
}

fn prospect_haystack(prospect: &Prospect) -> String {
    let mut parts = vec![
        prospect.name.clone(),
        prospect.title.clone(),
        prospect.company.clone(),
    ];
    for value in [
        &prospect.linkedin_url,
        &prospect.company_url,
        &prospect.background,
        &prospect.trigger,
        &prospect.persona,
        &prospect.segment,
    ] {
        if let Some(value) = value {
            parts.push(value.clone());
        }
    }
    for signal in &prospect.signals {
        parts.push(signal.id.clone());
        parts.push(signal.title.clone());
        for value in [
            &signal.source,
            &signal.confidence,
            &signal.freshness,
            &signal.state_as,
        ] {
            if let Some(value) = value {
                parts.push(value.clone());
            }
        }
    }
    parts.join(" ").to_lowercase()
}

fn entry_route(root: &Path, manifest: &Manifest, persona: &str, job: &str) -> Result<Value> {
    let selected = select_cards(manifest, Some(persona), Some(job));
    let selected_ids: BTreeSet<String> = selected
        .iter()
        .filter_map(|value| value["id"].as_str().map(str::to_string))
        .collect();
    let persona_lower = persona.to_lowercase();
    let job_lower = job.to_lowercase();
    let job_tokens: Vec<String> = job_lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 4)
        .map(str::to_string)
        .collect();
    let mut matches = Vec::new();
    let mut gaps = Vec::new();

    for card_ref in &manifest.cards {
        if !selected_ids.contains(&card_ref.id) {
            continue;
        }
        let card = read_card(&root.join(DEFAULT_DIR).join(&card_ref.path))?;
        let mut card_match_count = 0usize;
        for entry in &card.entries {
            let entry_text = format!(
                "{} {} {}",
                entry.title,
                entry.body,
                entry.applies_to.join(" ")
            )
            .to_lowercase();
            let applies = entry.applies_to.is_empty()
                || entry
                    .applies_to
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(persona));
            let job_match = card.tags.iter().any(|tag| {
                let tag_lower = tag.to_lowercase();
                job_lower.contains(&tag_lower) || tag_lower.contains(&job_lower)
            }) || job_tokens.iter().any(|token| entry_text.contains(token));
            let persona_match = entry_text.contains(&persona_lower);
            if applies || job_match || persona_match {
                card_match_count += 1;
                matches.push(json!({
                    "card_id": card.id,
                    "card_kind": card.kind,
                    "entry_id": entry.id,
                    "title": entry.title,
                    "status": if matches!(card.kind, CardKind::AvoidRules | CardKind::FitRules | CardKind::Claims | CardKind::Positioning | CardKind::ChannelPolicies) { "required" } else { "supporting" },
                    "reason": if applies { "persona applies" } else if job_match { "job/tag match" } else { "persona text match" },
                    "evidence_count": entry.evidence.len(),
                    "avoid_count": entry.avoid.len()
                }));
            }
        }
        if card_match_count == 0 {
            gaps.push(json!({
                "card_id": card.id,
                "path": format!("{DEFAULT_DIR}/{}", card_ref.path),
                "reason": "card routed, but no entry matched persona/job cleanly"
            }));
        }
    }

    Ok(json!({
        "contract": "mdp.entry-route.v0",
        "persona": persona,
        "job": job,
        "matches": matches,
        "gaps": gaps,
        "policy": "Load matched entries first. Load the full card only when an entry is ambiguous, missing, or a guardrail card needs complete review."
    }))
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

fn starter_manifest(name: &str, slug: &str, _template: &str) -> Manifest {
    let personas = vec![
        "GTM Engineering".to_string(),
        "PMM".to_string(),
        "PM".to_string(),
    ];
    Manifest {
        format: FORMAT_VERSION.to_string(),
        id: slug.to_string(),
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: Some("A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, and copy guidance.".to_string()),
        personas: personas.clone(),
        target_personas: personas,
        operator_roles: vec!["GTM Engineering".to_string(), "PMM".to_string()],
        supported_channels: vec!["linkedin".to_string(), "email".to_string(), "call-prep".to_string(), "agent-brief".to_string()],
        cards: vec![
            card_ref("personas", "cards/personas.yaml", CardKind::Personas, "Who the decision pack serves and what each persona needs.", &["GTM Engineering", "PMM", "PM"], &["persona"]),
            card_ref("positioning", "cards/positioning.yaml", CardKind::Positioning, "Category, product boundaries, value pillars, and what this pack is not.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"]),
            card_ref("fit-rules", "cards/fit-rules.yaml", CardKind::FitRules, "ICP, fit, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"]),
            card_ref("signals", "cards/signals.yaml", CardKind::Signals, "Structured buying signals, triggers, and source interpretation rules.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "clay", "deepline", "linkedin"]),
            card_ref("pains", "cards/pains.yaml", CardKind::Pains, "Buyer pains, triggers, and evidence requirements.", &["PMM", "PM"], &["pain", "trigger"]),
            card_ref("claims", "cards/claims.yaml", CardKind::Claims, "Approved claims and proof requirements an agent may use.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"]),
            card_ref("motions", "cards/motions.yaml", CardKind::Motions, "Approved GTM motions and motion boundaries.", &["GTM Engineering", "PMM"], &["motion", "workflow"]),
            card_ref("channel-policies", "cards/channel-policies.yaml", CardKind::ChannelPolicies, "Channel-specific policy for LinkedIn, email, call prep, and agent briefs.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "brief"]),
            card_ref("hooks", "cards/hooks.yaml", CardKind::Hooks, "Messaging hooks that can be reused after evidence checks.", &["PMM"], &["hook", "copy", "message"]),
            card_ref("ctas", "cards/ctas.yaml", CardKind::Ctas, "CTA rules, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"]),
            card_ref("avoid-rules", "cards/avoid-rules.yaml", CardKind::AvoidRules, "Claims and categories the agent must avoid.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"]),
            card_ref("copy-patterns", "cards/copy-patterns.yaml", CardKind::CopyPatterns, "Copy structures and brief patterns for GTM outputs.", &["PMM"], &["copy", "brief", "outbound", "message"]),
            card_ref("objections", "cards/objections.yaml", CardKind::Objections, "Expected objections, category confusion, and approved response logic.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"]),
            card_ref("gaps", "cards/gaps.yaml", CardKind::Gaps, "Known gaps and open questions agents must surface instead of filling in.", &["GTM Engineering", "PMM", "PM"], &["gap", "unknown", "open-question"]),
        ],
        policy: Policy { progressive_disclosure: true, load_manifest_first: true, max_cards_per_route: 12, json_contract: "mdp.cli.v0".to_string(), no_auth_required: true },
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

fn starter_cards(_template: &str) -> Vec<(&'static str, Card)> {
    vec![
        ("personas.yaml", card("personas", CardKind::Personas, "Core personas", "The users who author, maintain, and consume the decision pack.", &["GTM Engineering", "PMM", "PM"], &["persona"], vec![
            entry("gtm-engineering", "GTM Engineering", "Needs precise contracts, data boundaries, approved workflows, and machine-readable routing.", &["GTM Engineering"]),
            entry("pmm", "PMM", "Needs pains, triggers, hooks, proof points, CTA policy, and copy constraints without losing source fidelity.", &["PMM"]),
            entry("pm", "PM", "Needs product boundaries, roadmap-relevant pain evidence, and clear decisions about what the product is not.", &["PM"]),
        ])),
        ("positioning.yaml", card("positioning", CardKind::Positioning, "Positioning and boundaries", "Category and product truth that every routed brief should preserve.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"], vec![
            entry_with_evidence("decision-layer", "Decision/context layer", "Describe MDP as a local, agent-readable decision and context layer for GTM messaging. It stores what an agent should load, believe, avoid, and surface as a gap.", &["GTM Engineering", "PMM", "PM"], &["README.md"]),
            entry_with_evidence("not-execution-system", "Not execution", "Do not describe MDP as a sender, CRM, sequencer, enrichment provider, scraper, AI SDR, BI tool, or generic automation system.", &["GTM Engineering", "PMM", "PM"], &["README.md"]),
            entry_with_evidence("progressive-disclosure", "Progressive disclosure", "The pack is a small manifest plus modular cards. Agents should load only the cards returned by route or brief commands.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("fit-rules.yaml", card("fit-rules", CardKind::FitRules, "Fit rules", "ICP, qualification, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"], vec![
            entry("good-fit-agent-gtm", "Good fit: agent-assisted GTM", "Use when the account is building GTM workflows with agents, Clay/Deepline-style enrichment rows, Codex/Claude Code/OpenCode, or multiple systems that need consistent message context.", &["GTM Engineering", "PMM"]),
            Entry { id: "no-context-no-copy".to_string(), title: "No message without context".to_string(), body: "If the row has no persona, trigger, source, or useful account context, return insufficient-context instead of drafting polished copy.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], evidence: vec![], avoid: vec!["no source".to_string(), "unknown persona".to_string(), "no trigger".to_string()] },
            Entry { id: "bad-fit-sending-only".to_string(), title: "Bad fit: sending-only ask".to_string(), body: "If the request is only to blast, sequence, or auto-send messages without decision context, treat it as out of scope for MDP.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], evidence: vec![], avoid: vec!["blast".to_string(), "auto-send".to_string(), "sequence everyone".to_string()] },
        ])),
        ("signals.yaml", card("signals", CardKind::Signals, "Signals and triggers", "How to interpret enriched rows, LinkedIn context, source material, and account signals.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "clay", "deepline", "linkedin"], vec![
            entry("enriched-row-signal", "Enriched row signal", "Treat Clay, Deepline, CSV, or user-provided enrichment as input evidence. Preserve source and confidence when present, and state weak signals as hypotheses.", &["GTM Engineering", "PMM"]),
            entry("linkedin-profile-signal", "LinkedIn profile signal", "Use LinkedIn URLs or profile summaries as context for role, background, and likely priorities. Do not pretend the profile proves a product need by itself.", &["PMM"]),
            entry("company-context-signal", "Company context signal", "Company website, hiring, funding, product, and stack clues can shape the pain hypothesis when the pack states how to interpret them.", &["PMM", "PM"]),
        ])),
        ("pains.yaml", card("pains", CardKind::Pains, "Pains and triggers", "Reusable buyer pains with evidence expectations.", &["PMM", "PM"], &["pain", "trigger"], vec![
            entry("agent-context-drift", "Agent context drift", "Agents working on GTM tasks lose product truth when source context, contracts, and approved claims are scattered.", &["PMM", "PM"]),
            entry("handoff-friction", "Handoff friction", "Teams need a way to give agents enough context to draft or decide without dumping a giant doc into every prompt.", &["GTM Engineering", "PMM"]),
            entry("claim-inconsistency", "Claim inconsistency", "Different agents or workflows reuse outdated claims, unsupported proof points, or mismatched CTAs when there is no shared pack.", &["PMM"]),
        ])),
        ("claims.yaml", card("claims", CardKind::Claims, "Approved claims", "Claims an agent may use only when the route and source context support them.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"], vec![
            entry_with_evidence("modular-pack-routing", "Modular pack routing", "MDP lets teams store messaging decisions in a manifest plus modular cards so agents load relevant context instead of a giant prompt.", &["PMM", "GTM Engineering"], &["README.md"]),
            entry_with_evidence("local-offline", "Local offline CLI", "The MVP CLI runs locally/offline without auth and returns stable JSON for agent and script usage.", &["GTM Engineering"], &["README.md", "cli/src/main.rs"]),
            entry_with_evidence("versionable-context", "Versionable message context", "A pack can live in a repo so teams can review, diff, test, and update messaging decisions over time.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("motions.yaml", card("motions", CardKind::Motions, "Approved motions", "GTM workflows this pack can support as context.", &["GTM Engineering", "PMM"], &["motion", "workflow"], vec![
            entry("copy-brief", "Copy brief", "Route persona, pain, hook, avoid-rules, CTA policy, and copy-pattern cards to produce a grounded brief, not final unsupervised sending.", &["PMM"]),
            entry("agent-preflight", "Agent preflight", "Let an agent inspect the pack before doing GTM work and report missing evidence or unsupported claims.", &["GTM Engineering"]),
            entry("clay-row-to-brief", "Enriched row to brief", "Convert a Clay/Deepline-style row into a message brief before drafting. Keep source fields as inputs, not as proof of claims.", &["GTM Engineering", "PMM"]),
        ])),
        ("channel-policies.yaml", card("channel-policies", CardKind::ChannelPolicies, "Channel policies", "Channel-specific rules for how to use the routed message decisions.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "brief"], vec![
            entry("linkedin-opener", "LinkedIn opener", "Keep the opener short, use one sourced or explicitly hypothetical trigger, one relevant angle, and one low-friction ask.", &["PMM"]),
            entry("email-follow-up", "Email follow-up", "Use a clearer subject, one source-backed reason for relevance, one approved claim, and a reply path that does not force a meeting too early.", &["PMM"]),
            entry("call-prep", "Call prep", "Return likely persona, pains, allowed claims, avoid-rules, open questions, and the exact cards loaded. Do not pretend this is CRM history.", &["GTM Engineering", "PMM"]),
        ])),
        ("hooks.yaml", card("hooks", CardKind::Hooks, "Hooks", "Starter hook patterns that require local evidence before use.", &["PMM"], &["hook", "copy", "message"], vec![
            entry("manifest-not-monolith", "Manifest, not monolith", "Position the pack as a small manifest plus task-specific cards so agents load the minimum needed context.", &["PMM"]),
            entry("evidence-before-action", "Evidence before action", "Emphasize that GTM execution should start with source context, contracts, and approval boundaries.", &["PMM"]),
            entry("one-context-many-agents", "One context, many agents", "Use when the account has Claude Code, Codex, OpenCode, Clay, or other systems that need the same source of messaging truth.", &["PMM", "GTM Engineering"]),
        ])),
        ("ctas.yaml", card("ctas", CardKind::Ctas, "CTA rules", "Calls to action, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"], vec![
            entry("soft-ask", "Soft ask", "Default to a low-friction ask such as comparing notes, sanity-checking the hypothesis, or asking who owns the problem.", &["PMM", "GTM Engineering"]),
            entry("no-false-urgency", "No false urgency", "Do not manufacture urgency or imply the prospect has asked for help unless the source row says so.", &["PMM"]),
            entry("reply-path", "Reply path", "When the best next step is not a meeting, ask a routing question that helps identify the owner, priority, or current workflow.", &["PMM", "GTM Engineering"]),
        ])),
        ("avoid-rules.yaml", card("avoid-rules", CardKind::AvoidRules, "Avoid rules", "Category and claim boundaries agents must keep intact.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"], vec![
            Entry { id: "not-execution".to_string(), title: "Do not claim execution".to_string(), body: "Do not describe the decision pack as an AI SDR, sequencer, CRM, enrichment provider, scraper, BI tool, or generic RevOps automation system.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], evidence: vec!["README.md".to_string()], avoid: vec!["AI SDR".to_string(), "sequencer".to_string(), "CRM replacement".to_string(), "generic automation".to_string(), "scraper".to_string()] },
            Entry { id: "no-unsourced-claims".to_string(), title: "No unsourced claims".to_string(), body: "Do not add quantified outcomes, integrations, customer names, compliance claims, or product capabilities unless they are present in the claims card or supplied source material.".to_string(), applies_to: vec!["PMM".to_string(), "GTM Engineering".to_string()], evidence: vec![], avoid: vec!["guaranteed".to_string(), "proven ROI".to_string(), "fully automated".to_string()] },
        ])),
        ("copy-patterns.yaml", card("copy-patterns", CardKind::CopyPatterns, "Copy patterns", "Reusable structures for brief and copy outputs.", &["PMM"], &["copy", "brief", "outbound", "message"], vec![
            entry("brief-contract", "Brief contract", "Return audience, job, loaded cards, decision trace, approved claims, avoid rules, open questions, and draft direction.", &["PMM"]),
            entry("claim-gap", "Claim gap", "When evidence is missing, write the gap explicitly instead of smoothing over it with generic GTM language.", &["PMM", "PM"]),
            entry("trigger-pain-angle-ask", "Trigger -> pain -> angle -> ask", "Structure outbound copy as observed trigger, likely pain, approved angle, and low-friction ask. Mark weak inputs as hypotheses.", &["PMM"]),
        ])),
        ("objections.yaml", card("objections", CardKind::Objections, "Objections and alternatives", "Category confusion and response logic for agents to preserve.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"], vec![
            entry("why-not-prompt", "Why not one giant prompt?", "Explain that MDP favors versioned, testable, progressively loaded cards so agents can fetch only the context needed for the current job.", &["PMM", "GTM Engineering"]),
            entry("why-not-sequencer", "Why not a sequencer?", "Clarify that MDP stores message decisions and evidence. Sequencers or CRMs may consume outputs, but they are separate execution systems.", &["PMM", "GTM Engineering"]),
        ])),
        ("gaps.yaml", card("gaps", CardKind::Gaps, "Known gaps", "Durable gaps and open questions agents should surface instead of inventing answers.", &["GTM Engineering", "PMM", "PM"], &["gap", "unknown", "open-question"], vec![
            entry("missing-company-proof", "Missing company-specific proof", "If a prospect/account row lacks concrete source context, ask for source material or state the personalization gap before drafting.", &["PMM", "GTM Engineering"]),
            entry("unclear-fit", "Unclear fit", "If role, segment, or trigger does not map to a fit rule, return insufficient-context instead of forcing a message.", &["GTM Engineering", "PMM"]),
            entry("hosted-api-not-included", "Hosted API not included", "The MVP is local/offline. Do not imply a hosted API exists unless the user has deployed one separately.", &["GTM Engineering", "PMM"]),
        ])),
    ]
}

fn starter_eval() -> Value {
    json!({
        "id": "linkedin-copy-route",
        "persona": "PMM",
        "job": "linkedin outbound copy",
        "expect_load_order_contains": [
            ".mdp/cards/personas.yaml",
            ".mdp/cards/avoid-rules.yaml",
            ".mdp/cards/positioning.yaml",
            ".mdp/cards/claims.yaml",
            ".mdp/cards/ctas.yaml"
        ]
    })
}

fn starter_prospect(_template: &str) -> Value {
    json!({
        "name": "Alex Rivera",
        "title": "GTM Engineering Lead",
        "company": "ExampleCo",
        "linkedin_url": "https://www.linkedin.com/in/example-mdp-demo",
        "company_url": "https://example.com",
        "background": "building repeatable agent-assisted GTM workflows across Clay, Codex, and Claude Code",
        "trigger": "standardizing outbound context across agents and systems",
        "persona": "GTM Engineering",
        "segment": "agent-assisted GTM",
        "signals": [
            {
                "id": "agent-gtm-workflow",
                "title": "Building multi-agent GTM workflow",
                "source": "example enrichment row",
                "confidence": "medium",
                "freshness": "recent",
                "state_as": "hypothesis"
            }
        ]
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
