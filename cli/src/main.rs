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
    let cli = Cli::parse();
    let json_mode = cli.json;
    if let Err(err) = app::run(cli) {
        let _ = print_error(json_mode, err);
        std::process::exit(1);
    }
}
