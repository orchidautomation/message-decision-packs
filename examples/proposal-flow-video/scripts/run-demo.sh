#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
example_root="$repo_root/examples/proposal-flow-video"

if [[ -z "${DEMO_WORKDIR:-}" ]]; then
  workdir="/tmp/mdp-proposal-flow-video"
  rm -rf "$workdir"
else
  workdir="$DEMO_WORKDIR"
  if [[ -e "$workdir" ]]; then
    echo "DEMO_WORKDIR already exists: $workdir" >&2
    echo "Choose a new path or remove it yourself." >&2
    exit 1
  fi
fi

pack_root="$workdir/pack"
artifacts="$workdir/artifacts"
mkdir -p "$artifacts"
cp -R "$example_root/messy-sources" "$workdir/messy-sources"

run_mdp() {
  if [[ -n "${MDP_BIN:-}" ]]; then
    "$MDP_BIN" "$@"
  else
    cargo run --quiet --manifest-path "$repo_root/cli/Cargo.toml" -- "$@"
  fi
}

run_proposal_runner() {
  if [[ -n "${MDP_BIN:-}" ]]; then
    node "$repo_root/scripts/mdp-proposal-runner.mjs" "$@" --mdp-bin "$MDP_BIN"
  else
    node "$repo_root/scripts/mdp-proposal-runner.mjs" "$@"
  fi
}

copy_fixture() {
  local name="$1"
  cp "$example_root/fixtures/$name" "$artifacts/$name"
}

printf '\n== Proposal flow video demo ==\n'
printf 'workdir: %s\n' "$workdir"
printf 'repo:    %s\n\n' "$repo_root"

printf '1) Messy sources staged\n'
find "$workdir/messy-sources" -maxdepth 1 -type f | sort | sed 's#^#   - #'

printf '\n2) Create and validate a fresh proposal .mdp pack\n'
run_mdp --json init --template proposal --dir "$pack_root" --dry-run > "$artifacts/00-init-dry-run.json"
run_mdp --json init --template proposal --dir "$pack_root" > "$artifacts/01-init.json"
run_mdp --json skills --dir "$pack_root" > "$artifacts/02-skills.json"
run_mdp --json validate --dir "$pack_root" > "$artifacts/03-validate.json"
run_mdp --json eval --dir "$pack_root" > "$artifacts/04-eval.json"

printf '   pack: %s/.mdp\n' "$pack_root"

printf '\n3) Run the local proposal runner surface\n'
copy_fixture source-audit.json
runner_mode="${DEMO_RUNNER_MODE:-mock}"
runner_workdir="$workdir/runner"
mock_response="$artifacts/native-runner-mock-response.json"

if [[ "$runner_mode" == "mock" ]]; then
  python3 - "$example_root/fixtures/normalize-opportunity-output.json" "$mock_response" <<'PY'
import json, sys
fixture = json.load(open(sys.argv[1]))
payload = {
    "id": "resp_mock_proposal_flow_video",
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
fi

runner_args=(
  run
  --pack "$pack_root"
  --workdir "$runner_workdir"
  --source-audit "$artifacts/source-audit.json"
  --source "$workdir/messy-sources/01-rfp-ocr.txt"
  --source "$workdir/messy-sources/02-capture-notes.md"
  --source "$workdir/messy-sources/03-proof-inventory.md"
  --source "$workdir/messy-sources/04-compliance-matrix.csv"
  --source-kind synthetic-example
)

case "$runner_mode" in
  mock)
    runner_args+=(--model gpt-test --mock-response "$mock_response")
    ;;
  native)
    if [[ -z "${DEMO_OPENAI_MODEL:-}" ]]; then
      echo "DEMO_RUNNER_MODE=native requires DEMO_OPENAI_MODEL." >&2
      exit 1
    fi
    runner_args+=(--model "$DEMO_OPENAI_MODEL")
    if [[ "${DEMO_REQUIRE_AUDIT_GRADE:-0}" == "1" ]]; then
      runner_args+=(--require-audit-grade)
    fi
    ;;
  *)
    echo "Unsupported DEMO_RUNNER_MODE: $runner_mode (expected mock or native)" >&2
    exit 1
    ;;
esac

run_proposal_runner "${runner_args[@]}" > "$artifacts/proposal-runner-result.json"

