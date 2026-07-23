#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

pack="$tmp_dir/pack"
tools_json="$tmp_dir/tools.json"
mock_response="$tmp_dir/mock-response.json"
dry_result="$tmp_dir/dry-result.json"
mock_result="$tmp_dir/mock-result.json"
demo_stdout="$tmp_dir/demo.stdout"
helper_audit="$tmp_dir/helper-runner-audit.json"
helper_stdout="$tmp_dir/helper.stdout.json"
helper_receipt="$tmp_dir/helper-receipt.json"
helper_receipt_stdout="$tmp_dir/helper-receipt.stdout.json"

cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- init --template proposal --dir "$pack" > "$tmp_dir/init.json"

python3 - "$root/examples/proposal-flow-video/fixtures/normalize-opportunity-output.json" "$mock_response" <<'PY'
import json, sys
fixture = json.load(open(sys.argv[1]))
payload = {
    "id": "resp_mock_proposal_runner",
    "output": [
        {
            "type": "message",
            "content": [
                {
                    "type": "output_text",
                    "text": json.dumps(fixture, separators=(",", ":")),
                }
            ],
        }
    ],
}
json.dump(payload, open(sys.argv[2], "w"), indent=2)
open(sys.argv[2], "a").write("\n")
PY

node "$root/scripts/mdp-proposal-runner.mjs" tools > "$tools_json"
python3 - "$tools_json" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1]))
assert payload["contract"] == "mdp.proposal-runner-tools.v0"
names = {tool["name"] for tool in payload["tools"]}
for expected in [
    "mdp_intake_sources",
    "mdp_normalize_opportunity",
    "mdp_validate_normalization",
    "mdp_run_receipt",
    "mdp_review_proposal",
]:
    assert expected in names
assert "not currently a hosted MCP implementation" in payload["note"]
PY

node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/dry-run" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source-id synthetic-rfp-summary \
  --source-kind synthetic-example \
  --dry-run > "$dry_result"

python3 - "$dry_result" "$tmp_dir/dry-run/artifacts/native-normalize-request.json" "$tmp_dir/dry-run/artifacts/source-audit.json" <<'PY'
import json, sys
result = json.load(open(sys.argv[1]))
request = json.load(open(sys.argv[2]))
source_audit = json.load(open(sys.argv[3]))
payload = json.loads(request["input"][0]["content"])
assert result["contract"] == "mdp.proposal-runner-result.v0"
assert result["mode"] == "dry-run"
assert result["audit_grade_eligible"] is False
assert result["decision"] == "not-run"
assert request["contract"] == "mdp.native-normalize-request.v0"
assert request["declared_inputs_only"] is True
assert "instructions" not in request
assert "tools" not in request
assert "previous_response_id" not in request
assert "conversation" not in request
assert len(request["input"]) == 1
assert request["input"][0]["role"] == "user"
assert sorted(payload.keys()) == ["existing_pack_context", "raw_opportunity", "source_audit", "source_kind"]
assert source_audit["contract"] == "mdp.source-audit.v0"
assert source_audit["refs"][0]["ref"] == "raw_opportunity.sources[0]"
assert source_audit["refs"][0]["source_id"] == "synthetic-rfp-summary"
PY

node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/mock-run" \
  --source-audit "$root/examples/proposal-flow-video/fixtures/source-audit.json" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source "$root/examples/proposal-flow-video/messy-sources/02-capture-notes.md" \
  --source "$root/examples/proposal-flow-video/messy-sources/03-proof-inventory.md" \
  --source-kind synthetic-example \
  --model gpt-test \
  --mock-response "$mock_response" > "$mock_result"

python3 - "$mock_result" "$tmp_dir/mock-run/artifacts" <<'PY'
import json, pathlib, sys
result = json.load(open(sys.argv[1]))
artifacts = pathlib.Path(sys.argv[2])
request = json.load(open(artifacts / "native-normalize-request.json"))
request_payload = json.loads(request["input"][0]["content"])
validation = json.load(open(artifacts / "normalize-opportunity-validation.json"))["data"]
receipt = json.load(open(artifacts / "run-receipt.json"))
runner_audit = json.load(open(artifacts / "runner-audit.json"))
fit = json.load(open(artifacts / "fit-normalized-opportunity.json"))["data"]
route = json.load(open(artifacts / "route-bid-no-bid-review.json"))["summary"]

