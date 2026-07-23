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

printf '\n3) Stage runner/MCP-like source and normalization artifacts\n'
copy_fixture source-audit.json
copy_fixture normalize-opportunity-output.json
node "$example_root/scripts/write-demo-runner-audit.mjs" \
  --prompt-output "$artifacts/normalize-opportunity-output.json" \
  --out "$artifacts/runner-audit.demo-mcp.json" \
  > "$artifacts/runner-audit.demo-mcp.stdout.json"

python3 - "$artifacts/normalize-opportunity-output.json" "$artifacts/normalized-opportunity.json" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1]))
json.dump(payload["normalized_prospect"], open(sys.argv[2], "w"), indent=2)
open(sys.argv[2], "a").write("\n")
PY

printf '   source audit:  %s\n' "$artifacts/source-audit.json"
printf '   prompt output: %s\n' "$artifacts/normalize-opportunity-output.json"
printf '   runner audit:  %s\n' "$artifacts/runner-audit.demo-mcp.json"

printf '\n4) Prove normalization, readiness, and receipt boundaries with the CLI\n'
run_mdp --json validate-prompt-output \
  --dir "$pack_root" \
  --prompt-id normalize-opportunity \
  --file "$artifacts/normalize-opportunity-output.json" \
  --source-audit "$artifacts/source-audit.json" \
  > "$artifacts/normalize-opportunity-validation.json"

run_mdp --json fit \
  --dir "$pack_root" \
  --prospect "$artifacts/normalized-opportunity.json" \
  > "$artifacts/fit-normalized-opportunity.json"

set +e
run_mdp --json run-receipt \
  --dir "$pack_root" \
  --workflow proposal-review \
  --isolation isolated \
  --declared-inputs-only \
  --prompt-id normalize-opportunity \
  --prompt-output "$artifacts/normalize-opportunity-output.json" \
  --validation "$artifacts/normalize-opportunity-validation.json" \
  --source-audit "$artifacts/source-audit.json" \
  --runner-audit "$artifacts/runner-audit.demo-mcp.json" \
  --require-runner-audit \
  --out "$artifacts/run-receipt.json" \
  > "$artifacts/run-receipt.stdout.json"
receipt_status=$?
set -e
if [[ "$receipt_status" -eq 0 ]]; then
  echo "Expected the synthetic demo runner-audit fixture to be blocked, but it was accepted as audit-grade." >&2
  exit 1
fi
python3 - "$artifacts/run-receipt.json" <<'PY'
import json, sys
receipt = json.load(open(sys.argv[1]))
issues = {issue.get("code") for issue in receipt.get("issues", [])}
expected = {"runner_audit_demo_fixture", "runner_audit_synthetic_model"}
if receipt.get("decision") != "blocked" or not expected.issubset(issues):
    raise SystemExit(
        "Expected synthetic demo runner-audit fixture to produce a blocked receipt with fixture issue codes"
    )
PY

run_mdp --json --summary route \
  --entries \
  --dir "$pack_root" \
  --persona "Proposal Lead" \
  --job "bid no bid review" \
  > "$artifacts/route-bid-no-bid-review.json"

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
runner = load('runner-audit.demo-mcp.json')
print('\n== Demo summary ==')
print(f"prompt output valid: {validation['data']['valid']}")
print(f"fit status:          {fit['data']['status']} ({fit['data']['decision']})")
print(f"receipt decision:    {receipt['decision']} / runner assurance: {receipt['runner']['assurance']}")
print(f"runner fixture:      {runner.get('demo_fixture', False)} (CLI blocks this fixture from audit-grade; replace with real MCP/native runner audit for production)")
print(f"proof decision:      {proof['data']['decision']} / valid: {proof['data']['valid']}")
print(f"unsafe claim valid:  {claim['data']['valid']} / guardrails: {len(claim['data']['guardrail_hits'])}")
print(f"\nOpen: {root / 'proposal-review.md'}")
PY