runner_artifacts="$runner_workdir/artifacts"
cp "$runner_artifacts/source-audit.json" "$artifacts/source-audit.json"
cp "$runner_artifacts/native-normalize-request.json" "$artifacts/native-normalize-request.json"
cp "$runner_artifacts/normalize-opportunity-output.json" "$artifacts/normalize-opportunity-output.json"
cp "$runner_artifacts/runner-audit.json" "$artifacts/runner-audit.json"
cp "$runner_artifacts/normalize-opportunity-validation.json" "$artifacts/normalize-opportunity-validation.json"
cp "$runner_artifacts/run-receipt.json" "$artifacts/run-receipt.json"
cp "$runner_artifacts/run-receipt.stdout.json" "$artifacts/run-receipt.stdout.json"
cp "$runner_artifacts/normalized-opportunity.json" "$artifacts/normalized-opportunity.json"
cp "$runner_artifacts/fit-normalized-opportunity.json" "$artifacts/fit-normalized-opportunity.json"
cp "$runner_artifacts/route-bid-no-bid-review.json" "$artifacts/route-bid-no-bid-review.json"

printf '   source audit:  %s\n' "$artifacts/source-audit.json"
printf '   prompt output: %s\n' "$artifacts/normalize-opportunity-output.json"
printf '   runner audit:  %s\n' "$artifacts/runner-audit.json"
printf '   runner result: %s\n' "$artifacts/proposal-runner-result.json"

printf '\n4) Use CLI route gates after the runner receipt\n'

run_mdp --json --summary route \
  --entries \
  --dir "$pack_root" \
  --persona "Solution Owner" \
  --job "compliance review" \
  > "$artifacts/route-compliance-review.json"

printf '   validation: %s\n' "$artifacts/normalize-opportunity-validation.json"
printf '   fit:        %s\n' "$artifacts/fit-normalized-opportunity.json"
printf '   receipt:    %s\n' "$artifacts/run-receipt.json"

printf '\n5) Compile and verify a proof-carrying proposal review artifact\n'
run_mdp --json author-proof-output \
  --dir "$pack_root" \
  --draft "$pack_root/examples/proof-output-drafts/compliance-row.draft.json" \
  --out "$artifacts/proof-output.json" \
  > "$artifacts/proof-output-author.json"

run_mdp --json verify-output \
  --dir "$pack_root" \
  --file "$artifacts/proof-output.json" \
  > "$artifacts/proof-output-verify.json"

run_mdp verify-output \
  --readable \
  --dir "$pack_root" \
  --file "$artifacts/proof-output.json" \
  > "$artifacts/proposal-review.md"

if run_mdp --json check-claims \
  --dir "$pack_root" \
  --persona "Proposal Lead" \
  --job "compliance review" \
  --text "The sample team is CMMC compliant." \
  > "$artifacts/check-claims-unsupported.json"; then
  echo "Expected unsupported compliance claim to fail, but it passed." >&2
  exit 1
fi

printf '   proof output:       %s\n' "$artifacts/proof-output.json"
printf '   readable review:    %s\n' "$artifacts/proposal-review.md"
printf '   unsupported-claim:  %s\n' "$artifacts/check-claims-unsupported.json"

python3 - "$artifacts" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])

def load(name):
    return json.loads((root / name).read_text())

validation = load('normalize-opportunity-validation.json')
receipt = load('run-receipt.json')
fit = load('fit-normalized-opportunity.json')
proof = load('proof-output-verify.json')
claim = load('check-claims-unsupported.json')
runner = load('runner-audit.json')
runner_result = load('proposal-runner-result.json')
print('\n== Demo summary ==')
print(f"runner mode:         {runner_result['mode']} / audit eligible: {runner_result['audit_grade_eligible']}")
print(f"prompt output valid: {validation['data']['valid']}")
print(f"fit status:          {fit['data']['status']} ({fit['data']['decision']})")
print(f"receipt decision:    {receipt['decision']} / runner assurance: {receipt['runner']['assurance']}")
print(f"mock response:       {runner.get('mock_response', False)} (production needs real native/headless evidence)")
print(f"proof decision:      {proof['data']['decision']} / valid: {proof['data']['valid']}")
print(f"unsafe claim valid:  {claim['data']['valid']} / guardrails: {len(claim['data']['guardrail_hits'])}")
print(f"\nOpen: {root / 'proposal-review.md'}")
PY
