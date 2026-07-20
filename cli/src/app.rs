use crate::cli::{Cli, Commands, HumanBriefFormat, SampleLeadsFormat};
use crate::commands::{
    TargetInitOptions, capabilities, check_claims_scoped, demo_copy, doctor, emit_brief_scoped,
    eval_pack, explain, fit, gaps, init_pack_targeted, init_pack_targeted_dry_run, pack,
    prospect_brief_with_context, render_human_brief_file, render_human_brief_markdown,
    render_readable_prospect_brief, route_scoped, sample_leads, schema, skills, validate_pack,
    validate_prompt_output_file_with_source_audit, verify_output_file, verify_output_readable_file,
};
use crate::output::print_output;
use crate::pack_io::{planned_json_write, write_json_file};
use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
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
            target_name,
            target_kind,
            target_aliases,
            exclude_terms,
            dir,
            template,
            force,
            include_output_schemas,
            dry_run,
        } => {
            let custom_name = name.is_some();
            let resolved_name = name.unwrap_or_else(|| default_init_name(&template).to_string());
            let target_options = TargetInitOptions {
                custom_name,
                name: target_name.as_deref(),
                kind: &target_kind,
                aliases: &target_aliases,
                excluded_terms: &exclude_terms,
            };
            let data = if dry_run {
                init_pack_targeted_dry_run(
                    &dir,
                    &resolved_name,
                    &template,
                    &target_options,
                    force,
                    include_output_schemas,
                )?
            } else {
                init_pack_targeted(
                    &dir,
                    &resolved_name,
                    &template,
                    &target_options,
                    force,
                    include_output_schemas,
                )?
            };
            print_output(json_mode, summary_mode, "init", data)
        }
        Commands::Doctor { dir } => print_output(json_mode, summary_mode, "doctor", doctor(&dir)),
        Commands::Skills { dir, job } => print_output(
            json_mode,
            summary_mode,
            "skills",
            skills(dir.as_deref(), job.as_deref()),
        ),
        Commands::Validate { dir, strict } => {
            let data = apply_strict(validate_pack(&dir)?, strict, StrictWarningSource::Issues);
            print_checked(json_mode, summary_mode, "validate", data)
        }
        Commands::ValidatePromptOutput {
            dir,
            file,
            source_audit,
            prompt,
            prompt_id,
            strict,
        } => {
            let data = apply_strict(
                validate_prompt_output_file_with_source_audit(
                    &dir,
                    &file,
                    prompt.as_deref(),
                    prompt_id.as_deref(),
                    source_audit.as_deref(),
                )?,
                strict,
                StrictWarningSource::Issues,
            );
            print_checked(json_mode, summary_mode, "validate-prompt-output", data)
        }
        Commands::VerifyOutput {
            dir,
            file,
            readable,
        } => {
            if readable {
                let (markdown, data) = verify_output_readable_file(&dir, &file)?;
                println!("{markdown}");
                if data["valid"].as_bool().unwrap_or(false) {
                    Ok(())
                } else {
                    std::process::exit(1);
                }
            } else {
                let data = verify_output_file(&dir, &file)?;
                print_checked(json_mode, summary_mode, "verify-output", data)
            }
        }
        Commands::RenderBrief {
            dir,
            file,
            template,
            format,
            out,
            strict,
        } => {
            let mut data = render_human_brief_file(&dir, file.as_deref(), &template, strict)?;
            if let Some(path) = out {
                if format == HumanBriefFormat::Json {
                    data = attach_artifact(data, &path);
                    write_json_file(&path, &data)?;
                } else {
                    let markdown = render_human_brief_markdown(&data);
                    fs::write(&path, &markdown)?;
                    data = attach_markdown_artifact(data, &path);
                    if !json_mode && !summary_mode {
                        println!("{markdown}");
                        return Ok(());
                    }
                }
            } else {
                data = attach_stdout_artifact(data);
            }
            if format == HumanBriefFormat::Markdown && !json_mode && !summary_mode {
                println!("{}", render_human_brief_markdown(&data));
                Ok(())
            } else {
                print_output(
                    json_mode || format == HumanBriefFormat::Json,
                    summary_mode,
                    "render-brief",
                    data,
                )
            }
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
            scope,
            entries,
            eval_fixture,
        } => print_output(
            json_mode,
            summary_mode,
            "route",
            route_scoped(&dir, &persona, &job, &scope, entries, eval_fixture)?,
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
            scope,
            strict,
        } => {
            let data = check_claims_scoped(
                &dir,
                text.as_deref(),
                file.as_deref(),
                subject.as_deref(),
                persona.as_deref(),
                job.as_deref(),
                &scope,
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
            readable,
            out,
            dry_run,
        } => {
            let include_context = context || (readable && !json_mode && !summary_mode);
            let mut data = prospect_brief_with_context(
                &dir,
                &prospect,
                &channel,
                job.as_deref(),
                include_context,
            )?;
            data = attach_input_artifact(data, "prospect", &prospect);
            if readable && !json_mode && !summary_mode {
                let markdown = render_readable_prospect_brief(&data);
                if let Some(path) = out {
                    if dry_run {
                        let mut plan_data = attach_readable_dry_run_artifact(data, &path);
                        plan_data["readable_format"] = json!("markdown");
                        print_output(false, true, "brief", plan_data)?;
                    } else {
                        fs::write(&path, &markdown)?;
                        println!("{markdown}");
                    }
                } else {
                    println!("{markdown}");
                }
                return Ok(());
            }
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
            scope,
            out,
            dry_run,
        } => {
            let mut data =
                emit_brief_scoped(&dir, &persona, motion.as_deref(), job.as_deref(), &scope)?;
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

fn attach_readable_dry_run_artifact(mut data: Value, path: &Path) -> Value {
    let write_plan = planned_markdown_write(path);
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "artifact".to_string(),
            json!({
                "status": "dry-run",
                "kind": "markdown-file",
                "path": path.display().to_string(),
                "stdout": "also-emitted"
            }),
        );
        object.insert("dry_run".to_string(), json!(true));
        object.insert("write_plan".to_string(), Value::Array(vec![write_plan]));
    }
    data
}

