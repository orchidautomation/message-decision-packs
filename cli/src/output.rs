use anyhow::Result;
use serde_json::{Value, json};

pub(crate) fn print_output(json_mode: bool, command: &str, data: Value) -> Result<()> {
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"ok": true, "command": command, "data": data}))?
        );
    } else {
        print_human(command, &data)?;
    }
    Ok(())
}

pub(crate) fn print_error(json_mode: bool, err: anyhow::Error) -> Result<()> {
    let payload = json!({"ok": false, "error": {"code": "mdp_error", "message": err.to_string(), "details": []}});
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        eprintln!("error: {}", err);
    }
    Ok(())
}

fn print_human(command: &str, data: &Value) -> Result<()> {
    match command {
        "init" => {
            println!(
                "Created MDP package at {}",
                data["pack_dir"].as_str().unwrap_or("")
            );
            println!(
                "Next: mdp validate --dir {}",
                data["root"].as_str().unwrap_or(".")
            );
        }
        "doctor" | "validate" => {
            println!(
                "{}: {}",
                command,
                if data["valid"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "needs attention"
                }
            );
            if let Some(items) = data["issues"].as_array() {
                for item in items {
                    println!("- {}", issue_message(item));
                }
            }
        }
        _ => println!("{}", serde_json::to_string_pretty(data)?),
    }
    Ok(())
}

fn issue_message(item: &Value) -> String {
    if let Some(message) = item.as_str() {
        return message.to_string();
    }
    let code = item["code"].as_str().unwrap_or("issue");
    let path = item["path"].as_str().unwrap_or("");
    let message = item["message"].as_str().unwrap_or("");
    if path.is_empty() {
        format!("{code}: {message}")
    } else {
        format!("{code} at {path}: {message}")
    }
}
