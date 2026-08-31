use crate::commands::doctor;
use crate::constants::DEFAULT_DIR;
use crate::pack_io::read_manifest;
use serde_json::{Value, json};
use std::path::Path;

/// Observational status projection. This deliberately delegates health checks
/// to doctor so status and doctor cannot disagree about pack validity.
pub(crate) fn status(root: &Path) -> Value {
    let diagnosis = doctor(root);
    let manifest_observed = root.join(DEFAULT_DIR).join("manifest.yaml").is_file();
    let manifest = read_manifest(root).ok();
    let state = match diagnosis["status"].as_str().unwrap_or("invalid") {
        "ready" => "ready",
        "pack-missing" => "needs-input",
        "activation-blocked" => "blocked",
        _ => "invalid",
    };
    let blocker = diagnosis["issues"]
        .as_array()
        .and_then(|items| items.first())
        .cloned();
    let mut pack = serde_json::Map::new();
    if let Some(manifest) = &manifest {
        pack.insert("id".into(), json!(manifest.id));
        pack.insert("name".into(), json!(manifest.name));
        if let Some(profile) = &manifest.profile {
            pack.insert("profile_id".into(), json!(profile.id));
        }
        if let Some(target) = &manifest.target {
            if !target.name.is_empty() {
                pack.insert(
                    "target".into(),
                    json!({"kind": target.kind, "name": target.name}),
                );
            }
        }
    }
    let next_command = diagnosis["next_command"]
        .as_str()
        .unwrap_or("mdp init --name <name> --dir PACK_ROOT");
    json!({
        "contract": "mdp.status.v1",
        "cli_version": env!("CARGO_PKG_VERSION"),
        "mode": "local-offline",
        "auth_required": false,
        "requested_pack_root": root.display().to_string(),
        "manifest_observed": manifest_observed,
        "pack": pack,
        "health": {"state": state, "blocker": blocker},
        "next_command": next_command
    })
}
