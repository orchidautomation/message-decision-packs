#![recursion_limit = "256"]

mod app;
mod artifact_hash;
mod authority;
mod cli;
mod commands;
mod conformance;
mod constants;
mod decision_input;
mod diagnostics;
mod model_steps;
mod models;
mod output;
mod pack_io;
mod pack_readme;
mod primitives;
mod product_foundation;
mod prospect_validation;
mod routing;
mod run_contracts;
mod run_replay;
mod run_request_compiler;
mod run_runtime;
mod runtime_context;
mod scope;
mod skill_catalog;
mod starter;
mod target_starter;
mod utils;
mod value_contracts;

use crate::cli::Cli;
use crate::output::{DisplayKind, print_error};
use crate::run_request_compiler::CompilerError;
use clap::Parser;
use clap::error::ErrorKind;

fn main() {
    let raw_args = std::env::args().collect::<Vec<_>>();
    let json_mode = raw_args.iter().any(|arg| arg == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let is_display = matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            );
            if is_display {
                if json_mode {
                    // Render the help or version text via the clap error
                    // itself. `err.render()` carries the parsed command
                    // context (top-level or subcommand) so `mdp --json trace
                    // --help` returns the `trace` help, not the root help,
                    // and bypasses clap's color/stderr path.
                    let kind = if matches!(err.kind(), ErrorKind::DisplayHelp) {
                        DisplayKind::Help
                    } else {
                        DisplayKind::Version
                    };
                    let text = err.render().to_string();
                    let _ = crate::output::print_display_envelope(true, kind, &text);
                } else {
                    let _ = err.print();
                }
                std::process::exit(0);
            }
            let exit_code = 2;
            if json_mode {
                if is_prepare_run_invocation(&raw_args) {
                    let data = serde_json::json!({
                        "contract": "mdp.run-request-compile.v1",
                        "status": "blocked",
                        "diagnostics": [{
                            "code": "cli-arguments-invalid",
                            "contract": "mdp.run-request-compile.v1",
                            "message": "cli-arguments-invalid: preparation refused",
                            "next_command": "mdp prepare-run --help"
                        }],
                        "next_command": "mdp prepare-run --help"
                    });
                    println!("{}", prepare_run_error_envelope(data));
                } else {
                    let _ = print_error(json_mode, anyhow::anyhow!(err.to_string()));
                }
            } else {
                let _ = print_error(false, anyhow::anyhow!(err.to_string()));
            }
            std::process::exit(exit_code);
        }
    };
    let json_mode = cli.json;
    if let Err(err) = app::run(cli) {
        if json_mode {
            if let Some(failure) = err.downcast_ref::<CompilerError>() {
                let diagnostic = &failure.0;
                let data = serde_json::json!({
                    "contract": diagnostic.contract, "status": "blocked",
                    "diagnostics": [diagnostic], "next_command": diagnostic.next_command
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&prepare_run_error_envelope(data))
                        .unwrap_or_else(|_| "{\"ok\":false}".into())
                );
            } else {
                let _ = print_error(json_mode, err);
            }
        } else {
            let _ = print_error(json_mode, err);
        }
        std::process::exit(1);
    }
}

fn prepare_run_error_envelope(data: serde_json::Value) -> serde_json::Value {
    let actionable_diagnostics = crate::diagnostics::diagnostics_for_result("prepare-run", &data);
    let mut envelope = serde_json::json!({
        "ok": false,
        "command": "prepare-run",
        "data": data
    });
    if let (Some(object), Some(diagnostics)) = (envelope.as_object_mut(), actionable_diagnostics) {
        object.insert(
            "diagnostic_contract".to_string(),
            serde_json::json!(crate::diagnostics::ACTIONABLE_DIAGNOSTIC_CONTRACT),
        );
        object.insert(
            crate::diagnostics::ACTIONABLE_DIAGNOSTICS_FIELD.to_string(),
            diagnostics,
        );
    }
    envelope
}

fn is_prepare_run_invocation(args: &[String]) -> bool {
    let mut value_expected = false;
    for argument in args.iter().skip(1) {
        if value_expected {
            value_expected = false;
            continue;
        }
        if argument.starts_with('-') {
            if !argument.contains('=') && option_takes_value(argument) {
                value_expected = true;
            }
            continue;
        }
        return argument == "prepare-run";
    }
    false
}

fn option_takes_value(argument: &str) -> bool {
    matches!(
        argument,
        "--dir"
            | "--job"
            | "--operation"
            | "--input"
            | "--model"
            | "--retention-policy"
            | "--created-at"
            | "--out"
            | "--manifest-out"
            | "--request"
            | "--out-dir"
            | "--receipt"
            | "--bundle"
            | "--artifact-root"
            | "--scope"
            | "--text"
            | "--file"
            | "--prompt"
            | "--prompt-id"
            | "--count"
    )
}

#[cfg(test)]
mod tests {
    use super::is_prepare_run_invocation;

    #[test]
    fn parse_envelope_routing_uses_subcommand_position() {
        let args = |values: &[&str]| -> Vec<String> {
            values.iter().map(|value| (*value).to_string()).collect()
        };
        assert!(is_prepare_run_invocation(&args(&[
            "mdp",
            "--json",
            "prepare-run",
            "--bad"
        ])));
        assert!(!is_prepare_run_invocation(&args(&[
            "mdp",
            "--json",
            "validate",
            "--dir",
            "prepare-run",
            "--bad"
        ])));
    }
}
