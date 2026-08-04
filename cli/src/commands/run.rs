use crate::artifact_hash::{AuthorityJsonLimits, parse_authority_json};
use crate::run_contracts::RunRequestV1;
use crate::run_runtime::execute_run;
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub(crate) fn run_request_file(request_path: &Path, output_root: &Path) -> Result<Value> {
    let bytes = fs::read(request_path)
        .with_context(|| format!("reading run request {}", request_path.display()))?;
    let request: RunRequestV1 = parse_authority_json(&bytes, AuthorityJsonLimits::default())?;
    Ok(serde_json::to_value(execute_run(&request, output_root)?)?)
}
