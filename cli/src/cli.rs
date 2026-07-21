use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mdp")]
#[command(about = "Author and route modular message decision packs for agent workflows")]
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
    #[command(about = "Print agent-readable CLI capabilities and contracts")]
    Capabilities,
    #[command(about = "Create a starter MDP package")]
    Init {
        #[arg(long, help = "Pack display name; defaults by template")]
        name: Option<String>,
        #[arg(
            long,
            help = "External company, product, or project this pack positions"
        )]
        target_name: Option<String>,
        #[arg(
            long,
            default_value = "company",
            help = "Target identity kind (company, product, or project)"
        )]
        target_kind: String,
        #[arg(
            long = "target-alias",
            help = "Repeatable external alias for the target"
        )]
        target_aliases: Vec<String>,
        #[arg(
            long = "exclude-term",
            help = "Repeatable prior-target or starter term that must not survive authoring"
        )]
        exclude_terms: Vec<String>,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(
            long,
            default_value = "gtm",
            help = "Starter template to write (available: gtm, proposal)"
        )]
        template: String,
        #[arg(long, help = "Overwrite existing starter files")]
        force: bool,
        #[arg(
            long,
            help = "Inline full JSON Schemas in prompt output contracts instead of compact schema refs"
        )]
        include_output_schemas: bool,
        #[arg(long, help = "Show files that would be written without writing them")]
        dry_run: bool,
    },
    #[command(about = "Report local setup and pack health")]
    Doctor {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    #[command(about = "Print canonical skill inventory and pack-aware eligibility")]
    Skills {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, requires = "dir")]
        job: Option<String>,
    },
    #[command(about = "Validate manifest and card references")]
    Validate {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Fail validation-style flows on warnings where supported")]
        strict: bool,
    },
    #[command(about = "Validate model-produced prompt output JSON against a prompt contract")]
    ValidatePromptOutput {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        file: PathBuf,
        #[arg(
            long,
            help = "Optional mdp.source-audit.v0 JSON file for deterministic source-ref/snippet checks"
        )]
        source_audit: Option<PathBuf>,
        #[arg(long, help = "Prompt file path to validate against")]
        prompt: Option<PathBuf>,
        #[arg(long, help = "Prompt id to validate against")]
        prompt_id: Option<String>,
        #[arg(long, help = "Fail validation-style flows on warnings where supported")]
        strict: bool,
    },
    #[command(about = "Create an audit-grade runner receipt from local workflow artifacts")]
    RunReceipt {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, value_enum, default_value_t = RunReceiptWorkflow::ProposalReview)]
        workflow: RunReceiptWorkflow,
        #[arg(long, value_enum, default_value_t = RunIsolation::Unknown)]
        isolation: RunIsolation,
        #[arg(
            long,
            help = "Confirm the model call received only the prompt-declared payload inputs"
        )]
        declared_inputs_only: bool,
        #[arg(long, help = "Prompt id used for the model artifact and validation")]
        prompt_id: Option<String>,
        #[arg(long, help = "Model-produced mdp.prompt-output.v0 JSON artifact")]
        prompt_output: Option<PathBuf>,
        #[arg(long, help = "mdp validate-prompt-output JSON result")]
        validation: Option<PathBuf>,
        #[arg(long, help = "mdp.source-audit.v0 JSON ledger used by validation")]
        source_audit: Option<PathBuf>,
        #[arg(
            long = "artifact",
            value_name = "KIND=PATH",
            help = "Additional local artifact to hash into the receipt"
        )]
        artifacts: Vec<String>,
        #[arg(long, help = "Write the receipt JSON artifact")]
        out: Option<PathBuf>,
        #[arg(long, help = "Show the receipt artifact write without writing it")]
        dry_run: bool,
    },
    #[command(about = "Verify proof-carrying generated output against loaded pack IDs")]
    VerifyOutput {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        file: PathBuf,
        #[arg(long, help = "Emit a human-readable Markdown proposal review artifact")]
        readable: bool,
    },
    #[command(about = "Compile a proof-output draft into verified mdp.proof-output.v0 JSON")]
    AuthorProofOutput {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "mdp.proof-output-draft.v0 JSON to compile")]
        draft: PathBuf,
        #[arg(long, help = "Write the verified proof-output JSON artifact")]
        out: Option<PathBuf>,
        #[arg(long, help = "Show the output artifact write without writing it")]
        dry_run: bool,
    },
    #[command(about = "Render a compact human brief from an existing MDP artifact")]
    RenderBrief {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Artifact JSON to render; omit to read JSON from stdin")]
        file: Option<PathBuf>,
        #[arg(long, help = "Named human brief template to apply")]
        template: String,
        #[arg(long, value_enum, default_value_t = HumanBriefFormat::Markdown)]
        format: HumanBriefFormat,
        #[arg(long, help = "Write rendered output to a file instead of stdout only")]
        out: Option<PathBuf>,
        #[arg(long, help = "Fail when required gate or proof fields are missing")]
        strict: bool,
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
        #[arg(
            long = "scope",
            value_name = "DIMENSION=VALUE",
            help = "Repeatable portfolio context selector"
        )]
        scope: Vec<String>,
        #[arg(long, help = "Include entry-level route matches and gaps")]
        entries: bool,
        #[arg(long, help = "Include an eval fixture scaffold based on this route")]
        eval_fixture: bool,
    },
    #[command(about = "Generate clearly fake prospect fixtures for outbound-copy testing")]
    SampleLeads {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        persona: String,
        #[arg(long, default_value = "initial email outbound copy testing")]
        job: String,
        #[arg(long, default_value_t = 3, help = "Fixture row count, from 2 to 5")]
        count: usize,
        #[arg(long, default_value_t = 0, help = "Deterministic fixture variant seed")]
        seed: u64,
        #[arg(long, value_enum, default_value_t = SampleLeadsFormat::Json)]
        format: SampleLeadsFormat,
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
        #[arg(
            long = "scope",
            value_name = "DIMENSION=VALUE",
            help = "Repeatable portfolio context selector"
        )]
        scope: Vec<String>,
        #[arg(long, help = "Treat advisory constraint warnings as failures")]
        strict: bool,
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
        #[arg(long, help = "Fail validation-style flows on warnings where supported")]
        strict: bool,
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
        #[arg(
            long,
            help = "Emit a human-readable Markdown prospect brief instead of the JSON contract"
        )]
        readable: bool,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, help = "Show the output artifact write without writing it")]
        dry_run: bool,
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
        #[arg(
            long = "scope",
            value_name = "DIMENSION=VALUE",
            help = "Repeatable portfolio context selector"
        )]
        scope: Vec<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, help = "Show the output artifact write without writing it")]
        dry_run: bool,
    },
    #[command(about = "Compile a bounded portable representation with card hashes")]
    Pack {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, help = "Show the output artifact write without writing it")]
        dry_run: bool,
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
    ProofOutput,
    ProofOutputDraft,
    RunReceipt,
    Brief,
    HumanBrief,
    RuntimeContext,
    Prospect,
    Eval,
    Skills,
}

