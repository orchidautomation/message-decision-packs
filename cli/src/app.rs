use crate::cli::{Cli, Commands, SampleLeadsFormat};
use crate::commands::{
    capabilities, check_claims, demo_copy, doctor, emit_brief, eval_pack, explain, fit, gaps,
    init_pack, init_pack_dry_run, pack, prospect_brief_with_context, route, sample_leads, schema,
    validate_pack, validate_prompt_output_file,
};
use crate::output::print_output;
use crate::pack_io::{planned_json_write, write_json_file};
use anyhow::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) fn run(cli: Cli) -> Result<()> {
    let json_mode = cli.json;
    let summary_mode = cli.summary;
    match cli.command {
        Commands::Capabilities => {
            print_output(json_mode, summary_mode, "capabilities", capabilities())
        }
        Commands::Init {
            name,
            dir,
            template,
            force,
            include_output_schemas,
            dry_run,
        } => {
            let resolved_name = name.unwrap_or_else(|| default_init_name(&template).to_string());
            let data = if dry_run {
                init_pack_dry_run(
                    &dir,
                    &resolved_name,
                    &template,
                    force,
                    include_output_schemas,
                )?
            } else {
                init_pack(
                    &dir,
                    &resolved_name,
                    &template,
                    force,
                    include_output_schemas,
                )?
            };
            print_output(json_mode, summary_mode, "init", data)
        }
        Commands::Doctor { dir } => print_output(json_mode, summary_mode, "doctor", doctor(&dir)),
        Commands::Validate { dir, strict } => {
            let data = apply_strict(validate_pack(&dir)?, strict, StrictWarningSource::Issues);
            print_checked(json_mode, summary_mode, "validate", data)
        }
        Commands::ValidatePromptOutput {
            dir,
            file,
            prompt,
            prompt_id,
            strict,
        } => {
            let data = apply_strict(
                validate_prompt_output_file(&dir, &file, prompt.as_deref(), prompt_id.as_deref())?,
                strict,
                StrictWarningSource::Issues,
            );
            print_checked(json_mode, summary_mode, "validate-prompt-output", data)
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
        Commands::SampleLeads {
            dir,
            persona,
            job,
            count,
            seed,
            format,
        } => {
            let data = sample_leads(&dir, &persona, &job, count, seed)?;
            print_sample_leads(json_mode, summary_mode, format, data)
        }
        Commands::Fit { dir, prospect } => {
            print_output(json_mode, summary_mode, "fit", fit(&dir, &prospect)?)
        }
        Commands::CheckClaims {
            dir,
            text,
            file,
            subject,
            persona,
            job,
            strict,
        } => {
            let data = check_claims(
                &dir,
                text.as_deref(),
                file.as_deref(),
                subject.as_deref(),
                persona.as_deref(),
                job.as_deref(),
            )?;
            let data = apply_strict(data, strict, StrictWarningSource::ConstraintWarnings);
            print_checked(json_mode, summary_mode, "check-claims", data)
        }
        Commands::Gaps { dir } => print_output(json_mode, summary_mode, "gaps", gaps(&dir)?),
        Commands::Eval { dir, strict } => {
            let data = apply_strict(eval_pack(&dir)?, strict, StrictWarningSource::Issues);
            print_checked(json_mode, summary_mode, "eval", data)
        }
        Commands::Brief {
            dir,
            prospect,
            channel,
            job,
            context,
            out,
            dry_run,
        } => {
            let mut data =
                prospect_brief_with_context(&dir, &prospect, &channel, job.as_deref(), context)?;
            data = attach_input_artifact(data, "prospect", &prospect);
            if let Some(path) = out {
                if dry_run {
                    data = attach_dry_run_artifact(data, &path);
                } else {
                    data = attach_artifact(data, &path);
                    write_json_file(&path, &data)?;
                }
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
            dry_run,
        } => {
            let mut data = emit_brief(&dir, &persona, motion.as_deref(), job.as_deref())?;
            if let Some(path) = out {
                if dry_run {
                    data = attach_dry_run_artifact(data, &path);
                } else {
                    data = attach_artifact(data, &path);
                    write_json_file(&path, &data)?;
                }
            } else {
                data = attach_stdout_artifact(data);
            }
            print_output(json_mode, summary_mode, "emit-brief", data)
        }
        Commands::Pack { dir, out, dry_run } => {
            let mut data = pack(&dir)?;
            if let Some(path) = out {
                if dry_run {
                    data = attach_dry_run_artifact(data, &path);
                } else {
                    data = attach_artifact(data, &path);
                    write_json_file(&path, &data)?;
                }
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

fn default_init_name(template: &str) -> &'static str {
    match template {
        "proposal" => "Proposal Reference Profile Sample",
        _ => "Example Message Pack",
    }
}

fn print_sample_leads(
    json_mode: bool,
    summary_mode: bool,
    format: SampleLeadsFormat,
    data: Value,
) -> Result<()> {
    if json_mode || summary_mode || format == SampleLeadsFormat::Json {
        return print_output(json_mode, summary_mode, "sample-leads", data);
    }

    println!("{}", serde_yaml::to_string(&data)?);
    Ok(())
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

#[derive(Clone, Copy)]
enum StrictWarningSource {
    Issues,
    ConstraintWarnings,
}

fn apply_strict(mut data: Value, strict: bool, source: StrictWarningSource) -> Value {
    if !strict {
        return data;
    }

    let warnings = match source {
        StrictWarningSource::Issues => data["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|issue| issue["severity"].as_str() == Some("warning"))
            .cloned()
            .collect::<Vec<_>>(),
        StrictWarningSource::ConstraintWarnings => data["constraint_warnings"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    };

    if let Some(object) = data.as_object_mut() {
        object.insert(
            "strict".to_string(),
            json!({
                "enabled": true,
                "warning_count": warnings.len(),
                "warnings_fail": true,
                "source": match source {
                    StrictWarningSource::Issues => "issues",
                    StrictWarningSource::ConstraintWarnings => "constraint_warnings",
                }
            }),
        );
        if !warnings.is_empty() {
            object.insert("valid".to_string(), json!(false));
            object.insert("strict_warnings".to_string(), Value::Array(warnings));
        }
    }
    data
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

fn attach_dry_run_artifact(mut data: Value, path: &Path) -> Value {
    let write_plan = planned_json_write(path);
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "artifact".to_string(),
            json!({
                "status": "dry-run",
                "kind": "json-file",
                "path": path.display().to_string(),
                "stdout": "also-emitted"
            }),
        );
        object.insert("dry_run".to_string(), json!(true));
        object.insert("write_plan".to_string(), Value::Array(vec![write_plan]));
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
                dry_run: false,
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

    #[test]
    fn strict_claim_check_warnings_can_fail_validity() {
        let data = apply_strict(
            json!({
                "valid": true,
                "constraint_warnings": [{"code": "target_word_count", "message": "too short"}]
            }),
            true,
            StrictWarningSource::ConstraintWarnings,
        );

        assert_eq!(data["valid"], false);
        assert_eq!(data["strict"]["warning_count"], 1);
        assert_eq!(data["strict_warnings"][0]["code"], "target_word_count");
    }

    #[test]
    fn dry_run_artifact_does_not_mark_saved() {
        let path = PathBuf::from("/tmp/brief.json");
        let result = attach_dry_run_artifact(json!({"contract": "mdp.message-brief.v0"}), &path);

        assert_eq!(result["artifact"]["status"], "dry-run");
        assert_eq!(result["dry_run"], true);
        assert_eq!(result["write_plan"][0]["path"], "/tmp/brief.json");
    }
}
