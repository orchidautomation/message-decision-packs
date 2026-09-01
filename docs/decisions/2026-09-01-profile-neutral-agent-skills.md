# Profile-neutral agent skill contract

Status: accepted for MDP-309 on 2026-09-01.

## Decision

MDP publishes four task-oriented skills:

- `mdp` for orientation, CLI/contract inspection, and genuinely mixed work;
- `mdp-pack-builder` for explicit pack mutation;
- `mdp-pack-review` for read-only pack and installation QA;
- `mdp-pack-apply` for applying any selected profile job to supplied inputs.

`mdp-gtm-brief` and `mdp-proposal-review` are removed rather than shipped as
aliases. MDP is pre-1.0, the CLI already exposes an authoritative skill
inventory, and retaining aliases would preserve the vertical public surface
this change is intended to eliminate. Release notes and CLI diagnostics provide
the migration boundary; hosts must rediscover the new four-skill package after
upgrade.

## Disclosure algorithm

1. The host discovers only the four generic skill names and descriptions.
2. `mdp-pack-apply` requires an exact pack root and canonical job ID.
3. `mdp --json skills --dir PACK_ROOT --job JOB_ID` resolves the profile,
   readiness, and the single `mdp-pack-apply` recommendation.
4. `mdp --json requirements` and the pack's profile-owned job record disclose
   required inputs, prompt, output contract, and model-task boundary.
5. The skill loads only the shared apply/runtime reference plus the direct GTM
   or proposal reference selected by the resolved profile. It does not load the
   other profile's instructions.
6. The CLI remains authoritative for routing, readiness, artifacts, receipts,
   and verification. Skill prose cannot upgrade a blocked or invalid result.

Profile and job semantics remain in profile-owned manifests, prompts, schemas,
and apply references. The top-level skill contains no GTM or proposal workflow.

## File and execution ownership

- Builder may change an explicitly authorized candidate pack and must use the
  preview/apply validation boundary.
- Review is read-only unless the user separately authorizes repair.
- Apply treats pack authority and supplied inputs as immutable. It writes only
  CLI-owned run/output/receipt artifacts to paths returned by the CLI.
- Clean-context evaluation receives the bounded prompt package, normalized
  input, and routed context selected for the exact job. It does not receive the
  whole pack or ambient host context.

## Runtime and packaging boundary

The CLI is the product authority. MCP stays a thin evaluation transport with
exactly `mdp_run_tools`, `mdp_prepare_run`, `mdp_run`, and `mdp_verify_run`.
It does not gain pack editing, discovery, provider management, or orchestration.

Native activation may detect whether `OPENAI_API_KEY` exists but must never
print or persist its value. Real native model calls still require both the key
and explicit `MDP_ALLOW_NATIVE_MODEL_CALLS=1` consent at server start. The
portable Agent Plugins package remains CLI-only and does not claim native hooks
or MCP registration.

Codex, Claude Code, Cursor, and OpenCode native bundles and the ChatGPT Agent
Plugins archive must expose the same four authored skills. Generated host
bundles remain Pluxx-owned; `plugin/skills/` is the only authored source.

## Compatibility proof

GTM and proposal keep their existing canonical job IDs, input contracts,
prompts, output contracts, safety gates, and behavioral fixtures. Only the
public `skill_id` binding changes to `mdp-pack-apply`. Tests must prove both
profiles still route, prepare, run, verify, and fail closed as before.

