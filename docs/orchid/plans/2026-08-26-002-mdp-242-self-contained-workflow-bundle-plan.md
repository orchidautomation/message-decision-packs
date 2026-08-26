---
title: MDP-242 Self-Contained Workflow Bundle Handoff - Implementation Plan
type: feature
date: 2026-08-26
topic: self-contained-workflow-bundle
execution: orchid
artifact_contract: orchid-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-242
---

# MDP-242 Self-Contained Workflow Bundle Handoff - Implementation Plan

## Context and current behavior

At planning base `5aaaf850b24b57622aca118da84cf02649380ab7`, the CLI already provides the durable primitives this issue needs:

- `mdp --json prepare-run` compiles one explicit `mdp.run-request.v1` from a pack, job, selected step, model, and declared input paths.
- `mdp --json run --request ... --out-dir ...` creates a new run directory and returns canonical execution/authority data.
- `mdp --json verify-run --bundle ... --receipt ...` independently verifies the explicit bundle and receipt.
- `scripts/mdp-run-mcp-server.mjs` exposes the same file-oriented sequence without adding decision authority.

The product gap is in authored skill workflow behavior. `plugin/skills/mdp-gtm-brief/SKILL.md` and its mode references tell users/hosts to exchange `SOURCE_BINDING_JSON`, `SOURCE_ATTEMPT_REQUEST_JSON`, `COLLECTED_ATTEMPT_RESULTS_JSON`, `OUTPUT_JSON`, bound prompts, routed context, and invocation receipts. `plugin/skills/mdp-proposal-review/SKILL.md` similarly exposes several compatibility artifacts before the canonical run handoff. The general `plugin/skills/mdp/SKILL.md` documents prepare/run/verify, but there is no shared convention that makes intermediates private workflow state and returns one durable pointer.

## Objective, scope, out of scope, and decisions

Make every normal skill workflow accept the pack, exact job, and approved inputs/sources; privately manage required intermediates; and return one explicit verified run directory plus canonical decision, gaps, retention result, and next permitted action.

Pinned decisions:

- Reuse the existing `prepare-run` → `run` → `verify-run` authority path. Do not add a new CLI command or workflow manifest in this issue.
- A skill creates one private, permission-restricted scratch root per invocation. All intermediate paths stay inside it and are never copied through chat.
- The durable output is a caller-selected new run directory outside the scratch root. Handoff names only that directory and its verified bundle/receipt paths.
- Resume/review requires an explicit run directory and re-runs `verify-run`; there is no ambient latest selection.
- Scratch is removed on success, canonical no-draft/blocked result, handled failure, timeout, and cancellation. Only canonical allowed durable artifacts and bounded diagnostics survive.
- Advanced explicit-artifact workflows remain available but follow identical validation and authority gates.

Out of scope: a global artifact registry, generic list/show/clean commands, a second manifest, automatic commits, MCP dependency, host-specific state, or changing run-bundle/receipt schemas.

## Acceptance mapping

| Acceptance criterion | Implementation | Validation |
|---|---|---|
| Normal use requires no intermediate paths | Add one shared workflow-handoff reference and update each consumer skill to keep compile/normalize/route/receipt paths internal. | Cold skill evals supply only pack/job/approved input and reject prompts that request intermediate choreography. |
| Scratch stays private and bodies are not copied through chat | Require per-run restricted scratch ownership, path-only process boundaries, and bounded result summaries. | Contract lint and privacy evals reject body/path leakage and permissions weaker than current-user-only. |
| Success returns the existing run directory | Standardize a handoff block containing explicit run directory, verification status, canonical decision/terminal, gaps, retention, and next action. | Synthetic E2E verifies the named bundle/receipt inside the returned directory. |
| Resume/review is explicit and verified | Add an explicit resume branch that refuses missing/ambiguous pointers and runs `verify-run` before consuming results. | Valid, missing, tampered, wrong-artifact-root, and ambient-latest cases. |
| Every terminal path cleans MDP-owned scratch | Require one cleanup boundary around the whole workflow and define permitted durable/diagnostic survivors. | Success, no-draft, handled failure, timeout, cancellation, and concurrent-run filesystem assertions. |
| Advanced explicit paths preserve authority | Keep current reference routes as an advanced section and run the same validators before canonical execution. | Parity eval compares decision, gaps, and receipt identity for managed and explicit routes. |
| No MCP or host dependency | Document direct CLI as canonical and MCP as optional transport only. | Run the synthetic E2E through direct CLI; packaging tests ensure references are self-contained. |

## Affected files and symbols