#[derive(Clone, ValueEnum, PartialEq, Eq)]
pub(crate) enum SampleLeadsFormat {
    Json,
    Yaml,
}

#[derive(Clone, ValueEnum, PartialEq, Eq)]
pub(crate) enum HumanBriefFormat {
    Markdown,
    Json,
}

#[derive(Clone, ValueEnum, PartialEq, Eq)]
pub(crate) enum RunReceiptWorkflow {
    ProposalReview,
    GtmProspect,
    PackBuild,
    Custom,
}

impl RunReceiptWorkflow {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ProposalReview => "proposal-review",
            Self::GtmProspect => "gtm-prospect",
            Self::PackBuild => "pack-build",
            Self::Custom => "custom",
        }
    }

    pub(crate) fn requires_source_audit(&self) -> bool {
        matches!(self, Self::ProposalReview)
    }
}

#[derive(Clone, ValueEnum, PartialEq, Eq)]
pub(crate) enum RunIsolation {
    Isolated,
    Ambient,
    Unknown,
}

impl RunIsolation {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Isolated => "isolated",
            Self::Ambient => "ambient",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn conversation_context_used(&self) -> Option<bool> {
        match self {
            Self::Isolated => Some(false),
            Self::Ambient => Some(true),
            Self::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_accepts_inventory_pack_and_single_job_forms() {
        let inventory =
            Cli::try_parse_from(["mdp", "--json", "skills"]).expect("inventory form should parse");
        assert!(matches!(
            inventory.command,
            Commands::Skills {
                dir: None,
                job: None
            }
        ));

        let pack =
            Cli::try_parse_from(["mdp", "skills", "--dir", "."]).expect("pack form should parse");
        assert!(matches!(
            pack.command,
            Commands::Skills {
                dir: Some(_),
                job: None
            }
        ));

        let job = Cli::try_parse_from([
            "mdp",
            "skills",
            "--dir",
            ".",
            "--job",
            "prospect-fit-or-brief",
        ])
        .expect("single-job form should parse");
        assert!(matches!(
            job.command,
            Commands::Skills {
                dir: Some(_),
                job: Some(_)
            }
        ));
    }

    #[test]
    fn skills_requires_dir_for_job_and_removed_agent_surface_is_unknown() {
        assert!(Cli::try_parse_from(["mdp", "skills", "--job", "prospect-fit-or-brief"]).is_err());
        assert!(Cli::try_parse_from(["mdp", "agent-surface"]).is_err());
    }

    #[test]
    fn run_receipt_parses_audit_boundary_flags() {
        let parsed = Cli::try_parse_from([
            "mdp",
            "run-receipt",
            "--dir",
            ".",
            "--workflow",
            "proposal-review",
            "--isolation",
            "isolated",
            "--declared-inputs-only",
            "--prompt-id",
            "normalize-opportunity",
            "--prompt-output",
            "/tmp/prompt-output.json",
            "--validation",
            "/tmp/validation.json",
            "--source-audit",
            "/tmp/source-audit.json",
        ])
        .expect("run-receipt should parse");

        assert!(matches!(
            parsed.command,
            Commands::RunReceipt {
                workflow: RunReceiptWorkflow::ProposalReview,
                isolation: RunIsolation::Isolated,
                declared_inputs_only: true,
                ..
            }
        ));
    }
}
