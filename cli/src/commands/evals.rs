use crate::commands::routing::route;
use crate::constants::DEFAULT_DIR;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(crate) fn eval_pack(root: &Path) -> Result<Value> {
    let eval_dir = root.join(DEFAULT_DIR).join("evals");
    if !eval_dir.exists() {
        return Ok(json!({
            "contract": "mdp.eval.v0",
            "valid": true,
            "fixtures": [],
            "issues": [],
            "note": "No .mdp/evals directory found. Add YAML fixtures to make route behavior testable."
        }));
    }
    let mut fixtures = Vec::new();
    let mut issues = Vec::new();
    for entry in
        fs::read_dir(&eval_dir).with_context(|| format!("reading {}", eval_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let fixture: Value =
            serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        let persona = fixture["persona"].as_str().unwrap_or("GTM Engineering");
        let job = fixture["job"].as_str().unwrap_or("linkedin outbound copy");
        let result = route(root, persona, job, false)?;
        let load_order = result["load_order"].as_array().cloned().unwrap_or_default();
        let expected = fixture["expect_load_order_contains"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let missing: Vec<Value> = expected
            .into_iter()
            .filter(|expected_path| !load_order.iter().any(|actual| actual == expected_path))
            .collect();
        if !missing.is_empty() {
            issues.push(json!({"fixture": path.display().to_string(), "missing": missing}));
        }
        fixtures.push(json!({"path": path.display().to_string(), "persona": persona, "job": job, "load_order": load_order}));
    }
    Ok(
        json!({"contract": "mdp.eval.v0", "valid": issues.is_empty(), "fixtures": fixtures, "issues": issues}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true)
            .expect("starter pack should initialize");
        root
    }

    #[test]
    fn eval_reports_missing_expected_route_paths() {
        let root = temp_pack("eval-contract");
        let fixture_path = root.join(".mdp").join("evals").join("missing-card.yaml");
        std::fs::write(
            &fixture_path,
            r#"id: missing-card
persona: PMM
job: linkedin outbound copy
expect_load_order_contains:
  - .mdp/cards/not-real.yaml
"#,
        )
        .expect("fixture should be writable");

        let result = eval_pack(&root).expect("eval should succeed");

        assert_eq!(result["contract"], "mdp.eval.v0");
        assert_eq!(result["valid"], false);
        assert_eq!(
            result["issues"][0]["missing"][0],
            ".mdp/cards/not-real.yaml"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
