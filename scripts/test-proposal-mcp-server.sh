#!/usr/bin/env bash
set -euo pipefail

root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

pack="$root/plugin/assets/templates/proposal"
workdir="$tmp_dir/dry-run"
transcript="$tmp_dir/transcript.ndjson"
stdout_jsonl="$tmp_dir/stdout.jsonl"
stderr_log="$tmp_dir/stderr.log"
mcp_result_schema="$tmp_dir/proposal-mcp-run-result.schema.json"
env_probe_runner="$tmp_dir/env-probe-runner.mjs"
slow_runner="$tmp_dir/slow-runner.mjs"
sensitive_runner="$tmp_dir/sensitive-runner.mjs"
source_symlink="$tmp_dir/source-symlink.txt"

cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema proposal-mcp-run-result > "$mcp_result_schema"

cat > "$env_probe_runner" <<'JS'
const providerKey = ['OPENAI', 'API', 'KEY'].join('_')
const snapshot = {
  unexpected_marker_present: Object.hasOwn(process.env, 'MDP_MCP_TEST_MARKER'),
  provider_key_present: Object.hasOwn(process.env, providerKey),
  keys: Object.keys(process.env).sort(),
}
process.stdout.write(`${JSON.stringify(snapshot)}\n`)
JS

cat > "$slow_runner" <<'JS'
setTimeout(() => process.stdout.write('unexpected completion\n'), 5_000)
JS

cat > "$sensitive_runner" <<'JS'
const providerKey = ['OPENAI', 'API', 'KEY'].join('_')
process.stderr.write(`provider rejected credential ${process.env[providerKey]}\n`)
process.exit(9)
JS

source_file="$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt"
ln -s "$source_file" "$source_symlink"

python3 - "$root" "$pack" "$workdir" "$transcript" "$env_probe_runner" "$slow_runner" "$sensitive_runner" "$source_symlink" <<'PY'
import json, pathlib, sys
root, pack, workdir, transcript, env_probe, slow_runner, sensitive_runner, source_symlink = sys.argv[1:]
source = str(pathlib.Path(root) / "examples" / "proposal-flow-video" / "messy-sources" / "01-rfp-ocr.txt")
messages = [
    {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "clientInfo": {"name": "mdp-proposal-mcp-test", "version": "0.0.0-test"},
            "capabilities": {},
        },
    },
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "mdp_proposal_tools", "arguments": {}}},
    {
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": workdir,
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "native_runner": env_probe,
                "dry_run": True,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "bad-raw-text"),
                "source_text": "do not accept ambient chat text",
                "dry_run": True,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "audit-grade-dry-run"),
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "native_runner": env_probe,
                "dry_run": True,
                "require_audit_grade": True,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "timeout"),
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "native_runner": slow_runner,
                "dry_run": True,
                "timeout_ms": 100,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "sensitive-output"),
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "native_runner": sensitive_runner,
                "dry_run": True,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "symlink"),
                "source_paths": [source_symlink],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "dry_run": True,
            },
        },
    },
]
with open(transcript, "w", encoding="utf-8") as handle:
    for message in messages:
        json.dump(message, handle, separators=(",", ":"))
        handle.write("\n")
PY

node --check "$root/scripts/mdp-proposal-mcp-server.mjs"
provider_key="OPENAI_API_KEY"
env MDP_MCP_TEST_MARKER=must-not-leak "$provider_key=mdp-redaction-test-value" \
  node "$root/scripts/mdp-proposal-mcp-server.mjs" < "$transcript" > "$stdout_jsonl" 2> "$stderr_log"

if [ -s "$stderr_log" ]; then
  echo "MCP server wrote unexpected stderr:" >&2
  cat "$stderr_log" >&2
  exit 1
fi

python3 - "$stdout_jsonl" "$workdir/artifacts/native-normalize-request.json" "$workdir/artifacts/source-intake.json" "$workdir/.mdp-proposal-workdir.json" "$workdir/artifacts/native-normalize-dry-run.json" "$mcp_result_schema" "$stderr_log" <<'PY'
import json, pathlib, sys
stdout_path = pathlib.Path(sys.argv[1])
request_path = pathlib.Path(sys.argv[2])
source_intake_path = pathlib.Path(sys.argv[3])
workdir_manifest_path = pathlib.Path(sys.argv[4])
env_probe_path = pathlib.Path(sys.argv[5])
mcp_result_schema = json.load(open(sys.argv[6]))["data"]
stderr_path = pathlib.Path(sys.argv[7])
lines = [line for line in stdout_path.read_text(encoding="utf-8").splitlines() if line.strip()]
assert len(lines) == 9, f"expected 9 JSON-RPC responses, got {len(lines)}: {stdout_path.read_text()}"
messages = [json.loads(line) for line in lines]
responses = {message["id"]: message for message in messages}

def result(id_, label):
    response = responses[id_]
    assert "error" not in response, f"{label} returned error: {response.get('error')}"
    assert "result" in response, f"{label} missing result"
    return response["result"]

init = result(1, "initialize")
assert init["serverInfo"]["name"] == "message-decision-packs-proposal"
assert "tools" in init["capabilities"]
assert "explicit local file paths" in init["instructions"]

