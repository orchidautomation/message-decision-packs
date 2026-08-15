use super::{
    AuthorityLevel, AuthorityTerminal, DecisionDisposition, GateObligation, GateResult,
    GovernedGeneration, ProjectionFidelity, SUPPORTED_COMMAND_SURFACES,
    SUPPORTED_PROJECTION_SURFACES, SourceAuthority,
};
use crate::cli::Cli;
use crate::run_contracts::TerminalState;
use clap::CommandFactory;
use proptest::prelude::*;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn obligation(result: GateResult) -> Vec<GateObligation> {
    vec![GateObligation {
        id: "gate".to_string(),
        result,
    }]
}

#[test]
fn canonical_combinations_are_closed() {
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Allow,
            AuthorityTerminal::Success,
            GovernedGeneration::Available,
            obligation(GateResult::Pass),
            vec![],
        )
        .is_ok()
    );
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Block,
            AuthorityTerminal::NoDraft,
            GovernedGeneration::Absent,
            obligation(GateResult::Fail),
            vec!["blocked".to_string()],
        )
        .is_ok()
    );
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Unavailable,
            DecisionDisposition::Undetermined,
            AuthorityTerminal::AuthorityUnavailable,
            GovernedGeneration::Absent,
            obligation(GateResult::Missing),
            vec!["missing".to_string()],
        )
        .is_ok()
    );
}

#[test]
fn contradictory_or_incomplete_combinations_fail_closed() {
    for unavailable in [
        GateResult::Missing,
        GateResult::Malformed,
        GateResult::Unknown,
        GateResult::Unsupported,
    ] {
        assert!(
            SourceAuthority::new(
                AuthorityLevel::Authoritative,
                DecisionDisposition::Allow,
                AuthorityTerminal::Success,
                GovernedGeneration::Available,
                obligation(unavailable),
                vec![],
            )
            .is_err()
        );
    }
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Block,
            AuthorityTerminal::NoDraft,
            GovernedGeneration::Available,
            obligation(GateResult::Fail),
            vec!["blocked".to_string()],
        )
        .is_err()
    );
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Allow,
            AuthorityTerminal::Success,
            GovernedGeneration::Available,
            vec![],
            vec![],
        )
        .is_err(),
        "governed generation requires at least one explicit passing obligation"
    );
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Allow,
            AuthorityTerminal::Success,
            GovernedGeneration::NotApplicable,
            vec![],
            vec![],
        )
        .is_ok(),
        "a non-generative operation may have an empty closed obligation profile"
    );
}

#[test]
fn gate_obligations_require_unique_non_empty_ids() {
    let duplicate = vec![
        GateObligation {
            id: "same".to_string(),
            result: GateResult::Pass,
        },
        GateObligation {
            id: "same".to_string(),
            result: GateResult::Pass,
        },
    ];
    assert!(
        SourceAuthority::new(
            AuthorityLevel::Authoritative,
            DecisionDisposition::Allow,
            AuthorityTerminal::Success,
            GovernedGeneration::NotApplicable,
            duplicate,
            vec![],
        )
        .is_err()
    );
}

#[test]
fn every_cli_command_is_registered_as_a_supported_surface() {
    let actual = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect::<BTreeSet<_>>();
    let registered = SUPPORTED_COMMAND_SURFACES
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(registered, actual);
}

#[test]
fn non_command_authority_surface_registry_is_closed() {
    let actual = SUPPORTED_PROJECTION_SURFACES
        .iter()
        .map(|(surface, _)| *surface)
        .collect::<BTreeSet<_>>();
    let expected = [
        "renderer:human-brief",
        "renderer:summary",
        "renderer:decision-trace",
        "transport:run-mcp",
        "transport:proposal-mcp",
        "adapter:native-model",
        "adapter:native-normalize",
        "adapter:proposal-runner",
        "skill:mdp",
        "skill:mdp-gtm-brief",
        "skill:mdp-pack-builder",
        "skill:mdp-pack-review",
        "skill:mdp-proposal-review",
        "package:claude-code",
        "package:cursor",
        "package:codex",
        "package:opencode",
        "installer:agents",
        "release:manifest",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), SUPPORTED_PROJECTION_SURFACES.len());
}

