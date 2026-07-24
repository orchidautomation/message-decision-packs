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
minimal_attrs_output="$tmp_dir/minimal-attrs-output.json"
dry_result="$tmp_dir/dry-result.json"
mock_result="$tmp_dir/mock-result.json"
demo_stdout="$tmp_dir/demo.stdout"
helper_audit="$tmp_dir/helper-runner-audit.json"
helper_stdout="$tmp_dir/helper.stdout.json"
helper_receipt="$tmp_dir/helper-receipt.json"
helper_receipt_stdout="$tmp_dir/helper-receipt.stdout.json"
source_intake_schema="$tmp_dir/source-intake.schema.json"
source_audit_schema="$tmp_dir/source-audit.schema.json"
native_request_schema="$tmp_dir/native-normalize-request.schema.json"
prompt_output_schema="$tmp_dir/prompt-output.schema.json"
runner_result_schema="$tmp_dir/proposal-runner-result.schema.json"
run_manifest_schema="$tmp_dir/proposal-run-manifest.schema.json"

cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- init --template proposal --dir "$pack" > "$tmp_dir/init.json"
cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema source-intake > "$source_intake_schema"
cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema source-audit > "$source_audit_schema"
cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema native-normalize-request > "$native_request_schema"
cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema prompt-output > "$prompt_output_schema"
cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema proposal-runner-result > "$runner_result_schema"
cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json schema proposal-run-manifest > "$run_manifest_schema"

python3 - "$root/examples/proposal-flow-video/fixtures/normalize-opportunity-output.json" "$mock_response" "$minimal_attrs_output" <<'PY'
import copy, json, sys
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
minimal = copy.deepcopy(fixture)
for key in ["normalized_prospect", "normalized_opportunity"]:
    attrs = minimal[key]["attributes"]
    attrs.pop("opportunity_stage", None)
    attrs.pop("pursuit_decision", None)
json.dump(minimal, open(sys.argv[3], "w"), indent=2)
open(sys.argv[3], "a").write("\n")
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
assert "bundled local stdio MCP wrapper" in payload["note"]
assert "hosted or remote MCP" in payload["note"]
PY

node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/dry-run" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source-id synthetic-rfp-summary \
  --source-kind synthetic-example \
  --dry-run > "$dry_result"

python3 - "$dry_result" "$tmp_dir/dry-run/artifacts/native-normalize-request.json" "$tmp_dir/dry-run/artifacts/source-audit.json" "$tmp_dir/dry-run/artifacts/source-intake.json" "$tmp_dir/dry-run/.mdp-proposal-workdir.json" "$tmp_dir/dry-run/.mdp-proposal-run.json" "$source_intake_schema" "$source_audit_schema" "$native_request_schema" "$prompt_output_schema" "$runner_result_schema" "$run_manifest_schema" <<'PY'
import json, pathlib, sys
result = json.load(open(sys.argv[1]))
request = json.load(open(sys.argv[2]))
source_audit = json.load(open(sys.argv[3]))
source_intake = json.load(open(sys.argv[4]))
workdir_manifest = json.load(open(sys.argv[5]))
run_manifest = json.load(open(sys.argv[6]))
source_intake_schema = json.load(open(sys.argv[7]))["data"]
source_audit_schema = json.load(open(sys.argv[8]))["data"]
native_request_schema = json.load(open(sys.argv[9]))["data"]
prompt_output_schema = json.load(open(sys.argv[10]))["data"]
runner_result_schema = json.load(open(sys.argv[11]))["data"]
run_manifest_schema = json.load(open(sys.argv[12]))["data"]
payload = json.loads(request["input"][0]["content"])

def assert_required_keys(value, schema):
    missing = sorted(set(schema["required"]) - set(value))
    assert not missing, f"{schema.get('title')} missing required keys: {missing}"

