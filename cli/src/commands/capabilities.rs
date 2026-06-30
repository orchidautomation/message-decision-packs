use crate::constants::{
    DEFAULT_DIR, FORMAT_VERSION, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_VERSION,
    PROMPT_OUTPUT_CONTRACT, PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
};
use serde_json::{Value, json};

pub(crate) fn capabilities() -> Value {
    json!({
        "contract": "mdp.capabilities.v0",
        "tool": "mdp",
        "format_version": FORMAT_VERSION,
        "defaults": {
            "pack_dir": DEFAULT_DIR,
            "offline_by_default": true,
            "auth_required": false
        },
        "global_options": [
            {"name": "--json", "description": "Emit stable machine-readable JSON"},
            {"name": "--summary", "description": "Emit a compact status summary"}
        ],
        "prompt_contracts": {
            "prompt_format": PROMPT_FORMAT_VERSION,
            "prompt_output": PROMPT_OUTPUT_CONTRACT,
            "card_patch_schema_ref": PROMPT_CARD_PATCH_SCHEMA_REF,
            "prospect_normalization_schema_ref": PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF
        },
        "commands": [
            command("capabilities", "mdp.capabilities.v0", "read-only", false, false, false, &[]),
            command("init", "mdp.init.v0", "writes-files", true, false, false, &["--name", "--dir", "--template", "--force", "--include-output-schemas", "--dry-run"]),
            command("doctor", "mdp.doctor.v0", "read-only", false, false, false, &["--dir"]),
            command("validate", "mdp.validate.v0", "read-only", false, false, true, &["--dir", "--strict"]),
            command("validate-prompt-output", "mdp.validate-prompt-output.v0", "read-only", false, false, true, &["--dir", "--file", "--prompt", "--prompt-id", "--strict"]),
            command("explain", "mdp.explain.v0", "read-only", false, false, false, &["--dir", "--persona"]),
            command("route", "mdp.route.v0", "read-only", false, false, false, &["--dir", "--persona", "--job", "--entries", "--eval-fixture"]),
            command("sample-leads", "mdp.sample-leads.v0", "read-only", false, false, false, &["--dir", "--persona", "--job", "--count", "--seed", "--format"]),
            command("fit", "mdp.fit.v0", "read-only", false, false, false, &["--dir", "--prospect"]),
            command("check-claims", "mdp.claim-check.v0", "read-only", false, false, true, &["--dir", "--text", "--file", "--subject", "--persona", "--job", "--strict"]),
            command("gaps", "mdp.gaps.v0", "read-only", false, false, false, &["--dir"]),
            command("eval", "mdp.eval.v0", "read-only", false, false, true, &["--dir", "--strict"]),
            command("brief", "mdp.message-brief.v0", "writes-files-with-out", true, true, false, &["--dir", "--prospect", "--channel", "--job", "--context", "--out", "--dry-run"]),
            command("copy", "mdp.copy-demo.v0", "writes-files-with-out", false, true, false, &["--dir", "--prospect", "--channel", "--out"]),
            command("emit-brief", "mdp.brief.v0", "writes-files-with-out", true, true, false, &["--dir", "--persona", "--motion", "--job", "--out", "--dry-run"]),
            command("pack", "mdp.pack.v0", "writes-files-with-out", true, true, false, &["--dir", "--out", "--dry-run"]),
            command("schema", "mdp.schema.v0", "read-only", false, false, false, &["target"])
        ],
        "stable_error_codes": [
            {"code": "pack_not_found", "meaning": "A pack manifest or required .mdp path could not be read"},
            {"code": "invalid_manifest", "meaning": "A pack manifest could not be parsed or uses invalid structure"},
            {"code": "missing_card", "meaning": "A referenced card could not be found or read"},
            {"code": "unsupported_claim", "meaning": "Draft text contains unsupported claims or claim-check failures"},
            {"code": "insufficient_context", "meaning": "A fit or drafting path lacks enough context to proceed"},
            {"code": "write_conflict", "meaning": "A write would overwrite an existing file without explicit permission"},
            {"code": "invalid_argument", "meaning": "CLI arguments are missing, conflicting, or unsupported"},
            {"code": "mdp_error", "meaning": "Fallback for uncategorized MDP errors"}
        ],
        "boundaries": [
            "No auth, hosted API, scraping, enrichment, CRM writeback, sending, sequencing, or BI behavior.",
            "Dry-run reports local file writes only; it does not perform network calls or mutate packs.",
            "Strict mode is opt-in and preserves default compatibility."
        ]
    })
}

fn command(
    name: &str,
    output_contract: &str,
    side_effects: &str,
    dry_run: bool,
    out: bool,
    strict: bool,
    args: &[&str],
) -> Value {
    json!({
        "name": name,
        "output_contract": output_contract,
        "side_effects": side_effects,
        "supports_json": true,
        "supports_summary": true,
        "supports_out": out,
        "supports_dry_run": dry_run,
        "supports_strict": strict,
        "args": args
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_exposes_agent_driving_contracts() {
        let result = capabilities();
        assert_eq!(result["contract"], "mdp.capabilities.v0");
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "capabilities")
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "init" && command["supports_dry_run"] == true)
        );
        assert!(
            result["stable_error_codes"]
                .as_array()
                .expect("error codes array")
                .iter()
                .any(|code| code["code"] == "write_conflict")
        );
    }
}