- `plugin/skills/mdp/references/workflow-bundle-handoff.md` (new): canonical private-scratch lifecycle, direct CLI sequence, explicit resume verification, result/handoff schema, cleanup matrix, and advanced-path parity.
- `plugin/skills/mdp/SKILL.md`: make the managed bundle handoff the normal path and link the canonical reference; preserve detailed authority and advanced operator guidance.
- `plugin/skills/mdp-gtm-brief/SKILL.md` and `plugin/skills/mdp-gtm-brief/references/prospect-fit-or-brief.md`: replace user-facing intermediate handoffs with internal workflow state; keep exact lineage validation as an advanced/implementation detail.
- `plugin/skills/mdp-gtm-brief/references/outbound-copy-brief.md` and `outbound-copy-review.md`: consume an explicit verified run directory and canonical routed context/receipt instead of ambient or chat-carried artifacts.
- `plugin/skills/mdp-proposal-review/SKILL.md` and `references/evidence-path.md`: make canonical v1 run-directory handoff primary; retain v0 proposal runner as labeled compatibility only.
- `plugin/skills/mdp-pack-review/SKILL.md`: verify an explicitly supplied durable pointer when reviewing execution evidence; never discover latest state.
- `scripts/test_skill_contracts.py`, `scripts/test_skill_packaging.py`, and the affected skill eval fixtures/indexes: enforce trigger clarity, reference closure, managed/advanced parity, cleanup, privacy, and explicit-resume behavior.
- `docs/run-receipts.md` and `docs/new-codex-user-journey.md`: document the user-facing handoff and one synthetic golden path.

Forbidden without replanning: MCP server/runner files owned by MDP-246, Rust run-bundle/receipt authority or schemas, global artifact commands, and generated host bundles outside `plugin/skills/`.

## Ordered implementation steps

1. Write the canonical workflow-bundle reference around the existing CLI primitives. Define the input boundary, restricted scratch ownership, durable output selection, exact command order, cleanup matrix, and bounded handoff fields.
2. Update the general MDP skill so managed handoff is the default for normal work. Keep explicit artifacts as an advanced path and preserve every fail-closed authority rule.
3. Refactor GTM brief instructions so requirements compilation, source-attempt collection, normalization, validation, routed context, invocation receipt, and request compilation occur inside one private workflow root. The user sees gaps or one verified durable pointer, not intermediate file requests.
4. Refactor proposal review instructions to use the same canonical v1 handoff. Keep source approval explicit and out-of-band; keep the v0 proposal runner/MCP route clearly labeled compatibility.
5. Update review/resume instructions to require an explicit directory, locate only its named bundle/receipt, call `verify-run`, and stop on missing, ambiguous, invalid, or tampered artifacts. Never scan for the newest run.
6. Standardize the final handoff block: run directory, verification, canonical decision/terminal, unresolved gaps, retained/discarded state, receipt/bundle names, and next permitted action. Do not include source bodies or private intermediate paths.
7. Add static contract tests for the new default, advanced parity, direct-CLI availability, explicit resume, cleanup language, and absence of ambient-latest behavior.
8. Add cold behavioral evals for success, no-draft, handled failure, timeout, cancellation, concurrent workflows, tampered resume, and a user asking for raw intermediates. Require authority monotonicity and privacy in every case.
9. Run one synthetic direct-CLI golden path and document the exact user-visible input/handoff. If the existing CLI cannot complete that proof without exposing an intermediate, stop and replan the smallest CLI affordance instead of silently inventing one.

## Tests and validation

Focused:

```bash
python3 scripts/test_skill_contracts.py
python3 scripts/test_skill_packaging.py
python3 scripts/test_skill_eval_harness.py --skill mdp
python3 scripts/test_skill_eval_harness.py --skill mdp-gtm-brief
python3 scripts/test_skill_eval_harness.py --skill mdp-proposal-review
python3 scripts/test_skill_eval_harness.py --skill mdp-pack-review
```

Regression:

```bash
make validate-skills
make validate
```

Manual synthetic proof: use a temporary pack/input and durable output parent, complete one managed workflow, parse the returned pointer, run `mdp --json verify-run` against its bundle/receipt, confirm private scratch is gone, then repeat with a tampered receipt and an interrupted invocation.

## Compatibility, migration, rollout, observability, and rollback

- Existing explicit-artifact instructions remain as an advanced route; users are no longer required to shuttle them during normal use.
- Direct CLI remains canonical. MCP is optional transport and never required for the workflow.
- No artifact/schema migration is introduced. Previously created explicit run directories remain resumable when verification passes.
- The handoff reports retained versus discarded state and canonical gaps/decision so operators can diagnose without private artifact bodies.
- Roll back by reverting the authored skill/docs/tests PR. Generated host bundles remain Pluxx-owned and are not edited here.

## Risks and safety boundaries

- A skill cannot upgrade a blocked/no-draft result because the user requested a polished output; it must return the canonical decision and gaps.
- Cleanup may remove only the exact invocation-owned scratch root. Never recursively delete a caller-selected pack, durable output root, repository, or broad temp directory.
- The durable run directory must be outside scratch before cleanup and must not overwrite an existing path.
- Verification proves artifact integrity/contract consistency, not source truth, isolation, or business correctness.
- Cross-skill links must remain packaging-safe; only authored `plugin/skills/` sources are edited.

## Blockers and readiness verdict

The canonical CLI already supplies compile, run, bundle, receipt, and verification primitives. The plan pins a no-new-CLI implementation and includes an explicit stop/replan gate if the synthetic proof disproves that assumption. No live issue dependency blocks the skill-contract work.

**Verdict: `READY_TO_PIN`.**
