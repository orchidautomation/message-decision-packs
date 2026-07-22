# Proposal Flow Video Demo

This is a public-safe, synthetic walkthrough for the proposal-review flow Brandon can record with a client.

It starts with intentionally messy source files, creates a fresh proposal `.mdp/` pack, stages the source-audit and normalization artifacts a runner/MCP would produce, then uses the `mdp` CLI to prove the artifacts before producing a bounded proposal-review artifact.

> Scope note: the fixture runner audit in this demo is synthetic so the whole walkthrough can run offline. It demonstrates the exact `mdp.runner-audit.v0` contract the runner/MCP must emit and how the CLI binds hashes. For a paid pilot or real client review, replace `artifacts/runner-audit.demo-mcp.json` with the native/headless runner or MCP-produced audit artifact.

## Run it

From the repository root:

```bash
bash examples/proposal-flow-video/scripts/run-demo.sh
```

The script writes a clean run under `/tmp/mdp-proposal-flow-video` by default and prints the output paths. It uses the source-tree CLI (`cargo run --manifest-path cli/Cargo.toml --`) so it works before the latest release is installed. To force an installed CLI instead:

```bash
MDP_BIN=mdp bash examples/proposal-flow-video/scripts/run-demo.sh
```

## What the video shows

1. **Messy source intake**
   - `messy-sources/01-rfp-ocr.txt`: OCR-ish RFP excerpt with typos and repeated labels.
   - `messy-sources/02-capture-notes.md`: capture notes with assumptions mixed into facts.
   - `messy-sources/03-proof-inventory.md`: approved synthetic proof plus explicit missing proof.
   - `messy-sources/04-compliance-matrix.csv`: rough requirement rows.

2. **Create the pack**
   - `mdp init --template proposal --dir /tmp/mdp-proposal-flow-video/pack`
   - `mdp skills`, `mdp validate`, and `mdp eval` prove the generated `.mdp/` is a valid proposal reference profile.

3. **Runner/MCP staging**
   - `fixtures/source-audit.json` is the bounded source ledger that maps raw refs to approved snippets and `.mdp/sources.yaml` source IDs.
   - `fixtures/normalize-opportunity-output.json` is the strict `mdp.prompt-output.v0` normalization result.
   - `scripts/write-demo-runner-audit.mjs` writes a synthetic `mdp.runner-audit.v0` fixture bound to the prompt-output SHA.

4. **CLI proof gates**
   - `mdp validate-prompt-output --source-audit` checks the model output shape and source refs.
   - `mdp fit` shows readiness/insufficient-context posture for the normalized opportunity compatibility object.
   - `mdp run-receipt --require-runner-audit` hashes and binds pack manifest, prompt output, validation result, source audit, and runner audit.
   - `mdp route --entries` shows the cards a proposal review job should load.
   - `mdp author-proof-output` compiles a proof-output draft only if verification passes.
   - `mdp verify-output --readable` renders a human review layer without treating it as final proposal prose.

## Talk track

- “MDP is local decision context, not a proposal writer or submission system.”
- “The source audit is the bridge from messy PDFs/docs into bounded refs the CLI can check.”
- “The runner/MCP owns the fresh stateless model call and emits runner-audit evidence.”
- “The CLI owns deterministic checks: pack validity, prompt-output refs, receipt hashes, route selection, proof bindings, and claim guardrails.”
- “If proof, certification, compliance status, deadline, or past performance is missing, the workflow surfaces a gap instead of smoothing it into confident copy.”

## Output map

After a run, inspect:

- `/tmp/mdp-proposal-flow-video/pack/.mdp/` — generated proposal pack.
- `/tmp/mdp-proposal-flow-video/artifacts/normalize-opportunity-validation.json` — prompt-output/source-audit validation.
- `/tmp/mdp-proposal-flow-video/artifacts/run-receipt.json` — hash-bound run receipt.
- `/tmp/mdp-proposal-flow-video/artifacts/route-compliance-review.json` — selected pack entries for compliance review.
- `/tmp/mdp-proposal-flow-video/artifacts/proof-output.json` — verified machine proof-output artifact.
- `/tmp/mdp-proposal-flow-video/artifacts/proposal-review.md` — human-readable review layer for the video.

## Production replacement points

For a real client run, keep raw proposal material in customer-controlled scratch, not in this repo. The runner/MCP should create these artifacts from the approved source package:

1. `source-audit.json`
2. `normalize-opportunity-output.json`
3. `runner-audit.json`

Then run the same CLI gates from the script. Do not call a review audit-grade unless `mdp run-receipt --require-runner-audit` returns `decision: audit-grade` using the real runner/MCP audit artifact.
