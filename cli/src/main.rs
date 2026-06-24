mod app;
mod cli;
mod commands;
mod constants;
mod models;
mod output;
mod pack_io;
mod routing;
mod starter;
mod utils;

use crate::cli::Cli;
use crate::output::print_error;
use clap::Parser;

fn main() {
    let json_mode = std::env::args().any(|arg| arg == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if json_mode {
                let _ = print_error(json_mode, anyhow::anyhow!(err.to_string()));
            } else {
                let _ = err.print();
            }
            std::process::exit(2);
        }
    };
    let json_mode = cli.json;
    if let Err(err) = app::run(cli) {
        let _ = print_error(json_mode, err);
        std::process::exit(1);
    }
}
