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

python3 - "$root" "$pack" "$workdir" "$transcript" <<'PY'
import json, pathlib, sys
root, pack, workdir, transcript = sys.argv[1:]
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
]
with open(transcript, "w", encoding="utf-8") as handle:
    for message in messages:
        json.dump(message, handle, separators=(",", ":"))
        handle.write("\n")
PY

node --check "$root/scripts/mdp-proposal-mcp-server.mjs"
node "$root/scripts/mdp-proposal-mcp-server.mjs" < "$transcript" > "$stdout_jsonl" 2> "$stderr_log"

if [ -s "$stderr_log" ]; then
  echo "MCP server wrote unexpected stderr:" >&2
  cat "$stderr_log" >&2
  exit 1
fi

python3 - "$stdout_jsonl" "$workdir/artifacts/native-normalize-request.json" <<'PY'
import json, pathlib, sys
stdout_path = pathlib.Path(sys.argv[1])
request_path = pathlib.Path(sys.argv[2])
lines = [line for line in stdout_path.read_text(encoding="utf-8").splitlines() if line.strip()]
assert len(lines) == 5, f"expected 5 JSON-RPC responses, got {len(lines)}: {stdout_path.read_text()}"
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

tools_call = result(3, "tools/call mdp_proposal_tools")
assert tools_call["isError"] is False
assert tools_call["structuredContent"]["contract"] == "mdp.proposal-mcp-tools.v0"
assert tools_call["structuredContent"]["hosted_or_remote_mcp"] is False

run_call = result(4, "tools/call mdp_proposal_run dry-run")
assert run_call["isError"] is False, run_call["content"][0]["text"]
run_content = run_call["structuredContent"]
assert run_content["contract"] == "mdp.proposal-mcp-run-result.v0"
assert run_content["hosted_or_remote_mcp"] is False
assert run_content["runner_result"]["mode"] == "dry-run"
assert run_content["runner_result"]["audit_grade_eligible"] is False
assert request_path.exists(), "dry-run did not create native-normalize-request.json"
request = json.loads(request_path.read_text(encoding="utf-8"))
payload = json.loads(request["input"][0]["content"])
assert request["declared_inputs_only"] is True
for forbidden in ["instructions", "tools", "previous_response_id", "conversation"]:
    assert forbidden not in request, f"request contains forbidden {forbidden}"
assert sorted(payload) == ["existing_pack_context", "raw_opportunity", "source_audit", "source_kind"]

raw_text_response = responses[5]
assert "error" in raw_text_response, "raw source_text argument must return a JSON-RPC invalid-params error"
assert raw_text_response["error"]["code"] == -32602
assert "Unsupported arguments: source_text" in raw_text_response["error"]["message"]

PY

echo '{"ok":true,"contract":"mdp.proposal-mcp-test.v0"}'