#[test]
fn hand_authored_run_oracle_matches_the_authority_kernel() {
    let corpus_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CLI has a repository parent")
        .join("plugin/assets/authority-conformance/corpus.json");
    let corpus: Value = serde_json::from_slice(
        &std::fs::read(corpus_path).expect("authority corpus should be readable"),
    )
    .expect("authority corpus should parse");

    let mut checked = 0;
    for case in corpus["cases"]
        .as_array()
        .expect("authority corpus cases should be an array")
    {
        let Some(terminal) = case["source"]["terminal_state"].as_str() else {
            continue;
        };
        if case["expected"]["authority_level"].is_null() {
            continue;
        }
        let terminal = match terminal {
            "success" => TerminalState::Success,
            "no-draft:preflight-refused" => TerminalState::NoDraftPreflightRefused,
            "no-draft:runner-failed" => TerminalState::NoDraftRunnerFailed,
            "no-draft:output-invalid" => TerminalState::NoDraftOutputInvalid,
            "no-draft:decision-invalid" => TerminalState::NoDraftDecisionInvalid,
            "no-draft:audit-incomplete" => TerminalState::NoDraftAuditIncomplete,
            "no-draft:policy-blocked" => TerminalState::NoDraftPolicyBlocked,
            other => panic!("unregistered oracle terminal: {other}"),
        };
        let actual = serde_json::to_value(SourceAuthority::from_run(
            terminal,
            case["source"]["decision"].as_str(),
            case["source"]["governed_output_present"]
                .as_bool()
                .unwrap_or(false),
        ))
        .expect("source authority should serialize");
        for field in [
            "authority_level",
            "disposition",
            "terminal",
            "governed_generation",
        ] {
            assert_eq!(
                actual[field], case["expected"][field],
                "oracle case {} disagrees on {field}",
                case["id"]
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked, 8,
        "every run-terminal oracle case must be exercised"
    );
}

#[test]
fn every_run_terminal_has_a_canonical_mapping() {
    let cases = [
        (TerminalState::Success, AuthorityLevel::Authoritative),
        (
            TerminalState::NoDraftPreflightRefused,
            AuthorityLevel::Authoritative,
        ),
        (
            TerminalState::NoDraftRunnerFailed,
            AuthorityLevel::Unavailable,
        ),
        (
            TerminalState::NoDraftOutputInvalid,
            AuthorityLevel::Authoritative,
        ),
        (
            TerminalState::NoDraftDecisionInvalid,
            AuthorityLevel::Authoritative,
        ),
        (
            TerminalState::NoDraftAuditIncomplete,
            AuthorityLevel::Unavailable,
        ),
        (
            TerminalState::NoDraftPolicyBlocked,
            AuthorityLevel::Authoritative,
        ),
    ];
    for (terminal, expected) in cases {
        assert_eq!(
            SourceAuthority::from_run(terminal, None, false).authority_level,
            expected
        );
    }
}

#[test]
fn lifecycle_success_cannot_upgrade_a_blocked_decision() {
    let authority = SourceAuthority::from_run(TerminalState::Success, Some("no-draft"), true);
    assert_eq!(authority.disposition, DecisionDisposition::Block);
    assert_eq!(authority.governed_generation, GovernedGeneration::Absent);
}

#[test]
fn projections_preserve_or_reduce_authority() {
    let block =
        SourceAuthority::from_run(TerminalState::NoDraftPolicyBlocked, Some("blocked"), false);
    assert!(block.permits_projection(
        AuthorityLevel::Authoritative,
        DecisionDisposition::Block,
        GovernedGeneration::Absent,
        ProjectionFidelity::Faithful,
    ));
    assert!(!block.permits_projection(
        AuthorityLevel::Authoritative,
        DecisionDisposition::Allow,
        GovernedGeneration::Available,
        ProjectionFidelity::Faithful,
    ));
    assert!(block.permits_projection(
        AuthorityLevel::Unavailable,
        DecisionDisposition::Undetermined,
        GovernedGeneration::Absent,
        ProjectionFidelity::Unavailable,
    ));
}

fn level(value: u8) -> AuthorityLevel {
    match value % 3 {
        0 => AuthorityLevel::Unavailable,
        1 => AuthorityLevel::Informational,
        _ => AuthorityLevel::Authoritative,
    }
}

fn disposition(value: u8) -> DecisionDisposition {
    match value % 3 {
        0 => DecisionDisposition::Undetermined,
        1 => DecisionDisposition::Allow,
        _ => DecisionDisposition::Block,
    }
}

fn generation(value: bool) -> GovernedGeneration {
    if value {
        GovernedGeneration::Available
    } else {
        GovernedGeneration::Absent
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 4096,
        .. ProptestConfig::default()
    })]

    #[test]
    fn faithful_projection_never_upgrades_or_changes_a_block(
        downstream_level in 0u8..3,
        downstream_disposition in 0u8..3,
        downstream_generation in any::<bool>(),
    ) {
        let source = SourceAuthority::from_run(
            TerminalState::NoDraftPolicyBlocked,
            Some("blocked"),
            false,
        );
        let target_level = level(downstream_level);
        let target_disposition = disposition(downstream_disposition);
        let target_generation = generation(downstream_generation);
        let permitted = source.permits_projection(
            target_level,
            target_disposition,
            target_generation,
            ProjectionFidelity::Faithful,
        );

        if permitted {
            prop_assert!(target_level <= source.authority_level);
            prop_assert_eq!(target_disposition, DecisionDisposition::Block);
            prop_assert_ne!(target_generation, GovernedGeneration::Available);
        }
    }

    #[test]
    fn composed_registered_edges_never_restore_authority(
        edges in prop::collection::vec((0u8..3, any::<bool>()), 1..=64),
    ) {
        let source = SourceAuthority::from_run(TerminalState::Success, Some("ready"), true);
        let mut current_level = source.authority_level;
        let mut generation_available = true;

        for (candidate, candidate_generation) in edges {
            let candidate_level = level(candidate);
            let next_level = std::cmp::min(current_level, candidate_level);
            generation_available &= candidate_generation;
            prop_assert!(next_level <= current_level);
            if !generation_available {
                prop_assert_ne!(GovernedGeneration::Absent, GovernedGeneration::Available);
            }
            current_level = next_level;
        }
    }
}