tools = result(2, "tools/list")["tools"]
names = {tool["name"] for tool in tools}
assert {"mdp_proposal_tools", "mdp_proposal_run"}.issubset(names)
run_tool = next(tool for tool in tools if tool["name"] == "mdp_proposal_run")
assert run_tool["inputSchema"]["additionalProperties"] is False
assert "source_text" not in run_tool["inputSchema"]["properties"]
assert "source_intake_path" in run_tool["inputSchema"]["properties"]
assert "reuse_workdir_id" in run_tool["inputSchema"]["properties"]
assert run_tool["inputSchema"]["properties"]["timeout_ms"]["maximum"] == 300000
assert "allow_existing" not in run_tool["inputSchema"]["properties"]
output_schema = run_tool["outputSchema"]
assert output_schema["additionalProperties"] is False
for required in ["mode", "decision", "audit_grade_eligible", "timed_out", "timeout_ms", "environment"]:
    assert required in output_schema["required"], f"missing outputSchema requirement {required}"

tools_call = result(3, "tools/call mdp_proposal_tools")
assert tools_call["isError"] is False
assert tools_call["structuredContent"]["contract"] == "mdp.proposal-mcp-tools.v0"
assert tools_call["structuredContent"]["hosted_or_remote_mcp"] is False

run_call = result(4, "tools/call mdp_proposal_run dry-run")
assert run_call["isError"] is False, run_call["content"][0]["text"]
run_content = run_call["structuredContent"]
assert run_content["contract"] == "mdp.proposal-mcp-run-result.v0"
assert run_content["contract"] == mcp_result_schema["properties"]["contract"]["const"]
assert not (set(mcp_result_schema["required"]) - set(run_content))
assert run_content["hosted_or_remote_mcp"] is False
assert mcp_result_schema["properties"]["hosted_or_remote_mcp"]["const"] is False
assert "does not prove model isolation" in mcp_result_schema["description"]
assert run_content["runner_result"]["mode"] == "dry-run"
assert run_content["runner_result"]["audit_grade_eligible"] is False
assert run_content["mode"] == "dry-run"
assert run_content["decision"] == "not-run"
assert run_content["audit_grade_eligible"] is False
assert run_content["timed_out"] is False
assert run_content["termination_signal"] is None
assert run_content["environment"]["policy"] == "allowlist"
assert run_content["environment"]["secret_" + "values_reported"] is False
assert request_path.exists(), "dry-run did not create native-normalize-request.json"
assert source_intake_path.exists(), "dry-run did not create source-intake.json"
assert workdir_manifest_path.exists(), "dry-run did not create workdir ownership manifest"
assert env_probe_path.exists(), "dry-run did not capture native runner environment probe"
request = json.loads(request_path.read_text(encoding="utf-8"))
source_intake = json.loads(source_intake_path.read_text(encoding="utf-8"))
workdir_manifest = json.loads(workdir_manifest_path.read_text(encoding="utf-8"))
env_probe = json.loads(env_probe_path.read_text(encoding="utf-8"))
payload = json.loads(request["input"][0]["content"])
assert request["declared_inputs_only"] is True
for forbidden in ["instructions", "tools", "previous_response_id", "conversation"]:
    assert forbidden not in request, f"request contains forbidden {forbidden}"
assert sorted(payload) == ["existing_pack_context", "raw_opportunity", "source_audit", "source_kind"]
assert source_intake["contract"] == "mdp.source-intake.v0"
assert source_intake["entries"][0]["state"] == "candidate"
assert source_intake["entries"][0]["artifact"]["sha256"] == payload["raw_opportunity"]["sources"][0]["sha256"]
assert workdir_manifest["contract"] == "mdp.proposal-workdir.v0"
assert env_probe["unexpected_marker_present"] is False
assert env_probe["provider_key_present"] is True

raw_text_response = responses[5]
assert "error" in raw_text_response, "raw source_text argument must return a JSON-RPC invalid-params error"
assert raw_text_response["error"]["code"] == -32602
assert "Unsupported arguments: source_text" in raw_text_response["error"]["message"]

audit_required = result(6, "tools/call require_audit_grade dry-run")
assert audit_required["isError"] is True
audit_content = audit_required["structuredContent"]
assert audit_content["runner_exit_status"] == 2
assert audit_content["mode"] == "dry-run"
assert audit_content["decision"] == "not-run"
assert audit_content["audit_grade_eligible"] is False

timeout = result(7, "tools/call timeout")
assert timeout["isError"] is True
timeout_content = timeout["structuredContent"]
assert timeout_content["timed_out"] is True
assert timeout_content["runner_exit_status"] == 124
assert timeout_content["termination_signal"] == "SIGTERM"
assert timeout_content["timeout_ms"] == 100
assert "timed out after 100ms" in timeout_content["stderr"]
assert len(timeout_content["stdout"]) <= 12050
assert len(timeout_content["stderr"]) <= 12050

sensitive = result(8, "tools/call sensitive output redaction")
assert sensitive["isError"] is True
sensitive_json = json.dumps(sensitive)
assert "mdp-redaction-test-value" not in sensitive_json
assert "[REDACTED:" in sensitive_json

symlink = responses[9]
assert "error" in symlink
assert symlink["error"]["code"] == -32602
assert "must not be a symlink" in symlink["error"]["message"]

all_output = stdout_path.read_text(encoding="utf-8") + stderr_path.read_text(encoding="utf-8")
assert "must-not-leak" not in all_output
assert "mdp-redaction-test-value" not in all_output
PY

echo '{"ok":true,"contract":"mdp.proposal-mcp-test.v0"}'
