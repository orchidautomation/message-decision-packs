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
source_symlink="$tmp_dir/source-symlink.txt"
oversized_source="$tmp_dir/oversized-source.txt"

cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema proposal-mcp-run-result > "$mcp_result_schema"

source_file="$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt"
ln -s "$source_file" "$source_symlink"
truncate -s 5000001 "$oversized_source"

python3 - "$root" "$pack" "$workdir" "$transcript" "$source_symlink" "$oversized_source" <<'PY'
import json, pathlib, sys
root, pack, workdir, transcript, source_symlink, oversized_source = sys.argv[1:]
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
                "workdir": str(pathlib.Path(workdir).parent / "native-runner-override"),
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "native_runner": source,
                "dry_run": True,
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
                "workdir": str(pathlib.Path(workdir).parent / "mdp-bin-override"),
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "mdp_bin": source,
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
    {
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "too-many-sources"),
                "source_paths": [source] * 17,
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "dry_run": True,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "oversized-source"),
                "source_paths": [oversized_source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "dry_run": True,
            },
        },
    },
    {
        "jsonrpc": "2.0",
        "id": 12,
        "method": "tools/call",
        "params": {
            "name": "mdp_proposal_run",
            "arguments": {
                "pack": pack,
                "workdir": str(pathlib.Path(workdir).parent / "oversized-excerpt"),
                "source_paths": [source],
                "source_id": "synthetic-rfp-summary",
                "source_kind": "synthetic-example",
                "max_source_bytes": 100001,
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

python3 - "$stdout_jsonl" "$workdir/artifacts/native-normalize-request.json" "$workdir/artifacts/source-intake.json" "$workdir/.mdp-proposal-workdir.json" "$mcp_result_schema" "$stderr_log" <<'PY'
import json, pathlib, sys
stdout_path = pathlib.Path(sys.argv[1])
request_path = pathlib.Path(sys.argv[2])
source_intake_path = pathlib.Path(sys.argv[3])
workdir_manifest_path = pathlib.Path(sys.argv[4])
mcp_result_schema = json.load(open(sys.argv[5]))["data"]
stderr_path = pathlib.Path(sys.argv[6])
lines = [line for line in stdout_path.read_text(encoding="utf-8").splitlines() if line.strip()]
assert len(lines) == 12, f"expected 12 JSON-RPC responses, got {len(lines)}: {stdout_path.read_text()}"
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
assert run_tool["inputSchema"]["properties"]["source_paths"]["maxItems"] == 16
assert run_tool["inputSchema"]["properties"]["max_source_bytes"]["maximum"] == 100000
assert "mdp_bin" not in run_tool["inputSchema"]["properties"]
assert "native_runner" not in run_tool["inputSchema"]["properties"]
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
request = json.loads(request_path.read_text(encoding="utf-8"))
source_intake = json.loads(source_intake_path.read_text(encoding="utf-8"))
workdir_manifest = json.loads(workdir_manifest_path.read_text(encoding="utf-8"))
payload = json.loads(request["input"][0]["content"])
assert request["declared_inputs_only"] is True
for forbidden in ["instructions", "tools", "previous_response_id", "conversation"]:
    assert forbidden not in request, f"request contains forbidden {forbidden}"
assert sorted(payload) == ["existing_pack_context", "raw_opportunity", "source_audit", "source_kind"]
assert source_intake["contract"] == "mdp.source-intake.v0"
assert source_intake["entries"][0]["state"] == "candidate"
assert source_intake["entries"][0]["artifact"]["sha256"] == payload["raw_opportunity"]["sources"][0]["sha256"]
assert workdir_manifest["contract"] == "mdp.proposal-workdir.v0"

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

native_override = responses[7]
assert native_override["error"]["code"] == -32602
assert "Unsupported arguments: native_runner" in native_override["error"]["message"]

mdp_override = responses[8]
assert mdp_override["error"]["code"] == -32602
assert "Unsupported arguments: mdp_bin" in mdp_override["error"]["message"]

symlink = responses[9]
assert "error" in symlink
assert symlink["error"]["code"] == -32602
assert "must not be a symlink" in symlink["error"]["message"]

too_many = responses[10]
assert too_many["error"]["code"] == -32602
assert "at most 16" in too_many["error"]["message"]

oversized_source = responses[11]
assert oversized_source["error"]["code"] == -32602
assert "5000000 byte file limit" in oversized_source["error"]["message"]

oversized_excerpt = responses[12]
assert oversized_excerpt["error"]["code"] == -32602
assert "max_source_bytes must be between 1000 and 100000" in oversized_excerpt["error"]["message"]

all_output = stdout_path.read_text(encoding="utf-8") + stderr_path.read_text(encoding="utf-8")
assert "must-not-leak" not in all_output
assert "mdp-redaction-test-value" not in all_output
PY

timeout_bundle="$tmp_dir/timeout-bundle"
mkdir -p "$timeout_bundle/scripts"
cp "$root/scripts/mdp-proposal-mcp-server.mjs" "$timeout_bundle/scripts/"
cat > "$timeout_bundle/scripts/mdp-proposal-runner.mjs" <<'JS'
import { spawn } from 'node:child_process'
import { dirname, join } from 'node:path'
const args = process.argv.slice(2)
const workdir = args[args.indexOf('--workdir') + 1]
const marker = join(dirname(workdir), 'delayed-marker')
spawn(process.execPath, ['-e', `setTimeout(() => require('fs').writeFileSync(${JSON.stringify(marker)}, 'escaped'), 500)`], {
  stdio: 'ignore',
})
setTimeout(() => {}, 5_000)
JS
timeout_transcript="$tmp_dir/timeout-transcript.ndjson"
python3 - "$pack" "$source_file" "$tmp_dir/timeout-workdir" "$timeout_transcript" <<'PY'
import json, sys
pack, source, workdir, transcript = sys.argv[1:]
message = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "mdp_proposal_run",
        "arguments": {
            "pack": pack,
            "workdir": workdir,
            "source_paths": [source],
            "source_id": "synthetic-rfp-summary",
            "source_kind": "synthetic-example",
            "dry_run": True,
            "timeout_ms": 100,
        },
    },
}
open(transcript, "w", encoding="utf-8").write(json.dumps(message) + "\n")
PY
node "$timeout_bundle/scripts/mdp-proposal-mcp-server.mjs" < "$timeout_transcript" > "$tmp_dir/timeout-output.jsonl"
sleep 1
test ! -e "$tmp_dir/delayed-marker"
python3 - "$tmp_dir/timeout-output.jsonl" <<'PY'
import json, sys
response = json.loads(open(sys.argv[1], encoding="utf-8").read())
content = response["result"]["structuredContent"]
assert response["result"]["isError"] is True
assert content["timed_out"] is True
assert content["runner_exit_status"] == 124
assert content["termination_signal"] == "SIGTERM"
PY

line_limit_input="$tmp_dir/line-limit.ndjson"
python3 - "$line_limit_input" <<'PY'
import json, sys
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    handle.write(json.dumps({"jsonrpc":"2.0","id":1,"method":"x","padding":"x" * 1000100}) + "\n")
    handle.write(json.dumps({"jsonrpc":"2.0","id":2,"method":"ping"}) + "\n")
PY
node "$root/scripts/mdp-proposal-mcp-server.mjs" < "$line_limit_input" > "$tmp_dir/line-limit-output.jsonl"
python3 - "$tmp_dir/line-limit-output.jsonl" <<'PY'
import json, sys
messages = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8") if line.strip()]
assert messages[0]["error"]["code"] == -32600
assert "exceeds 1000000 bytes" in messages[0]["error"]["message"]
assert messages[1] == {"jsonrpc":"2.0","id":2,"result":{}}
PY

echo '{"ok":true,"contract":"mdp.proposal-mcp-test.v0"}'