def assert_openai_strict_schema(schema, path="#"):
    assert "oneOf" not in schema, f"{path} must use OpenAI-supported anyOf, not oneOf"
    if schema.get("type") == "object":
        properties = schema.get("properties", {})
        assert schema.get("additionalProperties") is False, f"{path} object must set additionalProperties false"
        assert sorted(schema.get("required", [])) == sorted(properties.keys()), f"{path} object required must include every property"
        for key, value in properties.items():
            assert_openai_strict_schema(value, f"{path}/properties/{key}")
    if "items" in schema:
        assert_openai_strict_schema(schema["items"], f"{path}/items")
    for keyword in ["anyOf"]:
        for index, value in enumerate(schema.get(keyword, [])):
            assert_openai_strict_schema(value, f"{path}/{keyword}/{index}")

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
assert_openai_strict_schema(request["prompt_output_schema"])
top_level_required = request["prompt_output_schema"]["required"]
top_level_properties = request["prompt_output_schema"]["properties"]
assert "normalized_prospect" in top_level_required
assert "normalized_prospect" in top_level_properties
assert "normalized_opportunity" not in top_level_required
assert "normalized_opportunity" not in top_level_properties
attributes_schema = top_level_properties["normalized_prospect"]["properties"]["attributes"]
assert attributes_schema["required"] == ["source_safety"]
assert sorted(attributes_schema["properties"].keys()) == ["source_safety"]
missing_schema = request["prompt_output_schema"]["properties"]["normalization_trace"]["properties"]["missing_required"]
any_of = missing_schema["items"]["anyOf"]
object_shapes = [shape for shape in any_of if shape.get("type") == "object"]
assert any(shape.get("type") == "string" for shape in any_of)
assert len(object_shapes) == 1
missing_object = object_shapes[0]
assert missing_object["additionalProperties"] is False
assert missing_object["required"] == ["field", "path", "reason", "source_evidence"]
assert sorted(missing_object["properties"].keys()) == ["field", "path", "reason", "source_evidence"]
assert source_audit["contract"] == "mdp.source-audit.v0"
assert source_audit["refs"][0]["ref"] == "raw_opportunity.sources[0]"
assert source_audit["refs"][0]["source_id"] == "synthetic-rfp-summary"
assert source_intake["contract"] == "mdp.source-intake.v0"
assert len(source_intake["entries"]) == 1
intake_entry = source_intake["entries"][0]
assert intake_entry["state"] == "candidate"
assert intake_entry["approval_class"] == "candidate"
assert intake_entry["privacy_class"] == "synthetic-public"
assert intake_entry["source_kind"] == "synthetic-example"
assert intake_entry["artifact"]["sha256"] == payload["raw_opportunity"]["sources"][0]["sha256"]
assert payload["raw_opportunity"]["source_intake"]["sha256"]
assert intake_entry["audit_refs"] == ["raw_opportunity.sources[0]"]
assert workdir_manifest["contract"] == "mdp.proposal-workdir.v0"
assert workdir_manifest["workdir_id"]
assert run_manifest["contract"] == "mdp.proposal-run-manifest.v0"
assert run_manifest["run_id"] == result["run_id"]
assert run_manifest["status"] == "completed"
assert run_manifest["ended_at"]
assert not pathlib.Path(result["workdir"], ".mdp-proposal-run.lock").exists()
assert any(item["path"] == "artifacts/proposal-runner-result.json" for item in run_manifest["artifacts"])
assert source_audit["contract"] == source_audit_schema["properties"]["contract"]["const"]
assert request["contract"] == native_request_schema["properties"]["contract"]["const"]
assert result["contract"] == runner_result_schema["properties"]["contract"]["const"]
assert prompt_output_schema["properties"]["contract"]["const"] == "mdp.prompt-output.v0"
assert source_intake_schema["properties"]["contract"]["const"] == "mdp.source-intake.v0"
assert run_manifest_schema["properties"]["contract"]["const"] == "mdp.proposal-run-manifest.v0"
assert "Only a human operator" in source_intake_schema["description"]
for value, schema in [
    (source_intake, source_intake_schema),
    (source_audit, source_audit_schema),
    (request, native_request_schema),
    (result, runner_result_schema),
]:
    assert_required_keys(value, schema)
