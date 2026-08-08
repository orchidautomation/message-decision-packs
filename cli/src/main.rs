#![recursion_limit = "256"]

mod app;
mod artifact_hash;
mod cli;
mod commands;
mod constants;
mod models;
mod output;
mod pack_io;
mod pack_readme;
mod product_foundation;
mod prospect_validation;
mod routing;
mod run_contracts;
mod run_replay;
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
use clap::Parser;
use clap::error::ErrorKind;

fn main() {
    let json_mode = std::env::args().any(|arg| arg == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let is_display = matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            );
            let exit_code = if is_display { 0 } else { 2 };
            if json_mode && !is_display {
                let _ = print_error(json_mode, anyhow::anyhow!(err.to_string()));
            } else {
                let _ = err.print();
            }
            std::process::exit(exit_code);
        }
    };
    let json_mode = cli.json;
    if let Err(err) = app::run(cli) {
        let _ = print_error(json_mode, err);
        std::process::exit(1);
    }
}
