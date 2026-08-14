use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
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
    #[command(about = "Compile and inspect cold-model conformance evidence")]
    Conformance {
        #[command(subcommand)]
        command: ConformanceCommand,
    },
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
    #[command(about = "Compile the decision inputs required for one pack job")]
    Requirements {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Closed profile job id to compile")]
        job: String,
    },
    #[command(about = "Validate one integration-owned source binding against an exact pack job")]
    ValidateSourceBinding {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, help = "Closed profile job id to validate")]
        job: String,
        #[arg(
            long,
            help = "Version-compatible mdp.source-binding.v1 or signal-aware v2 JSON file"
        )]
        file: PathBuf,
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
        #[arg(
            long,
            help = "Exact mdp.source-binding.v2 JSON artifact for signal-aware normalization lineage validation"
        )]
        source_binding: Option<PathBuf>,
        #[arg(
            long,
            help = "Exact version-compatible source-attempt request JSON file for decision-input normalization binding and freshness validation"
        )]
        source_attempt_request: Option<PathBuf>,
        #[arg(
            long,
            help = "Exact version-compatible collected attempt-results JSON file for immutable execution-fact binding and raw-value normalization"
        )]
        collected_attempt_results: Option<PathBuf>,
        #[arg(
            long,
            help = "Exact mdp.prompt-invocation.v1 JSON receipt binding a governed artifact to the host-supplied prompt and declared inputs"
        )]
        invocation_receipt: Option<PathBuf>,
        #[arg(
            long,
            help = "Exact canonical mdp.routed-context.v1 JSON input used by a governed generation or review prompt"
        )]
        routed_context: Option<PathBuf>,
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
            long,
            help = "Optional mdp.runner-audit.v0 JSON proving the headless/stateless runner boundary"
        )]
        runner_audit: Option<PathBuf>,
        #[arg(
            long,
            help = "Block unless --runner-audit proves a supported isolated runner mode"
        )]
        require_runner_audit: bool,
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
    #[command(about = "Verify one v1 clean-run bundle and receipt without invoking a runner")]
    VerifyRun {
        #[arg(long, help = "mdp.run-bundle.v1 JSON file; required for v1 receipts")]
        bundle: Option<PathBuf>,
        #[arg(long, help = "mdp.run-receipt.v1 JSON file")]
        receipt: PathBuf,
        #[arg(
            long,
            help = "Optional root containing receipt artifacts at their logical names"
        )]
        artifact_root: Option<PathBuf>,
    },
    #[command(
        about = "Project a saved decision result or verified run into a bounded trace",
        group(ArgGroup::new("trace_source").required(true).multiple(false).args(["file", "bundle"]))
    )]
    Trace {
        #[arg(long, conflicts_with_all = ["bundle", "receipt"], help = "Saved CLI JSON result or supported raw contracted artifact")]
        file: Option<PathBuf>,
        #[arg(long, requires = "receipt", help = "mdp.run-bundle.v1 JSON file")]
        bundle: Option<PathBuf>,
        #[arg(long, requires = "bundle", help = "mdp.run-receipt.v1 JSON file")]
        receipt: Option<PathBuf>,
        #[arg(
            long,
            help = "Root containing receipt artifacts or a composite conformance authority"
        )]
        artifact_root: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = TraceFormat::Json)]
        format: TraceFormat,
        #[arg(long, help = "Write the projection or Mermaid view to a file")]
        out: Option<PathBuf>,
    },
    #[command(about = "Atomically consume one verified receipt in the local conformance ledger")]
    ConsumeRun {
        #[arg(long, help = "Append-only local reference ledger path")]
        ledger: PathBuf,
        #[arg(long)]
        job_id: String,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        receipt_sha256: String,
        #[arg(long)]
        expected_prior_version: u64,
        #[arg(
            long,
            help = "Permit replay only when job, key, receipt, and prior version match"
        )]
        permit_exact_replay: bool,
    },
    #[command(about = "Execute one clean run from an exact v1 request file")]
    Run {
        #[arg(long, help = "mdp.run-request.v1 JSON file")]
        request: PathBuf,
        #[arg(long, help = "New directory for immutable published run artifacts")]
        out_dir: PathBuf,
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
        #[arg(long, required_unless_present = "normalized_input")]
        prospect: Option<PathBuf>,
        #[arg(long, conflicts_with = "prospect")]
        normalized_input: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        prompt: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        source_binding: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        source_attempt_request: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        collected_attempt_results: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        job: Option<String>,
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
        #[arg(long, required_unless_present = "normalized_input")]
        prospect: Option<PathBuf>,
        #[arg(long, conflicts_with = "prospect")]
        normalized_input: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        prompt: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        source_binding: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        source_attempt_request: Option<PathBuf>,
        #[arg(long, requires = "normalized_input")]
        collected_attempt_results: Option<PathBuf>,
        #[arg(long, default_value = "linkedin")]
        channel: String,
        #[arg(long)]
        job: Option<String>,
        #[arg(long, help = "Include bounded entry-level context for drafting")]
        context: bool,
        #[arg(
            long,
            value_name = "PATH",
            help = "Write exact canonical mdp.routed-context.v1 bytes for the selected job"
        )]
        routed_context_out: Option<PathBuf>,
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
        #[arg(
            long,
            value_name = "PATH",
            help = "Write exact canonical mdp.routed-context.v1 bytes for the selected job"
        )]
        routed_context_out: Option<PathBuf>,
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