assert "cannot be audit-grade" in runner_result_schema["properties"]["mode"]["description"]
PY

python3 - "$tmp_dir/dry-run/artifacts/source-intake.json" "$tmp_dir/approved-source-intake.json" <<'PY'
import json, sys
value = json.load(open(sys.argv[1]))
for entry in value["entries"]:
    entry["state"] = "approved"
    entry["approval_class"] = "operator-approved"
    entry["approval"] = {
        "decision": "approved",
        "operator": "synthetic-test-human",
        "decided_at": "2026-07-24T00:00:00Z",
        "purpose": "proposal-review",
        "artifact_sha256": entry["artifact"]["sha256"],
    }
json.dump(value, open(sys.argv[2], "w"), indent=2)
open(sys.argv[2], "a").write("\n")
PY

node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/approved-dry-run" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source-intake "$tmp_dir/approved-source-intake.json" \
  --source-id synthetic-rfp-summary \
  --source-kind synthetic-example \
  --dry-run > "$tmp_dir/approved-dry-result.json"

python3 - "$tmp_dir/approved-dry-run/artifacts/source-intake.json" <<'PY'
import json, sys
entry = json.load(open(sys.argv[1]))["entries"][0]
assert entry["state"] == "approved"
assert entry["approval_class"] == "operator-approved"
assert entry["approval"]["artifact_sha256"] == entry["artifact"]["sha256"]
PY

workdir_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["workdir_id"])' "$tmp_dir/dry-run/.mdp-proposal-workdir.json")"
node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/dry-run" \
  --reuse-workdir-id "$workdir_id" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source-id synthetic-rfp-summary \
  --source-kind synthetic-example \
  --dry-run > "$tmp_dir/reused-dry-result.json"

cat > "$tmp_dir/slow-native-runner.mjs" <<'JS'
setTimeout(() => {
  console.log(JSON.stringify({ contract: "mdp.native-normalize-dry-run.v0", ok: true }))
}, 1500)
JS

node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/concurrent-workdir" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source-id synthetic-rfp-summary \
  --source-kind synthetic-example \
  --native-runner "$tmp_dir/slow-native-runner.mjs" \
  --dry-run > "$tmp_dir/concurrent-first.json" 2> "$tmp_dir/concurrent-first.stderr" &
first_pid=$!
for _ in $(seq 1 50); do
  if test -f "$tmp_dir/concurrent-workdir/.mdp-proposal-run.json"; then break; fi
  sleep 0.05
done
concurrent_workdir_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["workdir_id"])' "$tmp_dir/concurrent-workdir/.mdp-proposal-workdir.json")"

expect_fail() {
  local expected="$1"
  shift
  local stderr="$tmp_dir/expected-failure.stderr"
  if "$@" >"$tmp_dir/expected-failure.stdout" 2>"$stderr"; then
    echo "expected command to fail: $*" >&2
    exit 1
  fi
  grep -F "$expected" "$stderr" >/dev/null
}

expect_fail "partial or unknown runs fail closed" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/concurrent-workdir" \
    --reuse-workdir-id "$concurrent_workdir_id" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id synthetic-rfp-summary \
    --source-kind synthetic-example \
    --dry-run
wait "$first_pid"

expect_fail "source-id must be lowercase safe ID" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/unsafe-source-id" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id ../escape \
    --source-kind synthetic-example \
    --dry-run

expect_fail "Generating a source audit from --source requires --source-id" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/missing-source-id" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-kind synthetic-example \
    --dry-run

expect_fail "does not exist in .mdp/sources.yaml" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/unknown-source-id" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id nonexistent-source \
    --source-kind synthetic-example \
    --dry-run

