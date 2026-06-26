use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mdp")]
#[command(about = "Author and route modular message decision packs for GTM agents")]
#[command(version)]
pub(crate) struct Cli {
    #[arg(long, global = true, help = "Emit stable machine-readable JSON")]
    pub(crate) json: bool,
    #[arg(
        long,
        global = true,
        help = "Emit a concise status summary instead of the full command payload"
    )]
    pub(crate) summary: bool,
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
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
        #[arg(
            long,
            help = "Inline full JSON Schemas in prompt output contracts instead of compact schema refs"
        )]
        include_output_schemas: bool,
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
        #[arg(long, help = "Include an eval fixture scaffold based on this route")]
        eval_fixture: bool,
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
        #[arg(
            long,
            help = "Optional subject line to check against routed subject constraints"
        )]
        subject: Option<String>,
        #[arg(long, help = "Optional persona for route-scoped constraint checks")]
        persona: Option<String>,
        #[arg(long, help = "Optional job for route-scoped constraint checks")]
        job: Option<String>,
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
        #[arg(long, help = "Include bounded entry-level context for drafting")]
        context: bool,
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
pub(crate) enum SchemaTarget {
    Manifest,
    Card,
    Prompt,
    Brief,
    Prospect,
    Eval,
}
