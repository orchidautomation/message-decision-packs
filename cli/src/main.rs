#![recursion_limit = "256"]

mod app;
mod artifact_hash;
mod authority;
mod cli;
mod commands;
mod conformance;
mod constants;
mod model_steps;
mod models;
mod output;
mod pack_io;
mod pack_readme;
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
use crate::output::print_error;
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
            let exit_code = if is_display { 0 } else { 2 };
            if json_mode && !is_display {
                if is_prepare_run_invocation(&raw_args) {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": false,
                            "command": "prepare-run",
                            "data": {
                                "contract": "mdp.run-request-compile.v1",
                                "status": "blocked",
                                "diagnostics": [{
                                    "code": "cli-arguments-invalid",
                                    "contract": "mdp.run-request-compile.v1",
                                    "message": "cli-arguments-invalid: preparation refused",
                                    "next_command": "mdp prepare-run --help"
                                }],
                                "next_command": "mdp prepare-run --help"
                            }
                        })
                    );
                } else {
                    let _ = print_error(json_mode, anyhow::anyhow!(err.to_string()));
                }
            } else {
                let _ = err.print();
            }
            std::process::exit(exit_code);
        }
    };
    let json_mode = cli.json;
    if let Err(err) = app::run(cli) {
        if json_mode {
            if let Some(failure) = err.downcast_ref::<CompilerError>() {
                let diagnostic = &failure.0;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": false, "command": "prepare-run", "data": {
                            "contract": diagnostic.contract, "status": "blocked",
                            "diagnostics": [diagnostic], "next_command": diagnostic.next_command
                        }
                    }))
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