fn attach_markdown_artifact(mut data: Value, path: &Path) -> Value {
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "artifact".to_string(),
            json!({
                "status": "saved",
                "kind": "markdown-file",
                "path": path.display().to_string(),
                "stdout": "also-emitted"
            }),
        );
    }
    data
}

fn planned_markdown_write(path: &Path) -> Value {
    let parent_exists = path.parent().map(Path::exists).unwrap_or(true);
    let action = if !parent_exists {
        "parent-missing"
    } else if path.exists() {
        "overwrite"
    } else {
        "create"
    };
    json!({
        "kind": "markdown-file",
        "path": path.display().to_string(),
        "action": action,
        "will_write": parent_exists,
        "parent_exists": parent_exists
    })
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
                readable: false,
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
    fn readable_brief_includes_context_even_without_context_flag() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-readable-brief-out-{nonce}"));
        init_pack(&root, "Readable Brief Out Pack", "gtm", true, false)
            .expect("pack should initialize");
        let prospect = root.join("examples").join("clay-row.json");
        let out = root.join(".mdp").join("briefs").join("brief.md");

        run(Cli {
            json: false,
            summary: false,
            command: Commands::Brief {
                dir: root.clone(),
                prospect,
                channel: "linkedin".to_string(),
                job: None,
                context: false,
                readable: true,
                out: Some(out.clone()),
                dry_run: false,
            },
        })
        .expect("readable brief command should run");

        let saved = std::fs::read_to_string(&out).expect("saved brief should be readable");
        assert!(saved.contains("- draft_status: ready"));
        assert!(saved.contains("- context_contract: mdp.context.v0"));
        assert!(saved.contains("- context_status: ready"));
        assert!(saved.contains("**Routed evidence entries**"));
        assert!(saved.contains("**Routed guardrails**"));

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