python3 - "$tmp_dir/unknown-source-id/.mdp-proposal-run.json" <<'PY'
import json, pathlib, sys
manifest = json.load(open(sys.argv[1]))
assert manifest["status"] == "blocked"
assert manifest["decision"] == "blocked"
assert manifest["ended_at"]
assert manifest["error"]["code"] == "runner-failed"
assert not pathlib.Path(sys.argv[1]).with_name(".mdp-proposal-run.lock").exists()
PY

ln -s "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" "$tmp_dir/source-link.txt"
expect_fail "source must not be a symlink" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/symlink-source" \
    --source "$tmp_dir/source-link.txt" \
    --source-id synthetic-rfp-summary \
    --source-kind synthetic-example \
    --dry-run

mkdir -m 700 "$tmp_dir/stale-workdir"
printf 'unowned stale data\n' > "$tmp_dir/stale-workdir/stale.txt"
expect_fail "Workdir already exists and is not empty" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/stale-workdir" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id synthetic-rfp-summary \
    --source-kind synthetic-example \
    --dry-run

expect_fail "Workdir reuse manifest does not match" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/dry-run" \
    --reuse-workdir-id wrong-id \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id synthetic-rfp-summary \
    --source-kind synthetic-example \
    --dry-run

ln -s "$tmp_dir/dry-run" "$tmp_dir/workdir-link"
expect_fail "Workdir must not be a symlink" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/workdir-link" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id synthetic-rfp-summary \
    --source-kind synthetic-example \
    --dry-run

python3 - "$tmp_dir/malicious-source-audit.json" <<'PY'
import json, sys
json.dump({
    "contract": "mdp.source-audit.v0",
    "refs": [
        {
            "ref": "raw_opportunity.summary",
            "source_id": "synthetic-rfp-summary",
            "locator": "different-file.txt#injected",
            "snippet": "attacker supplied unrelated text",
        },
        {
            "ref": "source_kind",
            "source_id": "synthetic-rfp-summary",
            "locator": "operator-input#source-kind",
            "snippet": "synthetic-example",
        },
    ],
}, open(sys.argv[1], "w"), indent=2)
PY
expect_fail "must bind to exactly one staged source with matching snippet bytes" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/malicious-audit" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-audit "$tmp_dir/malicious-source-audit.json" \
    --source-kind synthetic-example \
    --dry-run

expect_fail "Real native runs require --source-intake" \
  node "$root/scripts/mdp-proposal-runner.mjs" run \
    --pack "$pack" \
    --workdir "$tmp_dir/real-without-intake" \
    --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
    --source-id synthetic-rfp-summary \
    --source-kind synthetic-example \
    --model test-model

cargo run --quiet --manifest-path "$root/cli/Cargo.toml" -- --json validate-prompt-output \
  --dir "$pack" \
  --prompt-id normalize-opportunity \
  --file "$minimal_attrs_output" \
  --source-audit "$root/examples/proposal-flow-video/fixtures/source-audit.json" > "$tmp_dir/minimal-attrs-validation.json"

python3 - "$tmp_dir/minimal-attrs-validation.json" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1]))["data"]
assert payload["valid"] is True
PY

node "$root/scripts/mdp-proposal-runner.mjs" run \
  --pack "$pack" \
  --workdir "$tmp_dir/mock-run" \
  --source-audit "$root/examples/proposal-flow-video/fixtures/source-audit.json" \
  --source "$root/examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt" \
  --source "$root/examples/proposal-flow-video/messy-sources/02-capture-notes.md" \
  --source "$root/examples/proposal-flow-video/messy-sources/03-proof-inventory.md" \
  --source "$root/examples/proposal-flow-video/messy-sources/04-compliance-matrix.csv" \
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
source_intake = json.load(open(artifacts / "source-intake.json"))
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
intake_artifact = next(item for item in receipt["artifacts"] if item["kind"] == "source-intake")
assert intake_artifact["sha256"] == __import__("hashlib").sha256(
    (artifacts / "source-intake.json").read_bytes()
).hexdigest()
assert source_intake["entries"][0]["state"] == "candidate"
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