assert result["mode"] == "mock"
assert result["ok"] is False
assert result["audit_grade_eligible"] is False
assert result["decision"] == "blocked"
assert result["runner_assurance"] == "invalid"
assert "Mock mode is offline-only" in result["caveats"][0]
assert validation["valid"] is True
assert receipt["decision"] == "blocked"
assert receipt["runner"]["assurance"] == "invalid"
assert runner_audit["contract"] == "mdp.runner-audit.v0"
assert runner_audit["runner"] == "native-api"
assert runner_audit["mock_response"] is True
assert runner_audit["isolated_invocation"] is False
assert runner_audit["stateless_request"] is False
assert runner_audit["tool_invocations_observed"] == 0
assert sorted(request_payload.keys()) == ["existing_pack_context", "raw_opportunity", "source_audit", "source_kind"]
assert fit["status"] in {"fit", "insufficient-context", "disqualified"}
assert fit["decision"]
assert route["job"] == "bid no bid review"
assert route["persona"] == "Proposal Lead"
assert route["card_count"] > 0
PY

DEMO_WORKDIR="$tmp_dir/demo" bash "$root/examples/proposal-flow-video/scripts/run-demo.sh" > "$demo_stdout"

python3 - "$tmp_dir/demo/artifacts/proposal-runner-result.json" "$tmp_dir/demo/artifacts/run-receipt.json" "$tmp_dir/demo/artifacts/runner-audit.json" "$tmp_dir/demo/artifacts/proof-output-verify.json" "$tmp_dir/demo/artifacts/check-claims-unsupported.json" <<'PY'
import json, sys
runner_result = json.load(open(sys.argv[1]))
receipt = json.load(open(sys.argv[2]))
runner_audit = json.load(open(sys.argv[3]))
proof = json.load(open(sys.argv[4]))["data"]
claim = json.load(open(sys.argv[5]))["data"]
assert runner_result["mode"] == "mock"
assert runner_result["audit_grade_eligible"] is False
assert receipt["decision"] == "blocked"
assert receipt["runner"]["assurance"] == "invalid"
assert runner_audit["mock_response"] is True
assert proof["valid"] is True
assert claim["valid"] is False
PY

node "$root/examples/proposal-flow-video/scripts/write-demo-runner-audit.mjs" \
  --prompt-output "$root/examples/proposal-flow-video/fixtures/normalize-opportunity-output.json" \
  --out "$helper_audit" > "$helper_stdout"

cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json validate-prompt-output \
  --dir "$pack" \
  --prompt-id normalize-opportunity \
  --file "$root/examples/proposal-flow-video/fixtures/normalize-opportunity-output.json" \
  --source-audit "$root/examples/proposal-flow-video/fixtures/source-audit.json" > "$tmp_dir/helper-validation.json"

if cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json run-receipt \
  --dir "$pack" \
  --workflow proposal-review \
  --isolation isolated \
  --declared-inputs-only \
  --prompt-id normalize-opportunity \
  --prompt-output "$root/examples/proposal-flow-video/fixtures/normalize-opportunity-output.json" \
  --validation "$tmp_dir/helper-validation.json" \
  --source-audit "$root/examples/proposal-flow-video/fixtures/source-audit.json" \
  --runner-audit "$helper_audit" \
  --require-runner-audit \
  --out "$helper_receipt" > "$helper_receipt_stdout"; then
  echo "expected legacy demo runner-audit helper to block receipt" >&2
  exit 1
fi

python3 - "$helper_stdout" "$helper_audit" "$helper_receipt" <<'PY'
import json, sys
helper = json.load(open(sys.argv[1]))
audit = json.load(open(sys.argv[2]))
receipt = json.load(open(sys.argv[3]))
assert helper["audit_grade_eligible"] is False
assert audit["mock_response"] is True
assert audit["isolated_invocation"] is False
assert audit["declared_inputs_only"] is False
assert receipt["decision"] == "blocked"
assert receipt["runner"]["assurance"] == "invalid"
PY

echo '{"ok":true,"contract":"mdp.proposal-runner-test.v0"}'
