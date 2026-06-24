use crate::cli::{Cli, Commands};
use crate::commands::{
    check_claims, demo_copy, doctor, emit_brief, eval_pack, explain, fit, gaps, init_pack, pack,
    prospect_brief, route, schema, validate_pack,
};
use crate::output::print_output;
use crate::pack_io::write_json_file;
use anyhow::Result;
use serde_json::Value;

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init {
            name,
            dir,
            template,
            force,
        } => {
            let data = init_pack(&dir, &name, &template, force)?;
            print_output(cli.json, "init", data)
        }
        Commands::Doctor { dir } => print_output(cli.json, "doctor", doctor(&dir)),
        Commands::Validate { dir } => {
            let data = validate_pack(&dir)?;
            print_checked(cli.json, "validate", data)
        }
        Commands::Explain { dir, persona } => {
            print_output(cli.json, "explain", explain(&dir, persona.as_deref())?)
        }
        Commands::Route {
            dir,
            persona,
            job,
            entries,
        } => print_output(cli.json, "route", route(&dir, &persona, &job, entries)?),
        Commands::Fit { dir, prospect } => print_output(cli.json, "fit", fit(&dir, &prospect)?),
        Commands::CheckClaims { dir, text, file } => {
            let data = check_claims(&dir, text.as_deref(), file.as_deref())?;
            print_checked(cli.json, "check-claims", data)
        }
        Commands::Gaps { dir } => print_output(cli.json, "gaps", gaps(&dir)?),
        Commands::Eval { dir } => {
            let data = eval_pack(&dir)?;
            print_checked(cli.json, "eval", data)
        }
        Commands::Brief {
            dir,
            prospect,
            channel,
            job,
            out,
        } => {
            let data = prospect_brief(&dir, &prospect, &channel, job.as_deref())?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "brief", data)
        }
        Commands::Copy {
            dir,
            prospect,
            channel,
            out,
        } => {
            let data = demo_copy(&dir, &prospect, &channel)?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "copy", data)
        }
        Commands::EmitBrief {
            dir,
            persona,
            motion,
            job,
            out,
        } => {
            let data = emit_brief(&dir, &persona, motion.as_deref(), job.as_deref())?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "emit-brief", data)
        }
        Commands::Pack { dir, out } => {
            let data = pack(&dir)?;
            if let Some(path) = out {
                write_json_file(&path, &data)?;
            }
            print_output(cli.json, "pack", data)
        }
        Commands::Schema { target } => print_output(cli.json, "schema", schema(target)),
    }
}

fn print_checked(json_mode: bool, command: &str, data: Value) -> Result<()> {
    let valid = data["valid"].as_bool().unwrap_or(true);
    print_output(json_mode, command, data)?;
    if valid {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
