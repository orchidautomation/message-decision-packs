use crate::value_contracts::{valid_date, valid_date_time};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_CONTEXT_CONTRACT: &str = "mdp.runtime-context.v0";
const LOCAL_TIME_POLICY: &str = "MDP emits UTC runtime metadata only. If a workflow needs fiscal year, renewal date, event date, campaign window, or local business calendar logic, pass that as reviewed pack-declared metadata or supplied source context.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeContextViolation {
    pub(crate) code: &'static str,
    pub(crate) path: String,
    pub(crate) reason: String,
}

pub(crate) fn current_runtime_context() -> Result<Value> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| anyhow!("system clock is before unix epoch"))?;
    Ok(runtime_context_from_unix_seconds(elapsed.as_secs()))
}

pub(crate) fn runtime_context_from_unix_seconds(seconds: u64) -> Value {
    let (year, month, day, hour, minute, second) = utc_parts_from_unix_seconds(seconds);
    let date_utc = format!("{year:04}-{month:02}-{day:02}");
    let now_utc = format!("{date_utc}T{hour:02}:{minute:02}:{second:02}Z");
    json!({
        "contract": RUNTIME_CONTEXT_CONTRACT,
        "now_utc": now_utc,
        "date_utc": date_utc,
        "timezone": "UTC",
        "local_time_policy": LOCAL_TIME_POLICY
    })
}

pub(crate) fn runtime_context_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Runtime Context v0",
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "now_utc", "date_utc", "timezone", "local_time_policy"],
        "properties": {
            "contract": {"const": RUNTIME_CONTEXT_CONTRACT},
            "now_utc": {
                "type": "string",
                "format": "date-time",
                "description": "Current UTC run timestamp emitted by the CLI."
            },
            "date_utc": {
                "type": "string",
                "format": "date",
                "description": "UTC calendar date derived from now_utc."
            },
            "timezone": {
                "const": "UTC",
                "description": "The CLI does not infer operator-local timezone."
            },
            "local_time_policy": {
                "type": "string",
                "description": "Policy for local calendars and pack-declared timing attributes."
            }
        }
    })
}

pub(crate) fn validate_runtime_context(value: &Value, path: &str) -> Vec<RuntimeContextViolation> {
    let mut violations = Vec::new();
    let Some(context) = value.as_object() else {
        violations.push(RuntimeContextViolation {
            code: "runtime_context_type",
            path: path.to_string(),
            reason: "runtime_context must be an object".to_string(),
        });
        return violations;
    };

    for key in context.keys() {
        if !matches!(
            key.as_str(),
            "contract" | "now_utc" | "date_utc" | "timezone" | "local_time_policy"
        ) {
            violations.push(RuntimeContextViolation {
                code: "runtime_context_unknown_field",
                path: format!("{path}/{key}"),
                reason: format!("runtime_context contains unsupported field {key}"),
            });
        }
    }

    validate_string_field(
        context,
        "contract",
        path,
        "runtime_context_contract",
        &mut violations,
        |value| value == RUNTIME_CONTEXT_CONTRACT,
        format!("runtime_context.contract must be {RUNTIME_CONTEXT_CONTRACT}"),
    );
    validate_string_field(
        context,
        "now_utc",
        path,
        "runtime_context_now_utc_format",
        &mut violations,
        |value| utc_date_from_date_time(value).is_some(),
        "runtime_context.now_utc must be an ISO UTC date-time such as 2026-07-02T03:45:00Z",
    );
    validate_string_field(
        context,
        "date_utc",
        path,
        "runtime_context_date_utc_format",
        &mut violations,
        valid_date,
        "runtime_context.date_utc must be an ISO date such as 2026-07-02",
    );
    validate_string_field(
        context,
        "timezone",
        path,
        "runtime_context_timezone",
        &mut violations,
        |value| value == "UTC",
        "runtime_context.timezone must be UTC",
    );
    validate_string_field(
        context,
        "local_time_policy",
        path,
        "runtime_context_local_time_policy",
        &mut violations,
        |value| !value.trim().is_empty(),
        "runtime_context.local_time_policy must explain local calendar policy",
    );
    validate_date_matches_now_utc(context, path, &mut violations);

    violations
}

