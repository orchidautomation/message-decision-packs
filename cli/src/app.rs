use crate::cli::{Cli, Commands};
use crate::commands::{
    check_claims, demo_copy, doctor, emit_brief, eval_pack, explain, fit, gaps, init_pack, pack,
    prospect_brief_with_context, route, schema, validate_pack,
};
use crate::output::print_output;
use crate::pack_io::write_json_file;
use anyhow::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) fn run(cli: Cli) -> Result<()> {
    let json_mode = cli.json;
    let summary_mode = cli.summary;
    match cli.command {
        Commands::Init {
            name,
            dir,
            template,
            force,
            include_output_schemas,
        } => {
            let data = init_pack(&dir, &name, &template, force, include_output_schemas)?;
            print_output(json_mode, summary_mode, "init", data)
        }
        Commands::Doctor { dir } => print_output(json_mode, summary_mode, "doctor", doctor(&dir)),
        Commands::Validate { dir } => {
            let data = validate_pack(&dir)?;
            print_checked(json_mode, summary_mode, "validate", data)
        }
        Commands::Explain { dir, persona } => print_output(
            json_mode,
            summary_mode,
            "explain",
            explain(&dir, persona.as_deref())?,
        ),
        Commands::Route {
            dir,
            persona,
            job,
            entries,
            eval_fixture,
        } => print_output(
            json_mode,
            summary_mode,
            "route",
            route(&dir, &persona, &job, entries, eval_fixture)?,
        ),
        Commands::Fit { dir, prospect } => {
            print_output(json_mode, summary_mode, "fit", fit(&dir, &prospect)?)
        }
        Commands::CheckClaims { dir, text, file } => {
            let data = check_claims(&dir, text.as_deref(), file.as_deref())?;
            print_checked(json_mode, summary_mode, "check-claims", data)
        }
        Commands::Gaps { dir } => print_output(json_mode, summary_mode, "gaps", gaps(&dir)?),
        Commands::Eval { dir } => {
            let data = eval_pack(&dir)?;
            print_checked(json_mode, summary_mode, "eval", data)
        }
        Commands::Brief {
            dir,
            prospect,
            channel,
            job,
            context,
            out,
        } => {
            let mut data =
                prospect_brief_with_context(&dir, &prospect, &channel, job.as_deref(), context)?;
            data = attach_input_artifact(data, "prospect", &prospect);
            if let Some(path) = out {
                data = attach_artifact(data, &path);
                write_json_file(&path, &data)?;
            } else {
                data = attach_stdout_artifact(data);
            }
            print_output(json_mode, summary_mode, "brief", data)
        }
        Commands::Copy {
            dir,
            prospect,
            channel,
            out,
        } => {
            let mut data = demo_copy(&dir, &prospect, &channel)?;
            data = attach_input_artifact(data, "prospect", &prospect);
            if let Some(path) = out {
                data = attach_artifact(data, &path);
                write_json_file(&path, &data)?;
            } else {
                data = attach_stdout_artifact(data);
            }
            print_output(json_mode, summary_mode, "copy", data)
        }
        Commands::EmitBrief {
            dir,
            persona,
            motion,
            job,
            out,
        } => {
            let mut data = emit_brief(&dir, &persona, motion.as_deref(), job.as_deref())?;
            if let Some(path) = out {
                data = attach_artifact(data, &path);
                write_json_file(&path, &data)?;
            } else {
                data = attach_stdout_artifact(data);
            }
            print_output(json_mode, summary_mode, "emit-brief", data)
        }
        Commands::Pack { dir, out } => {
            let mut data = pack(&dir)?;
            if let Some(path) = out {
                data = attach_artifact(data, &path);
                write_json_file(&path, &data)?;
            } else {
                data = attach_stdout_artifact(data);
            }
            print_output(json_mode, summary_mode, "pack", data)
        }
        Commands::Schema { target } => {
            print_output(json_mode, summary_mode, "schema", schema(target))
        }
    }
}

fn print_checked(json_mode: bool, summary_mode: bool, command: &str, data: Value) -> Result<()> {
    let valid = data["valid"].as_bool().unwrap_or(true);
    print_output(json_mode, summary_mode, command, data)?;
    if valid {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn attach_artifact(mut data: Value, path: &Path) -> Value {
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "artifact".to_string(),
            json!({
                "status": "saved",
                "kind": "json-file",
                "path": path.display().to_string(),
                "stdout": "also-emitted"
            }),
        );
    }
    data
}

fn attach_stdout_artifact(mut data: Value) -> Value {
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "artifact".to_string(),
            json!({
                "status": "stdout-only",
                "kind": "stdout",
                "path": Value::Null
            }),
        );
    }
    data
}

fn attach_input_artifact(mut data: Value, kind: &str, path: &PathBuf) -> Value {
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "input_artifact".to_string(),
            json!({
                "kind": kind,
                "path": path.display().to_string()
            }),
        );
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Commands};
    use crate::commands::init::init_pack;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn attach_artifact_marks_saved_json_file() {
        let path = PathBuf::from("/tmp/brief.json");
        let result = attach_artifact(json!({"contract": "mdp.message-brief.v0"}), &path);

        assert_eq!(result["artifact"]["status"], "saved");
        assert_eq!(result["artifact"]["kind"], "json-file");
        assert_eq!(result["artifact"]["path"], "/tmp/brief.json");
    }

    #[test]
    fn attach_stdout_artifact_marks_stdout_only_output() {
        let result = attach_stdout_artifact(json!({"contract": "mdp.message-brief.v0"}));

        assert_eq!(result["artifact"]["status"], "stdout-only");
        assert_eq!(result["artifact"]["kind"], "stdout");
        assert!(result["artifact"]["path"].is_null());
    }

    #[test]
    fn brief_out_writes_self_describing_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-brief-out-{nonce}"));
        init_pack(&root, "Brief Out Pack", "gtm", true, false).expect("pack should initialize");
        let prospect = root.join("examples").join("clay-row.json");
        let out = root.join(".mdp").join("briefs").join("brief.json");

        run(Cli {
            json: true,
            summary: true,
            command: Commands::Brief {
                dir: root.clone(),
                prospect,
                channel: "linkedin".to_string(),
                job: None,
                context: true,
                out: Some(out.clone()),
            },
        })
        .expect("brief command should run");

        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(&out).expect("saved brief should be readable"),
        )
        .expect("saved brief should parse");
        assert_eq!(saved["artifact"]["status"], "saved");
        assert_eq!(saved["input_artifact"]["kind"], "prospect");
        assert_eq!(saved["context"]["contract"], "mdp.context.v0");

        let _ = std::fs::remove_dir_all(root);
    }
}
