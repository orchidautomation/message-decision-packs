# MDP Runner Receipts And Context Isolation

Date: 2026-07-21
Issue: MDP-114
Status: first implementation slice

## Decision

MDP keeps Pluxx as the cross-host packaging layer for skills, hooks, templates, and installer assets. Audit-grade normalization is a host-runner responsibility, not something a plain skill prompt can guarantee inside the current conversation.

A user may experience the workflow as one thread:

```text
messy documents -> build or select .mdp -> normalize opportunity -> validate -> review proposal -> readable result
```

Internally, the runner must treat that thread as a control plane only. The evidence plane is local files and receipts:

```text
source files / extracted text
  -> mdp.source-audit.v0
  -> fresh stateless model call with only prompt-declared inputs
  -> mdp.prompt-output.v0
  -> mdp validate-prompt-output --source-audit
  -> mdp.runner-audit.v0
  -> mdp run-receipt --require-runner-audit
  -> mdp fit / route / verify-output / render-brief as needed
```

## Why

Same-conversation normalization can accidentally mix the actual source material with prior chat, brainstorms, assumptions, draft language, or the operator's desired answer. `validate-prompt-output` checks structure, prompt identity, value contracts, source-ref IDs/snippets when a source audit is supplied, and readiness consistency. It does not prove that a model call was context-isolated, and it does not semantically compare every output claim back to every raw source document.

Therefore, production proposal workflows need a runner-owned boundary that can say:

- the normalizer ran in a fresh/stateless model invocation;
- only the prompt-declared payload fields crossed into that invocation;
- a native/headless runner produced a reviewable `mdp.runner-audit.v0` artifact;
- the source audit, prompt output, validation result, and downstream artifacts were persisted locally and hashed;
- the CLI gates ran on those artifacts.

## First CLI Slice

`mdp run-receipt` creates `mdp.run-receipt.v0` from local artifacts. For the default `proposal-review` workflow it requires:

- `--isolation isolated`
- `--declared-inputs-only`
- `--prompt-output <mdp.prompt-output.v0.json>`
- `--validation <validate-prompt-output result.json>` with `valid: true`
- `--source-audit <mdp.source-audit.v0.json>` that was also surfaced in the validation result
- optionally `--runner-audit <mdp.runner-audit.v0.json>`; for paid proposal pilots use `--require-runner-audit`

It records hashes and byte counts for each artifact, reports boundary/validation issues, and returns:

- `decision: audit-grade` and `valid: true` only when the boundary and artifacts satisfy the policy;
- `decision: advisory` when the artifacts validate but context isolation or declared-input confirmation is missing;
- `decision: blocked` when required artifacts are missing, malformed, failed validation, or a required runner audit is missing/invalid.

This is not a full MCP server yet. It is the deterministic receipt contract that the local MCP/runner should call once it owns the model invocation.

## Ownership Split

| Layer | Owns | Does not own |
| --- | --- | --- |
| Pluxx/plugin | Ship skills, hooks, templates, assets, host-specific bundles, and adapter shims that call the runner contract | Model invocation context isolation as a packaging-only concern |
| Host runner / future MCP | Fresh stateless normalization call, declared-input payload, local artifact staging, runner-audit emission, run receipt inputs | Pack fit/routing/proof logic |
| MDP CLI | Validate pack/prompt/proof artifacts, validate runner-audit shape, compute hashes, gate receipt status, run deterministic fit/route/checks | Semantic truth beyond supplied artifacts or host context isolation |
| Agent skill | Explain and orchestrate the workflow for non-programmer operators | Invent proof or claim an audit-grade boundary without a receipt |

## Follow-up Architecture

The next substantive slice should add a host-neutral local runner/MCP surface around the CLI. A small tool set is enough:

- `mdp_intake_sources`: stage PDFs/docs/extracted text and create source-audit inputs.
- `mdp_normalize_opportunity`: perform the fresh/stateless model call using `.mdp/prompts/normalize-opportunity.yaml` and declared inputs only.
- `mdp_validate_normalization`: run `validate-prompt-output --source-audit`.
- `mdp_create_run_receipt`: call `mdp run-receipt` and persist the receipt.
- `mdp_review_proposal`: run route/fit/proof/check gates and render the requested human layer.

The ChatGPT app can use the same runner contract for the polished workshop UX. Codex, Claude Code, Cursor, and other MCP-capable hosts should use the local runner/MCP. If a host cannot provide an isolated model invocation, the skill must label the output advisory rather than audit-grade.

Pluxx should remain the distribution/transpilation layer. It can package host-specific recipes for Codex, Claude Code, Cursor, OpenCode, Copilot, and ChatGPT, but the durable boundary belongs in the runner-audit contract plus CLI receipt so every host proves the same thing.
