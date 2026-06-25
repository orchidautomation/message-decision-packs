# Flue vs Eve For MDP Webhook Drafting

Date: 2026-06-25

## Decision

Use Flue first for the Profound MDP webhook draft example. Keep Eve as a follow-on option if the production target becomes explicitly Vercel-native.

## Reasoning

MDP's useful boundary is local and file-based: a webhook supplies a prospect row, trusted code writes a local prospect JSON file, the `mdp` CLI makes the fit and brief decisions, and an agent drafts only from the returned context. Flue's workflow API maps directly to that shape: finite operation, workflow filesystem, application-owned custom webhook route, and a Node target that can run the local CLI.

Eve is attractive when Vercel should own the operational substrate: Vercel Functions, Workflows, Sandbox, AI Gateway, Connect, and dashboard observability. That is useful once the host decision is made. It is a heavier commitment while MDP is still a local/offline CLI and pack contract.

## Operational Boundary

The Flue wrapper is not part of MDP's core. It is an adapter around the CLI:

- inbound webhook verification and admission belong to the wrapper;
- prospect normalization writes ignored scratch files;
- `mdp fit` owns whether drafting is allowed;
- `mdp brief --context` owns routed context;
- the model drafts from the brief only;
- `mdp check-claims` gates the drafted text;
- sending, scheduling, CRM updates, enrichment, scraping, and sequencer writes remain out of scope.

## Current Example

The runnable example lives at:

```text
examples/profound-gtm-vetting/flue-webhook-agent/
```

It uses the committed Profound pack and synthetic sample prospect row. Production payloads should stay in ignored scratch.