fn validate_date_matches_now_utc(
    context: &serde_json::Map<String, Value>,
    path: &str,
    violations: &mut Vec<RuntimeContextViolation>,
) {
    let Some(now_utc) = context.get("now_utc").and_then(Value::as_str) else {
        return;
    };
    let Some(date_utc) = context.get("date_utc").and_then(Value::as_str) else {
        return;
    };
    let Some(now_date) = utc_date_from_date_time(now_utc) else {
        return;
    };
    if valid_date(date_utc) && now_date != date_utc {
        violations.push(RuntimeContextViolation {
            code: "runtime_context_date_utc_mismatch",
            path: format!("{path}/date_utc"),
            reason: "runtime_context.date_utc must match the UTC date portion of runtime_context.now_utc"
                .to_string(),
        });
    }
}

fn utc_date_from_date_time(value: &str) -> Option<&str> {
    let (date, rest) = value.split_once('T')?;
    rest.strip_suffix('Z')?;
    (valid_date_time(value) && valid_date(date)).then_some(date)
}

fn validate_string_field(
    context: &serde_json::Map<String, Value>,
    field: &str,
    path: &str,
    code: &'static str,
    violations: &mut Vec<RuntimeContextViolation>,
    predicate: impl Fn(&str) -> bool,
    reason: impl Into<String>,
) {
    let Some(value) = context.get(field).and_then(Value::as_str) else {
        violations.push(RuntimeContextViolation {
            code,
            path: format!("{path}/{field}"),
            reason: format!("runtime_context.{field} must be a string"),
        });
        return;
    };
    if !predicate(value) {
        violations.push(RuntimeContextViolation {
            code,
            path: format!("{path}/{field}"),
            reason: reason.into(),
        });
    }
}

fn utc_parts_from_unix_seconds(seconds: u64) -> (i64, u64, u64, u64, u64, u64) {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u64, u64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(m <= 2);
    (year, m as u64, d as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_context_formats_unix_epoch() {
        let context = runtime_context_from_unix_seconds(0);

        assert_eq!(context["now_utc"], "1970-01-01T00:00:00Z");
        assert_eq!(context["date_utc"], "1970-01-01");
        assert_eq!(context["timezone"], "UTC");
        assert!(validate_runtime_context(&context, "$").is_empty());
    }

    #[test]
    fn runtime_context_formats_known_timestamp() {
        let context = runtime_context_from_unix_seconds(1_783_035_900);

        assert_eq!(context["now_utc"], "2026-07-02T23:45:00Z");
        assert_eq!(context["date_utc"], "2026-07-02");
        assert!(validate_runtime_context(&context, "$").is_empty());
    }

    #[test]
    fn runtime_context_validation_rejects_bad_dates() {
        let context = json!({
            "contract": RUNTIME_CONTEXT_CONTRACT,
            "now_utc": "2026-13-02 13:05:00",
            "date_utc": "2026-02-30",
            "timezone": "America/New_York",
            "local_time_policy": ""
        });

        let codes = validate_runtime_context(&context, "$")
            .into_iter()
            .map(|violation| violation.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"runtime_context_now_utc_format"));
        assert!(codes.contains(&"runtime_context_date_utc_format"));
        assert!(codes.contains(&"runtime_context_timezone"));
        assert!(codes.contains(&"runtime_context_local_time_policy"));
    }

    #[test]
    fn runtime_context_validation_rejects_contradictory_dates() {
        let context = json!({
            "contract": RUNTIME_CONTEXT_CONTRACT,
            "now_utc": "2026-07-02T00:15:00Z",
            "date_utc": "2026-07-03",
            "timezone": "UTC",
            "local_time_policy": LOCAL_TIME_POLICY
        });

        let codes = validate_runtime_context(&context, "$")
            .into_iter()
            .map(|violation| violation.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"runtime_context_date_utc_mismatch"));
    }

    #[test]
    fn runtime_context_validation_rejects_offset_now_utc() {
        let context = json!({
            "contract": RUNTIME_CONTEXT_CONTRACT,
            "now_utc": "2026-07-02T00:15:00-04:00",
            "date_utc": "2026-07-02",
            "timezone": "UTC",
            "local_time_policy": LOCAL_TIME_POLICY
        });

        let codes = validate_runtime_context(&context, "$")
            .into_iter()
            .map(|violation| violation.code)
            .collect::<Vec<_>>();

        assert!(codes.contains(&"runtime_context_now_utc_format"));
    }
}
