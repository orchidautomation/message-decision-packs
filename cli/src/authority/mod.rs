use crate::run_contracts::TerminalState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AuthorityLevel {
    Unavailable,
    Informational,
    Authoritative,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionDisposition {
    Undetermined,
    Allow,
    Block,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AuthorityTerminal {
    AuthorityUnavailable,
    DiagnosticComplete,
    Success,
    NoDraft,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GovernedGeneration {
    NotApplicable,
    Absent,
    Available,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GateResult {
    Pass,
    Fail,
    Missing,
    Malformed,
    Unknown,
    Unsupported,
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GateObligation {
    pub(crate) id: String,
    pub(crate) result: GateResult,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceAuthority {
    pub(crate) authority_level: AuthorityLevel,
    pub(crate) disposition: DecisionDisposition,
    pub(crate) terminal: AuthorityTerminal,
    pub(crate) governed_generation: GovernedGeneration,
    pub(crate) obligations: Vec<GateObligation>,
    pub(crate) reason_codes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProjectionFidelity {
    Faithful,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SurfaceRole {
    AuthorityOrigin,
    Projection,
    Verifier,
    Lifecycle,
    Diagnostic,
    ArtifactWriter,
    Transport,
    Adapter,
    Guidance,
    Package,
}

pub(crate) const SUPPORTED_COMMAND_SURFACES: &[(&str, SurfaceRole)] = &[
    ("capabilities", SurfaceRole::Projection),
    ("conformance", SurfaceRole::Verifier),
    ("init", SurfaceRole::ArtifactWriter),
    ("doctor", SurfaceRole::Diagnostic),
    ("skills", SurfaceRole::Projection),
    ("requirements", SurfaceRole::Projection),
    ("validate-source-binding", SurfaceRole::Verifier),
    ("validate", SurfaceRole::Verifier),
    ("validate-prompt-output", SurfaceRole::Verifier),
    ("run-receipt", SurfaceRole::AuthorityOrigin),
    ("verify-run", SurfaceRole::Verifier),
    ("trace", SurfaceRole::Projection),
    ("consume-run", SurfaceRole::Lifecycle),
    ("run", SurfaceRole::AuthorityOrigin),
    ("verify-output", SurfaceRole::Verifier),
    ("author-proof-output", SurfaceRole::ArtifactWriter),
    ("render-brief", SurfaceRole::Projection),
    ("explain", SurfaceRole::Projection),
    ("route", SurfaceRole::Projection),
    ("sample-leads", SurfaceRole::ArtifactWriter),
    ("fit", SurfaceRole::AuthorityOrigin),
    ("check-claims", SurfaceRole::Verifier),
    ("gaps", SurfaceRole::Diagnostic),
    ("eval", SurfaceRole::Verifier),
    ("brief", SurfaceRole::Projection),
    ("copy", SurfaceRole::Projection),
    ("emit-brief", SurfaceRole::Projection),
    ("pack", SurfaceRole::ArtifactWriter),
    ("schema", SurfaceRole::Projection),
];

pub(crate) const SUPPORTED_PROJECTION_SURFACES: &[(&str, SurfaceRole)] = &[
    ("renderer:human-brief", SurfaceRole::Projection),
    ("renderer:summary", SurfaceRole::Projection),
    ("renderer:decision-trace", SurfaceRole::Projection),
    ("transport:run-mcp", SurfaceRole::Transport),
    ("transport:proposal-mcp", SurfaceRole::Transport),
    ("adapter:native-model", SurfaceRole::Adapter),
    ("adapter:native-normalize", SurfaceRole::Adapter),
    ("adapter:proposal-runner", SurfaceRole::Adapter),
    ("skill:mdp", SurfaceRole::Guidance),
    ("skill:mdp-gtm-brief", SurfaceRole::Guidance),
    ("skill:mdp-pack-builder", SurfaceRole::Guidance),
    ("skill:mdp-pack-review", SurfaceRole::Guidance),
    ("skill:mdp-proposal-review", SurfaceRole::Guidance),
    ("package:claude-code", SurfaceRole::Package),
    ("package:cursor", SurfaceRole::Package),
    ("package:codex", SurfaceRole::Package),
    ("package:opencode", SurfaceRole::Package),
    ("installer:agents", SurfaceRole::Package),
    ("release:manifest", SurfaceRole::Package),
];

impl SourceAuthority {
    pub(crate) fn new(
        authority_level: AuthorityLevel,
        disposition: DecisionDisposition,
        terminal: AuthorityTerminal,
        governed_generation: GovernedGeneration,
        obligations: Vec<GateObligation>,
        reason_codes: Vec<String>,
    ) -> Result<Self, &'static str> {
        let mut obligation_ids = BTreeSet::new();
        if obligations.iter().any(|obligation| {
            obligation.id.trim().is_empty() || !obligation_ids.insert(&obligation.id)
        }) {
            return Err("gate obligations must have non-empty unique ids");
        }

        let has_denial = obligations
            .iter()
            .any(|obligation| obligation.result == GateResult::Fail);
        let has_unavailable = obligations.iter().any(|obligation| {
            matches!(
                obligation.result,
                GateResult::Missing
                    | GateResult::Malformed
                    | GateResult::Unknown
                    | GateResult::Unsupported
            )
        });
        let all_applicable_pass = obligations.iter().all(|obligation| {
            matches!(
                obligation.result,
                GateResult::Pass | GateResult::NotApplicable
            )
        });

        let valid = match (authority_level, disposition, terminal, governed_generation) {
            (
                AuthorityLevel::Unavailable,
                DecisionDisposition::Undetermined,
                AuthorityTerminal::AuthorityUnavailable,
                GovernedGeneration::Absent | GovernedGeneration::NotApplicable,
            ) => has_unavailable && !reason_codes.is_empty(),
            (
                AuthorityLevel::Informational,
                DecisionDisposition::Undetermined,
                AuthorityTerminal::DiagnosticComplete,
                GovernedGeneration::Absent | GovernedGeneration::NotApplicable,
            ) => !has_denial && !has_unavailable,
            (
                AuthorityLevel::Authoritative,
                DecisionDisposition::Allow,
                AuthorityTerminal::Success,
                GovernedGeneration::Available | GovernedGeneration::NotApplicable,
            ) => {
                all_applicable_pass
                    && reason_codes.is_empty()
                    && (governed_generation == GovernedGeneration::NotApplicable
                        || !obligations.is_empty())
            }
            (
                AuthorityLevel::Authoritative,
                DecisionDisposition::Block,
                AuthorityTerminal::NoDraft,
                GovernedGeneration::Absent,
            ) => has_denial && !reason_codes.is_empty(),
            _ => false,
        };

        if !valid {
            return Err("authority dimensions contradict gate or reason state");
        }

        Ok(Self {
            authority_level,
            disposition,
            terminal,
            governed_generation,
            obligations,
            reason_codes,
        })
    }

    pub(crate) fn from_run(
        terminal_state: TerminalState,
        decision: Option<&str>,
        governed_output_present: bool,
    ) -> Self {
        let decision_blocked = matches!(decision, Some("blocked" | "no-draft"));
        match terminal_state {
            TerminalState::Success if decision_blocked => {
                Self::block("run-decision-blocked", "run-decision")
            }
            TerminalState::Success => Self::allow(governed_output_present),
            TerminalState::NoDraftPreflightRefused => {
                Self::block("run-preflight-refused", "preflight")
            }
            TerminalState::NoDraftOutputInvalid => {
                Self::block("run-output-invalid", "output-validation")
            }
            TerminalState::NoDraftDecisionInvalid => {
                Self::block("run-decision-invalid", "decision-validation")
            }
            TerminalState::NoDraftPolicyBlocked => Self::block("run-policy-blocked", "policy"),
            TerminalState::NoDraftRunnerFailed => Self::unavailable("run-runner-failed", "runner"),
            TerminalState::NoDraftAuditIncomplete => {
                Self::unavailable("run-audit-incomplete", "audit")
            }
        }
    }

    pub(crate) fn permits_projection(
        &self,
        downstream_level: AuthorityLevel,
        downstream_disposition: DecisionDisposition,
        downstream_generation: GovernedGeneration,
        fidelity: ProjectionFidelity,
    ) -> bool {
        if fidelity == ProjectionFidelity::Unavailable {
            return downstream_level == AuthorityLevel::Unavailable
                && downstream_generation != GovernedGeneration::Available;
        }
        if downstream_level > self.authority_level {
            return false;
        }
        if self.authority_level == AuthorityLevel::Authoritative
            && downstream_disposition != self.disposition
        {
            return false;
        }
        downstream_generation != GovernedGeneration::Available
            || self.governed_generation == GovernedGeneration::Available
    }

    fn allow(governed_output_present: bool) -> Self {
        Self::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Allow,
            AuthorityTerminal::Success,
            if governed_output_present {
                GovernedGeneration::Available
            } else {
                GovernedGeneration::NotApplicable
            },
            vec![GateObligation {
                id: "run-terminal".to_string(),
                result: GateResult::Pass,
            }],
            Vec::new(),
        )
        .expect("canonical allow state should be valid")
    }

    pub(crate) fn block(reason: &str, gate: &str) -> Self {
        Self::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Block,
            AuthorityTerminal::NoDraft,
            GovernedGeneration::Absent,
            vec![GateObligation {
                id: gate.to_string(),
                result: GateResult::Fail,
            }],
            vec![reason.to_string()],
        )
        .expect("canonical block state should be valid")
    }

    pub(crate) fn unavailable(reason: &str, gate: &str) -> Self {
        Self::new(
            AuthorityLevel::Unavailable,
            DecisionDisposition::Undetermined,
            AuthorityTerminal::AuthorityUnavailable,
            GovernedGeneration::Absent,
            vec![GateObligation {
                id: gate.to_string(),
                result: GateResult::Unknown,
            }],
            vec![reason.to_string()],
        )
        .expect("canonical unavailable state should be valid")
    }
}

#[cfg(test)]
mod tests;