#[derive(Subcommand)]
pub(crate) enum ConformanceCommand {
    #[command(about = "Compile deterministic D1-D12 sufficiency assertions for one candidate")]
    Compile {
        #[arg(long, help = "Closed mdp.conformance-candidate.v1 JSON file")]
        candidate: PathBuf,
        #[arg(long, help = "Staged root containing every candidate authority")]
        artifact_root: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Validate recorded behavioral trials without invoking a model")]
    Validate {
        #[arg(long, help = "Staged root containing every behavioral evidence file")]
        artifact_root: PathBuf,
        #[arg(long, help = "Candidate path relative to the staged artifact root")]
        candidate: PathBuf,
        #[arg(long, help = "Closed mdp.deterministic-conformance.v1 JSON file")]
        deterministic: PathBuf,
        #[arg(long)]
        evaluator_inventory: PathBuf,
        #[arg(long)]
        lifecycle_policy: PathBuf,
        #[arg(long, required = true)]
        invocation: Vec<PathBuf>,
        #[arg(long, required = true)]
        trial: Vec<PathBuf>,
        #[arg(long)]
        evaluator_result: Vec<PathBuf>,
        #[arg(long)]
        publication_approval: Vec<PathBuf>,
        #[arg(long, required = true)]
        verifier_receipt: Vec<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Assemble one hash-linked mdp.job-conformance.v1 authority")]
    Assemble {
        #[arg(long, help = "Candidate path relative to the staged artifact root")]
        candidate: PathBuf,
        #[arg(
            long,
            help = "Deterministic evaluation path relative to the staged root"
        )]
        deterministic: PathBuf,
        #[arg(long, help = "Behavioral evaluation path relative to the staged root")]
        behavioral: PathBuf,
        #[arg(
            long,
            help = "Trial path relative to the staged root; repeat in declared order"
        )]
        trial: Vec<PathBuf>,
        #[arg(long, help = "Staged root containing every composite member")]
        artifact_root: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Project a validated composite into a private or public report")]
    Report {
        #[arg(long, help = "Job conformance path relative to the staged root")]
        conformance: PathBuf,
        #[arg(long)]
        artifact_root: PathBuf,
        #[arg(long, value_enum)]
        visibility: ConformanceReportVisibility,
        #[arg(long, help = "Recorded RFC 3339 report projection time")]
        generated_at: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum ConformanceReportVisibility {
    Private,
    Public,
}

#[derive(Clone, ValueEnum)]
pub(crate) enum SchemaTarget {
    Manifest,
    Card,
    Prompt,
    ProofOutput,
    ProofOutputDraft,
    SourceIntake,
    SourceAudit,
    NativeNormalizeRequest,
    PromptOutput,
    ProposalRunManifest,
    ProposalRunnerResult,
    ProposalRunnerResultV1,
    ProposalReadinessReport,
    ProposalMcpRunResult,
    RunReceipt,
    RunnerAudit,
    RunRequestV1,
    RunBundleV1,
    DriverRequestV1,
    DriverResultV1,
    DriverRequestV2,
    DriverResultV2,
    RunnerAuditV1,
    RunReceiptV1,
    RunVerificationV1,
    RunExecutionV1,
    DecisionTraceV1,
    CanonicalAuthorityBlockV1,
    ConformanceCandidateV1,
    ModelInvocationEvidenceV1,
    EvaluatorInventoryV1,
    EvaluatorResultV1,
    PrivateRecordPolicyV1,
    PublicationApprovalV1,
    ConformanceTrialV1,
    JobConformanceV1,
    ConformanceReportV1,
    PublicConformanceReportV1,
    DeterministicConformanceV1,
    ConformanceVerifierReceiptV1,
    BehavioralEvaluationV1,
    Brief,
    HumanBrief,
    RuntimeContext,
    RoutedContextV1,
    DecisionInput,
    SourceBinding,
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
pub(crate) enum TraceFormat {
    Json,
    Mermaid,
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
    fn conformance_compile_requires_candidate_and_artifact_root() {
        let parsed = Cli::try_parse_from([
            "mdp",
            "--json",
            "conformance",
            "compile",
            "--candidate",
            "candidate.json",
            "--artifact-root",
            "staged",
        ])
        .expect("conformance compile should parse");

        assert!(matches!(
            parsed.command,
            Commands::Conformance {
                command: ConformanceCommand::Compile {
                    candidate,
                    artifact_root,
                    ..
                }
            } if candidate == PathBuf::from("candidate.json")
                && artifact_root == PathBuf::from("staged")
        ));
        assert!(Cli::try_parse_from(["mdp", "conformance", "compile"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdp",
                "conformance",
                "compile",
                "--candidate",
                "candidate.json"
            ])
            .is_err()
        );
    }

    #[test]
    fn conformance_validate_requires_recorded_evidence_and_never_requests_a_provider() {
        let parsed = Cli::try_parse_from([
            "mdp",
            "conformance",
            "validate",
            "--artifact-root",
            "staged",
            "--candidate",
            "candidate.json",
            "--deterministic",
            "deterministic.json",
            "--evaluator-inventory",
            "inventory.json",
            "--lifecycle-policy",
            "policy.json",
            "--invocation",
            "invocation-1.json",
            "--trial",
            "trial-1.json",
            "--verifier-receipt",
            "verifier-1.json",
        ])
        .expect("recorded behavioral evidence should parse");
        match parsed.command {
            Commands::Conformance {
                command:
                    ConformanceCommand::Validate {
                        invocation,
                        trial,
                        deterministic,
                        ..
                    },
            } => {
                assert_eq!(invocation.len(), 1);
                assert_eq!(trial.len(), 1);
                assert_eq!(deterministic, PathBuf::from("deterministic.json"));
            }
            _ => panic!("expected behavioral validation command"),
        }
        assert!(Cli::try_parse_from(["mdp", "conformance", "validate"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdp",
                "conformance",
                "validate",
                "--candidate",
                "candidate.json",
                "--deterministic",
                "deterministic.json",
                "--evaluator-inventory",
                "inventory.json",
                "--lifecycle-policy",
                "policy.json",
                "--invocation",
                "invocation.json",
                "--trial",
                "trial.json",
                "--verifier-receipt",
                "verifier.json"
            ])
            .is_err(),
            "behavioral validation requires an artifact root"
        );
        let assembly = Cli::try_parse_from([
            "mdp",
            "conformance",
            "assemble",
            "--candidate",
            "candidate.json",
            "--deterministic",
            "deterministic.json",
            "--behavioral",
            "behavioral.json",
            "--artifact-root",
            "staged",
        ])
        .expect("an unassessed assembly may contain zero trials");
        assert!(matches!(
            assembly.command,
            Commands::Conformance {
                command: ConformanceCommand::Assemble { trial, .. }
            } if trial.is_empty()
        ));
    }

    #[test]
    fn trace_requires_exactly_one_complete_source_form() {
        let file = Cli::try_parse_from(["mdp", "trace", "--file", "fit.json"])
            .expect("saved result should parse");
        assert!(matches!(
            file.command,
            Commands::Trace {
                file: Some(_),
                bundle: None,
                receipt: None,
                ..
            }
        ));

        let run = Cli::try_parse_from([
            "mdp",
            "trace",
            "--bundle",
            "bundle.json",
            "--receipt",
            "receipt.json",
            "--artifact-root",
            "run",
            "--format",
            "mermaid",
        ])
        .expect("run authority pair should parse");
        assert!(matches!(
            run.command,
            Commands::Trace {
                file: None,
                bundle: Some(_),
                receipt: Some(_),
                format: TraceFormat::Mermaid,
                ..
            }
        ));

        assert!(Cli::try_parse_from(["mdp", "trace"]).is_err());
        assert!(Cli::try_parse_from(["mdp", "trace", "--bundle", "bundle.json"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdp",
                "trace",
                "--file",
                "fit.json",
                "--bundle",
                "bundle.json",
                "--receipt",
                "receipt.json"
            ])
            .is_err()
        );
    }

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
    fn requirements_requires_a_job_and_accepts_a_pack_dir() {
        let parsed = Cli::try_parse_from([
            "mdp",
            "--json",
            "requirements",
            "--dir",
            "example-pack",
            "--job",
            "prospect-fit-or-brief",
        ])
        .expect("requirements form should parse");
        assert!(matches!(
            parsed.command,
            Commands::Requirements { dir, job }
                if dir == PathBuf::from("example-pack") && job == "prospect-fit-or-brief"
        ));
    }

    #[test]
    fn validate_source_binding_requires_job_pack_and_file() {
        let parsed = Cli::try_parse_from([
            "mdp",
            "--json",
            "validate-source-binding",
            "--dir",
            "example-pack",
            "--job",
            "prospect-fit-or-brief",
            "--file",
            "binding.json",
        ])
        .expect("source binding form should parse");
        assert!(matches!(
            parsed.command,
            Commands::ValidateSourceBinding { dir, job, file }
                if dir == PathBuf::from("example-pack")
                    && job == "prospect-fit-or-brief"
                    && file == PathBuf::from("binding.json")
        ));
    }

    #[test]
    fn source_binding_schema_target_discovers_versioned_contract_family() {
        let parsed = Cli::try_parse_from(["mdp", "schema", "source-binding"])
            .expect("source-binding schema target should parse");
        assert!(matches!(
            parsed.command,
            Commands::Schema {
                target: SchemaTarget::SourceBinding
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
            "--runner-audit",
            "/tmp/runner-audit.json",
            "--require-runner-audit",
        ])
        .expect("run-receipt should parse");

        assert!(matches!(
            parsed.command,
            Commands::RunReceipt {
                workflow: RunReceiptWorkflow::ProposalReview,
                isolation: RunIsolation::Isolated,
                declared_inputs_only: true,
                require_runner_audit: true,
                ..
            }
        ));
    }

    #[test]
    fn v1_execution_schema_targets_use_explicit_versioned_names() {
        let targets = [
            ("run-request-v1", SchemaTarget::RunRequestV1),
            ("run-bundle-v1", SchemaTarget::RunBundleV1),
            ("driver-request-v1", SchemaTarget::DriverRequestV1),
            ("driver-result-v1", SchemaTarget::DriverResultV1),
            ("driver-request-v2", SchemaTarget::DriverRequestV2),
            ("driver-result-v2", SchemaTarget::DriverResultV2),
            ("runner-audit-v1", SchemaTarget::RunnerAuditV1),
            ("run-receipt-v1", SchemaTarget::RunReceiptV1),
            ("run-verification-v1", SchemaTarget::RunVerificationV1),
            ("run-execution-v1", SchemaTarget::RunExecutionV1),
            ("decision-trace-v1", SchemaTarget::DecisionTraceV1),
            ("routed-context-v1", SchemaTarget::RoutedContextV1),
            (
                "canonical-authority-block-v1",
                SchemaTarget::CanonicalAuthorityBlockV1,
            ),
            (
                "proposal-runner-result-v1",
                SchemaTarget::ProposalRunnerResultV1,
            ),
        ];

        for (name, expected) in targets {
            let parsed = Cli::try_parse_from(["mdp", "schema", name])
                .unwrap_or_else(|error| panic!("{name} should parse: {error}"));
            assert!(
                matches!(parsed.command, Commands::Schema { target } if std::mem::discriminant(&target) == std::mem::discriminant(&expected))
            );
        }

        assert!(Cli::try_parse_from(["mdp", "schema", "run-request"]).is_err());
    }

    #[test]
    fn conformance_schema_targets_use_explicit_versioned_names() {
        let targets = [
            (
                "conformance-candidate-v1",
                SchemaTarget::ConformanceCandidateV1,
            ),
            (
                "model-invocation-evidence-v1",
                SchemaTarget::ModelInvocationEvidenceV1,
            ),
            ("evaluator-inventory-v1", SchemaTarget::EvaluatorInventoryV1),
            ("evaluator-result-v1", SchemaTarget::EvaluatorResultV1),
            (
                "private-record-policy-v1",
                SchemaTarget::PrivateRecordPolicyV1,
            ),
            (
                "publication-approval-v1",
                SchemaTarget::PublicationApprovalV1,
            ),
            ("conformance-trial-v1", SchemaTarget::ConformanceTrialV1),
            ("job-conformance-v1", SchemaTarget::JobConformanceV1),
            ("conformance-report-v1", SchemaTarget::ConformanceReportV1),
            (
                "public-conformance-report-v1",
                SchemaTarget::PublicConformanceReportV1,
            ),
            (
                "deterministic-conformance-v1",
                SchemaTarget::DeterministicConformanceV1,
            ),
            (
                "conformance-verifier-receipt-v1",
                SchemaTarget::ConformanceVerifierReceiptV1,
            ),
            (
                "behavioral-evaluation-v1",
                SchemaTarget::BehavioralEvaluationV1,
            ),
        ];
        for (name, expected) in targets {
            let parsed = Cli::try_parse_from(["mdp", "schema", name])
                .unwrap_or_else(|error| panic!("{name} should parse: {error}"));
            assert!(
                matches!(parsed.command, Commands::Schema { target } if std::mem::discriminant(&target) == std::mem::discriminant(&expected))
            );
        }
    }
}
