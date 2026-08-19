use crate::artifact_hash::{canonical_json_bytes, sha256_hex};
use crate::cli::{
    Cli, Commands, ConformanceCommand, ConformanceReportVisibility, HumanBriefFormat,
    ReadmeCommand, SampleLeadsFormat, TraceFormat,
};
use crate::commands::briefs::prospect_brief_from_fit_with_context;
use crate::commands::prompt_output::validate_prompt_output_file_with_lineage_inputs;
use crate::commands::routing::{fit_for_job, fit_normalized};
use crate::commands::{
    AssembleConformancePaths, BehavioralEvidencePaths, RunReceiptOptions, TargetInitOptions,
    assemble_conformance, author_proof_output_file, capabilities, check_claims_scoped,
    check_readme, compile_candidate_file, demo_copy, doctor, emit_brief_scoped, eval_pack, explain,
    gaps, init_pack_targeted, init_pack_targeted_dry_run, pack, project_conformance_file,
    project_conformance_report, project_prompt_output_validation_file, project_run_files,
    project_source_file, prospect_brief_with_context, refresh_readme, render_human_brief_file,
    render_human_brief_markdown, render_mermaid, render_readable_prospect_brief, requirements,
    route_scoped, run_receipt, run_request_file, sample_leads, schema, skills,
    validate_behavioral_files, validate_pack, validate_prompt_output_file_with_inputs,
    validate_source_binding_file, verify_output_file, verify_output_readable_file,
    verify_run_files,
};
use crate::output::print_output;
use crate::pack_io::{planned_json_write, write_json_file};
use crate::run_replay::{
    LOCAL_LEDGER_DURABILITY_LIMITATION, ReplayConsumeRequest, compare_and_consume,
};
use anyhow::{Result, anyhow};
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
        Commands::Conformance { command } => match command {
            ConformanceCommand::Compile {
                candidate,
                artifact_root,
                out,
                dry_run,
            } => emit_conformance_output(
                json_mode,
                summary_mode,
                "conformance-compile",
                compile_candidate_file(&candidate, &artifact_root)?,
                out.as_deref(),
                dry_run,
            ),
            ConformanceCommand::Validate {
                artifact_root,
                candidate,
                deterministic,
                evaluator_inventory,
                lifecycle_policy,
                invocation,
                trial,
                evaluator_result,
                publication_approval,
                verifier_receipt,
                out,
                dry_run,
            } => emit_conformance_output(
                json_mode,
                summary_mode,
                "conformance-validate",
                validate_behavioral_files(BehavioralEvidencePaths {
                    artifact_root: &artifact_root,
                    candidate: &candidate,
                    deterministic: &deterministic,
                    evaluator_inventory: &evaluator_inventory,
                    lifecycle_policy: &lifecycle_policy,
                    invocations: &invocation,
                    trials: &trial,
                    evaluator_results: &evaluator_result,
                    publication_approvals: &publication_approval,
                    verifier_receipts: &verifier_receipt,
                })?,
                out.as_deref(),
                dry_run,
            ),
            ConformanceCommand::Assemble {
                candidate,
                deterministic,
                behavioral,
                trial,
                artifact_root,
                out,
                dry_run,
            } => emit_conformance_output(
                json_mode,
                summary_mode,
                "conformance-assemble",
                assemble_conformance(AssembleConformancePaths {
                    candidate: &candidate,
                    deterministic: &deterministic,
                    behavioral: &behavioral,
                    trials: &trial,
                    artifact_root: &artifact_root,
                })?,
                out.as_deref(),
                dry_run,
            ),
            ConformanceCommand::Report {
                conformance,
                artifact_root,
                visibility,
                generated_at,
                out,
                dry_run,
            } => emit_conformance_output(
                json_mode,
                summary_mode,
                "conformance-report",
                project_conformance_report(
                    &conformance,
                    &artifact_root,
                    &generated_at,
                    matches!(visibility, ConformanceReportVisibility::Public),
                )?,
                out.as_deref(),
                dry_run,
            ),
        },
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
        Commands::Requirements { dir, job } => print_checked(
            json_mode,
            summary_mode,
            "requirements",
            requirements(&dir, &job)?,
        ),
        Commands::ValidateSourceBinding { dir, job, file } => print_checked(
            json_mode,
            summary_mode,
            "validate-source-binding",
            validate_source_binding_file(&dir, &job, &file)?,
        ),
        Commands::Validate { dir, strict } => {
            let data = apply_strict(validate_pack(&dir)?, strict, StrictWarningSource::Issues);
            print_checked(json_mode, summary_mode, "validate", data)
        }
        Commands::Readme { command } => match command {
            ReadmeCommand::Check { dir } => {
                print_checked(json_mode, summary_mode, "readme-check", check_readme(&dir)?)
            }
            ReadmeCommand::Refresh { dir, out, dry_run } => {
                let data = refresh_readme(&dir, out.as_deref(), dry_run)?;
                print_output(json_mode, summary_mode, "readme-refresh", data)
            }
        },
        Commands::ValidatePromptOutput {
            dir,
            file,
            source_audit,
            source_binding,
            source_attempt_request,
            collected_attempt_results,
            invocation_receipt,
            routed_context,
            prompt,
            prompt_id,
            strict,
        } => {
            let validation = if source_binding.is_some() {
                validate_prompt_output_file_with_lineage_inputs(
                    &dir,
                    &file,
                    prompt.as_deref(),
                    prompt_id.as_deref(),
                    source_audit.as_deref(),
                    source_binding.as_deref(),
                    source_attempt_request.as_deref(),
                    collected_attempt_results.as_deref(),
                    invocation_receipt.as_deref(),
                    routed_context.as_deref(),
                )?
            } else {
                validate_prompt_output_file_with_inputs(
                    &dir,
                    &file,
                    prompt.as_deref(),
                    prompt_id.as_deref(),
                    source_audit.as_deref(),
                    source_attempt_request.as_deref(),
                    collected_attempt_results.as_deref(),
                    invocation_receipt.as_deref(),
                    routed_context.as_deref(),
                )?
            };
            let data = apply_strict(validation, strict, StrictWarningSource::Issues);
            print_checked(json_mode, summary_mode, "validate-prompt-output", data)
        }
        Commands::RunReceipt {
            dir,
            workflow,
            isolation,
            declared_inputs_only,
            prompt_id,
            prompt_output,
            validation,
            source_audit,
            runner_audit,
            require_runner_audit,
            artifacts,
            out,
            dry_run,
        } => {
            let mut data = run_receipt(RunReceiptOptions {
                root: &dir,
                workflow,
                isolation,
                declared_inputs_only,
                prompt_id: prompt_id.as_deref(),
                prompt_output: prompt_output.as_deref(),
                validation: validation.as_deref(),
                source_audit: source_audit.as_deref(),
                runner_audit: runner_audit.as_deref(),
                require_runner_audit,
                artifacts: &artifacts,
            })?;
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
            print_checked(json_mode, summary_mode, "run-receipt", data)
        }
        Commands::VerifyRun {
            bundle,
            receipt,
            artifact_root,
        } => print_checked(
            json_mode,
            summary_mode,
            "verify-run",
            verify_run_files(bundle.as_deref(), &receipt, artifact_root.as_deref())?,
        ),
        Commands::Trace {
            file,
            dir,
            prompt_output,
            validation_inputs,
            bundle,
            receipt,
            artifact_root,
            format,
            out,
        } => {
            let trace = match (file.as_deref(), bundle.as_deref(), receipt.as_deref()) {
                (Some(path), None, None) => match (
                    artifact_root.as_deref(),
                    dir.as_deref(),
                    prompt_output.as_deref(),
                ) {
                    (Some(root), None, None) => project_conformance_file(path, root)?,
                    (None, Some(root), Some(output)) => project_prompt_output_validation_file(
                        path,
                        root,
                        output,
                        &validation_inputs,
                    )?,
                    (None, None, None) => project_source_file(path)?,
                    _ => unreachable!("clap validates trace authority bindings"),
                },
                (None, Some(bundle), Some(receipt)) => {
                    project_run_files(bundle, receipt, artifact_root.as_deref())?
                }
                _ => unreachable!("clap validates trace source arguments"),
            };
            let data = serde_json::to_value(&trace)?;
            if format == TraceFormat::Mermaid {
                let mermaid = render_mermaid(&trace);
                if let Some(path) = out.as_deref() {
                    fs::write(path, &mermaid)?;
                }
                if !json_mode && !summary_mode {
                    println!("{mermaid}");
                    Ok(())
                } else {
                    print_output(json_mode, summary_mode, "trace", data)
                }
            } else {
                if let Some(path) = out.as_deref() {
                    write_json_file(path, &data)?;
                }
                print_output(json_mode, summary_mode, "trace", data)
            }
        }
        Commands::ConsumeRun {
            ledger,
            job_id,
            idempotency_key,
            receipt_sha256,
            expected_prior_version,
            permit_exact_replay,
        } => {
            let outcome = compare_and_consume(
                &ledger,
                &ReplayConsumeRequest {
                    job_id,
                    idempotency_key,
                    receipt_sha256,
                    expected_prior_version,
                    permit_exact_replay,
                },
            )?;
            print_output(
                json_mode,
                summary_mode,
                "consume-run",
                json!({
                    "contract": "mdp.run-consumption-result.v1",
                    "local_reference_only": true,
                    "outcome": outcome,
                    "limitation": LOCAL_LEDGER_DURABILITY_LIMITATION
                }),
            )
        }
        Commands::Run { request, out_dir } => print_run_execution(
            json_mode,
            summary_mode,
            run_request_file(&request, &out_dir)?,
        ),
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
        Commands::AuthorProofOutput {
            dir,
            draft,
            out,
            dry_run,
        } => {
            let mut data = author_proof_output_file(&dir, &draft)?;
            data = attach_input_artifact(data, "proof-output-draft", &draft);
            if let Some(path) = out {
                if data["valid"].as_bool() == Some(true) {
                    if dry_run {
                        data = attach_dry_run_artifact(data, &path);
                    } else {
                        write_json_file(&path, &data["proof_output"])?;
                        data = attach_artifact(data, &path);
                    }
                } else {
                    let reason = if data["checked"]["verification_ran"].as_bool() == Some(true) {
                        "verification-failed"
                    } else {
                        "draft-invalid"
                    };
                    data = attach_skipped_artifact(data, &path, reason);
                }
            } else {
                data = attach_stdout_artifact(data);
            }
            print_checked(json_mode, summary_mode, "author-proof-output", data)
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
        Commands::Fit {
            dir,
            prospect,
            normalized_input,
            prompt,
            source_binding,
            source_attempt_request,
            collected_attempt_results,
            job,
        } => {
            let data = if let Some(normalized) = normalized_input {
                fit_normalized(
                    &dir,
                    &normalized,
                    prompt
                        .as_deref()
                        .ok_or_else(|| anyhow!("--prompt is required with --normalized-input"))?,
                    source_binding.as_deref().ok_or_else(|| {
                        anyhow!("--source-binding is required with --normalized-input")
                    })?,
                    source_attempt_request.as_deref().ok_or_else(|| {
                        anyhow!("--source-attempt-request is required with --normalized-input")
                    })?,
                    collected_attempt_results.as_deref().ok_or_else(|| {
                        anyhow!("--collected-attempt-results is required with --normalized-input")
                    })?,
                    job.as_deref(),
                )?
            } else {
                fit_for_job(
                    &dir,
                    prospect
                        .as_deref()
                        .ok_or_else(|| anyhow!("--prospect is required"))?,
                    job.as_deref(),
                )?
            };
            print_checked(json_mode, summary_mode, "fit", data)
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
            normalized_input,
            prompt,
            source_binding,
            source_attempt_request,
            collected_attempt_results,
            channel,
            job,
            context,
            routed_context_out,
            readable,
            out,
            dry_run,
        } => {
            ensure_distinct_output_paths(out.as_deref(), routed_context_out.as_deref())?;
            let include_context = context
                || routed_context_out.is_some()
                || (readable && !json_mode && !summary_mode);
            let (mut data, input_kind, input_path) = if let Some(normalized) = normalized_input {
                let fit_result = fit_normalized(
                    &dir,
                    &normalized,
                    prompt
                        .as_deref()
                        .ok_or_else(|| anyhow!("--prompt is required with --normalized-input"))?,
                    source_binding.as_deref().ok_or_else(|| {
                        anyhow!("--source-binding is required with --normalized-input")
                    })?,
                    source_attempt_request.as_deref().ok_or_else(|| {
                        anyhow!("--source-attempt-request is required with --normalized-input")
                    })?,
                    collected_attempt_results.as_deref().ok_or_else(|| {
                        anyhow!("--collected-attempt-results is required with --normalized-input")
                    })?,
                    job.as_deref(),
                )?;
                let projected = serde_json::from_value(fit_result["prospect"].clone())?;
                (
                    prospect_brief_from_fit_with_context(
                        &dir,
                        projected,
                        fit_result,
                        &channel,
                        job.as_deref(),
                        include_context,
                    )?,
                    "normalized-decision-input",
                    normalized,
                )
            } else {
                let prospect = prospect.ok_or_else(|| anyhow!("--prospect is required"))?;
                (
                    prospect_brief_with_context(
                        &dir,
                        &prospect,
                        &channel,
                        job.as_deref(),
                        include_context,
                    )?,
                    "prospect",
                    prospect,
                )
            };
            data = attach_input_artifact(data, input_kind, &input_path);
            if let Some(path) = routed_context_out {
                data = export_routed_context(data, &path, dry_run)?;
            }
            if readable && !json_mode && !summary_mode {
                let valid = data["valid"].as_bool().unwrap_or(false);
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
                if valid {
                    return Ok(());
                }
                std::process::exit(1);
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
            print_checked(json_mode, summary_mode, "brief", data)
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
            routed_context_out,
            out,
            dry_run,
        } => {
            ensure_distinct_output_paths(out.as_deref(), routed_context_out.as_deref())?;
            let mut data =
                emit_brief_scoped(&dir, &persona, motion.as_deref(), job.as_deref(), &scope)?;
            if let Some(path) = routed_context_out {
                data = export_routed_context(data, &path, dry_run)?;
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
    // Most checked contracts carry an explicit `valid` gate. The typed composite
    // and report projections predate that field and are validated before they
    // reach this boundary; preserve their established successful process exit
    // without treating arbitrary missing gates as success.
    let valid = match data.get("valid") {
        Some(value) => value.as_bool().unwrap_or(false),
        None => matches!(
            (command, data.get("contract").and_then(Value::as_str)),
            ("conformance-assemble", Some("mdp.job-conformance.v1"))
                | ("conformance-report", Some("mdp.conformance-report.v1"))
                | (
                    "conformance-report",
                    Some("mdp.public-conformance-report.v1")
                )
        ),
    };
    print_output(json_mode, summary_mode, command, data)?;
    if valid {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn print_run_execution(json_mode: bool, summary_mode: bool, data: Value) -> Result<()> {
    // A verified no-draft decision is a completed run, not a transport failure.
    // Preserve the v1 process contract while the authority block carries the
    // machine-readable prohibition on governed generation.
    let completed_decision = data["terminal_state"] == "success"
        && matches!(
            data["authority"]["disposition"].as_str(),
            Some("allow" | "block")
        );
    if completed_decision {
        print_output(json_mode, summary_mode, "run", data)
    } else {
        print_checked(json_mode, summary_mode, "run", data)
    }
}

fn emit_conformance_output(
    json_mode: bool,
    summary_mode: bool,
    command: &str,
    mut data: Value,
    out: Option<&Path>,
    dry_run: bool,
) -> Result<()> {
    if let Some(path) = out {
        if fs::symlink_metadata(path).is_ok() {
            return Err(anyhow!(
                "conformance output already exists; refusing to overwrite"
            ));
        }
        if dry_run {
            data = prepare_conformance_dry_run(data, path)?;
        } else {
            write_json_file(path, &data)?;
        }
    } else if dry_run {
        return Err(anyhow!("--dry-run requires --out"));
    }
    print_checked(json_mode, summary_mode, command, data)
}

fn prepare_conformance_dry_run(data: Value, path: &Path) -> Result<Value> {
    let plan = planned_json_write(path);
    if plan["would_write"] != true {
        return Err(anyhow!("conformance output path is not writable"));
    }
    Ok(attach_dry_run_artifact(data, path))
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

fn export_routed_context(mut data: Value, path: &Path, dry_run: bool) -> Result<Value> {
    if data
        .pointer("/context/minimality/status")
        .and_then(Value::as_str)
        != Some("ready")
    {
        return Err(anyhow!(
            "routed context is unavailable because minimality is not ready"
        ));
    }
    let context = data
        .pointer("/context/model_context")
        .filter(|value| !value.is_null())
        .ok_or_else(|| anyhow!("routed context is unavailable because minimality is not ready"))?;
    let bytes = canonical_json_bytes(context)?;
    let status = if dry_run {
        "dry-run"
    } else {
        fs::write(path, &bytes)?;
        "saved"
    };
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "routed_context_artifact".to_string(),
            json!({
                "status": status,
                "kind": "canonical-json-file",
                "path": path.display().to_string(),
                "sha256": sha256_hex(&bytes),
                "bytes": bytes.len()
            }),
        );
    }
    Ok(data)
}

fn ensure_distinct_output_paths(
    out: Option<&Path>,
    routed_context_out: Option<&Path>,
) -> Result<()> {
    let (Some(out), Some(routed_context_out)) = (out, routed_context_out) else {
        return Ok(());
    };
    let resolved_out = resolve_output_path(out)?;
    let resolved_routed_context_out = resolve_output_path(routed_context_out)?;
    if resolved_out == resolved_routed_context_out
        || existing_paths_have_same_identity(out, routed_context_out)?
    {
        return Err(anyhow!(
            "--out and --routed-context-out must resolve to different paths"
        ));
    }
    Ok(())
}

fn resolve_output_path(path: &Path) -> Result<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Ok(canonical);
    }
    let mut candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    // `canonicalize` cannot resolve a dangling symlink. Follow the output leaf
    // explicitly so aliases still compare equal before either artifact is written.
    for depth in 0..40 {
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if depth == 39 {
                    return Err(anyhow!("output path has too many symbolic links"));
                }
                let target = fs::read_link(&candidate)?;
                candidate = if target.is_absolute() {
                    target
                } else {
                    candidate
                        .parent()
                        .unwrap_or_else(|| Path::new(""))
                        .join(target)
                };
            }
            Ok(_) => return Ok(candidate.canonicalize()?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }

    let Some(file_name) = candidate.file_name() else {
        return Ok(candidate);
    };
    let Some(parent) = candidate.parent() else {
        return Ok(candidate);
    };
    Ok(parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf())
        .join(file_name))
}

#[cfg(unix)]
fn existing_paths_have_same_identity(left: &Path, right: &Path) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;

    let (Some(left), Some(right)) = (existing_metadata(left)?, existing_metadata(right)?) else {
        return Ok(false);
    };
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

#[cfg(windows)]
fn existing_paths_have_same_identity(left: &Path, right: &Path) -> Result<bool> {
    use std::os::windows::fs::MetadataExt;

    let (Some(left), Some(right)) = (existing_metadata(left)?, existing_metadata(right)?) else {
        return Ok(false);
    };
    Ok(matches!(
        (
            left.volume_serial_number(),
            left.file_index(),
            right.volume_serial_number(),
            right.file_index(),
        ),
        (Some(left_volume), Some(left_index), Some(right_volume), Some(right_index))
            if left_volume == right_volume && left_index == right_index
    ))
}

#[cfg(not(any(unix, windows)))]
fn existing_paths_have_same_identity(_left: &Path, _right: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(any(unix, windows))]
fn existing_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
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

fn attach_skipped_artifact(mut data: Value, path: &Path, reason: &str) -> Value {
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "artifact".to_string(),
            json!({
                "status": "skipped",
                "kind": "json-file",
                "path": path.display().to_string(),
                "reason": reason
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

fn attach_input_artifact(mut data: Value, kind: &str, path: &Path) -> Value {
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
        let routed_context_out = root.join(".mdp").join("briefs").join("routed-context.json");

        run(Cli {
            json: true,
            summary: true,
            command: Commands::Brief {
                dir: root.clone(),
                prospect: Some(prospect),
                normalized_input: None,
                prompt: None,
                source_binding: None,
                source_attempt_request: None,
                collected_attempt_results: None,
                channel: "linkedin".to_string(),
                job: Some("outbound-copy-brief".to_string()),
                context: true,
                routed_context_out: Some(routed_context_out.clone()),
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
        assert_eq!(saved["routed_context_artifact"]["status"], "saved");
        let routed_bytes = std::fs::read(&routed_context_out).expect("routed context should exist");
        assert_eq!(
            routed_bytes,
            canonical_json_bytes(&saved["context"]["model_context"])
                .expect("routed context should serialize canonically")
        );
        assert!(!routed_bytes.ends_with(b"\n"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_rejects_aliased_output_paths_without_overwriting_routed_context() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-brief-aliased-out-{nonce}"));
        init_pack(&root, "Brief Aliased Out Pack", "gtm", true, false)
            .expect("pack should initialize");
        let prospect = root.join("examples").join("clay-row.json");
        let routed_context_out = root.join(".mdp").join("briefs").join("routed-context.json");

        run(Cli {
            json: true,
            summary: true,
            command: Commands::Brief {
                dir: root.clone(),
                prospect: Some(prospect.clone()),
                normalized_input: None,
                prompt: None,
                source_binding: None,
                source_attempt_request: None,
                collected_attempt_results: None,
                channel: "linkedin".to_string(),
                job: Some("outbound-copy-brief".to_string()),
                context: true,
                routed_context_out: Some(routed_context_out.clone()),
                readable: false,
                out: None,
                dry_run: false,
            },
        })
        .expect("routed context should export");
        let canonical_bytes = std::fs::read(&routed_context_out)
            .expect("canonical routed context should be readable");
        let aliased_out = routed_context_out
            .parent()
            .expect("routed context should have a parent")
            .join(".")
            .join("routed-context.json");

        let error = run(Cli {
            json: true,
            summary: true,
            command: Commands::Brief {
                dir: root.clone(),
                prospect: Some(prospect),
                normalized_input: None,
                prompt: None,
                source_binding: None,
                source_attempt_request: None,
                collected_attempt_results: None,
                channel: "linkedin".to_string(),
                job: Some("outbound-copy-brief".to_string()),
                context: true,
                routed_context_out: Some(routed_context_out.clone()),
                readable: false,
                out: Some(aliased_out),
                dry_run: false,
            },
        })
        .expect_err("aliased outputs should be rejected");

        assert!(
            error
                .to_string()
                .contains("--out and --routed-context-out must resolve to different paths")
        );
        assert_eq!(
            std::fs::read(&routed_context_out)
                .expect("routed context should remain readable after rejection"),
            canonical_bytes
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn brief_rejects_dangling_symlink_alias_without_writing_target() {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-brief-dangling-alias-{nonce}"));
        init_pack(&root, "Brief Dangling Alias Pack", "gtm", true, false)
            .expect("pack should initialize");
        let prospect = root.join("examples").join("clay-row.json");
        let output_dir = root.join(".mdp").join("briefs");
        let out = output_dir.join("brief.json");
        let routed_context_out = output_dir.join("routed-context.json");
        symlink("routed-context.json", &out).expect("dangling output alias should be created");

        let error = run(Cli {
            json: true,
            summary: true,
            command: Commands::Brief {
                dir: root.clone(),
                prospect: Some(prospect),
                normalized_input: None,
                prompt: None,
                source_binding: None,
                source_attempt_request: None,
                collected_attempt_results: None,
                channel: "linkedin".to_string(),
                job: Some("outbound-copy-brief".to_string()),
                context: true,
                routed_context_out: Some(routed_context_out.clone()),
                readable: false,
                out: Some(out.clone()),
                dry_run: false,
            },
        })
        .expect_err("dangling symlink aliases should be rejected");

        assert!(
            error
                .to_string()
                .contains("--out and --routed-context-out must resolve to different paths")
        );
        assert!(
            !routed_context_out.exists(),
            "routed context target should not be written"
        );
        assert_eq!(
            std::fs::read_link(&out).expect("output should remain a symlink"),
            PathBuf::from("routed-context.json")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn emit_brief_rejects_hard_link_aliases_without_overwriting() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-emit-brief-hard-link-{nonce}"));
        init_pack(&root, "Emit Brief Hard Link Pack", "gtm", true, false)
            .expect("pack should initialize");
        let output_dir = root.join(".mdp").join("briefs");
        let out = output_dir.join("brief.json");
        let routed_context_out = output_dir.join("routed-context.json");
        let original = b"existing artifact";
        std::fs::write(&out, original).expect("fixture artifact should write");
        std::fs::hard_link(&out, &routed_context_out).expect("hard-link alias should be created");

        let error = run(Cli {
            json: true,
            summary: true,
            command: Commands::EmitBrief {
                dir: root.clone(),
                persona: "Growth Engineer".to_string(),
                motion: None,
                job: Some("outbound-copy-brief".to_string()),
                scope: Vec::new(),
                routed_context_out: Some(routed_context_out.clone()),
                out: Some(out.clone()),
                dry_run: false,
            },
        })
        .expect_err("hard-link aliases should be rejected");

        assert!(
            error
                .to_string()
                .contains("--out and --routed-context-out must resolve to different paths")
        );
        assert_eq!(
            std::fs::read(&out).expect("output should remain readable"),
            original
        );
        assert_eq!(
            std::fs::read(&routed_context_out)
                .expect("routed context alias should remain readable"),
            original
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn emit_brief_dry_run_rejects_aliased_output_paths() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-emit-brief-aliased-out-{nonce}"));
        init_pack(&root, "Emit Brief Aliased Out Pack", "gtm", true, false)
            .expect("pack should initialize");
        let out = root.join(".mdp").join("briefs").join("brief.json");
        std::fs::write(&out, b"existing artifact").expect("fixture artifact should write");
        let aliased_routed_context_out = out
            .parent()
            .expect("brief should have a parent")
            .join(".")
            .join("brief.json");

        let error = run(Cli {
            json: true,
            summary: true,
            command: Commands::EmitBrief {
                dir: root.clone(),
                persona: "Growth Engineer".to_string(),
                motion: None,
                job: Some("outbound-copy-brief".to_string()),
                scope: Vec::new(),
                routed_context_out: Some(aliased_routed_context_out),
                out: Some(out.clone()),
                dry_run: true,
            },
        })
        .expect_err("aliased dry-run outputs should be rejected");

        assert!(
            error
                .to_string()
                .contains("--out and --routed-context-out must resolve to different paths")
        );
        assert_eq!(
            std::fs::read(&out).expect("fixture artifact should remain readable"),
            b"existing artifact"
        );

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
                prospect: Some(prospect),
                normalized_input: None,
                prompt: None,
                source_binding: None,
                source_attempt_request: None,
                collected_attempt_results: None,
                channel: "linkedin".to_string(),
                job: None,
                context: false,
                routed_context_out: None,
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

    #[test]
    fn conformance_dry_run_output_exposes_planned_artifact() {
        let path = std::env::temp_dir().join("conformance.json");
        let result = prepare_conformance_dry_run(
            json!({"contract": "mdp.deterministic-conformance.v1"}),
            &path,
        )
        .unwrap();
        assert_eq!(result["artifact"]["status"], "dry-run");
        assert_eq!(result["dry_run"], true);
        assert_eq!(result["write_plan"][0]["path"], path.display().to_string());
        assert_eq!(result["write_plan"][0]["would_write"], true);
    }
}
