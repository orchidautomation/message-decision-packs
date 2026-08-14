# MDP Concepts

This glossary is the canonical public vocabulary for Message Decision Packs.
Use the contract names below consistently in code, docs, skills, issues, and
review artifacts.

## Minimal Model Context

Canonical jobs can own deterministic entry and byte budgets. MDP selects required job authority and guardrails first, reports safe exclusion metadata, and hashes the exact `mdp.routed-context.v1` projection. Budget overflow or a whole-card fallback blocks; legacy jobs without a budget remain `unassessed`. Governed outputs are receipt-bound to those exact context bytes and may cite only typed authority selected in that projection. See [Minimal context routing](docs/minimal-context-routing.md).

## Product Model

- **Versioned decision context for agents** — MDP's primary category. A pack
  makes the rules, evidence, boundaries, gaps, and job-specific context an
  agent may use explicit and versioned. The CLI deterministically validates,
  resolves, and projects that authority; the host remains responsible for
  model calls and external actions.
- **Message Decision Pack (pack)** — a local `.mdp/` directory containing
  reviewed decision context and routing contracts.
- **Primitive** — one of the ten domain-agnostic decision families: actors,
  decision criteria, source signals, needs/requirements, evidence/proof,
  boundaries, output contracts, routing/jobs, gaps, and evals.
- **Profile** — a domain-specific mapping of vocabulary and jobs onto the
  primitives. GTM and proposal review are profiles, not separate engines.
- **Card** — a reviewed, typed collection of decision entries.
- **Job** — a closed profile-owned routing intent bound to an eligible skill.
- **Product-foundation facet** — a profile-owned index of exact existing card
  entries and explicit gaps for one product-understanding concern. It is not a
  primitive, card kind, or second source of product prose.
- **Product-foundation binding** — one canonical job's classification of facet
  IDs as required, conditional, optional, or excluded.
- **Resolved product foundation** — the CLI's deterministic, exact-job
  projection of required and triggered conditional facets. Optional, excluded,
  and untriggered content is not selected.
- **Job-owned model task** — one canonical job's explicit versioned prompt,
  declared input producers, instructions, and exact structured output contract.
  The customer's host executes it; MDP compiles and validates it.
- **Decision Input Contract** — a versioned pack declaration of the attributes, source attempts, normalization evidence, and status behavior required before a job can make deterministic MDP decisions.
- **Signal projection** — a pack-owned, repeated Decision Input projection that
  assigns a profile-defined signal kind and closed qualification roles to
  observations contributed by declared attributes. It is not inferred from
  titles, prose, provider fields, or legacy signal strings.
- **Source binding** — an integration-owned, provider-neutral mapping from one
  exact compiled job's qualified Decision Input attributes and signal
  projections to external fields,
  pinned to portable pack and requirements digests. It is not stored in the
  pack and does not execute collection.
- **Lineage-validated** — the submitted v2 source binding, request, collected
  results, normalized observations, and hashes are internally consistent with
  the compiled pack policy. It does not establish host authenticity,
  authorization, signer identity, non-repudiation, or observation truth.
- **Legacy signal** — readable prospect context without the compiled v2
  projection and receipt chain. It is `legacy` or `unassessed`, never
  first-class sourced proof for an explicit role.
- **Gap** — missing or unsupported context kept explicit rather than inferred.
- **Eval** — a deterministic fixture that tests routing or policy behavior.
- **Deterministic sufficiency** — the D1-D12 checks for one exact release and
  job. A passing result is `sufficient-for-job`; it permits behavioral testing
  but does not prove model performance.
- **Behavioral evaluation** — validated, externally recorded trial evidence
  for one candidate and host/model envelope. It is an intermediate input, not
  the private/public report authority.
- **Job conformance** — the sole hash-complete `mdp.job-conformance.v1`
  authority joining candidate, deterministic evaluation, behavioral
  evaluation, and exact trial set.
- **Decision trace** — a bounded, read-only `mdp.decision-trace.v1`
  projection of existing decision artifacts. The source artifacts retain all
  decision, output, and assurance authority.
- **Designed graph** — policy and gate relationships relevant to a decision.
- **Observed path** — the bounded facts recorded for one decision. A
  **decision graph** is the JSON or Mermaid visualization of the designed
  graph plus observed path. It is a visualization term, not a claim that MDP
  is a graph database, agent runtime, orchestration framework, persistent
  memory layer, or universal company graph.

## Evidence And Assurance

- **Source intake** — the human approval ledger for exact source bytes.
- **Source audit** — bounded source snippets and hashes used for citation
  validation; it does not grant approval.
- **Runner audit** — host-owned evidence about one model invocation boundary.
- **Run receipt** — the deterministic per-invocation assurance decision. This
  is the audit-grade gate.
- **Run manifest** — atomic ownership and terminal-state record for a proposal
  workdir; it does not replace a receipt.
- **Readiness report** — structured findings and hash anchors derived from
  runner artifacts; it cannot upgrade the receipt.
- **Confidence anchor** — a hash-bound artifact supporting why a finding is
  present. Confidence measures anchoring, not semantic truth probability.

## State Vocabulary

- **Integration support:** `verified`, `recipe-only`, `unsupported`, or
  `fixture/mock-only`.
- **Cold-model conformance:** `sufficient-for-job` means deterministic checks
  passed; `qualified-for-job-under-envelope` adds the declared behavioral
  threshold; `unassessed` means required evidence is missing or incomplete;
  `not-sufficient-for-job` and `not-qualified-for-job-under-envelope` mean a
  required deterministic or behavioral assertion failed. None grants drafting,
  sending, or publication authority.
- **Per-run decision:** `audit-grade`, `advisory`, `blocked`, or `not-run`.
- **Readiness:** `ready`, `advisory`, or `blocked`.
- **Product-foundation status:** `unassessed`, `ready`, or `blocked`.
- **Signal authority:** `lineage-validated`, `legacy`, or `unassessed`.

Product-foundation readiness is veto-only. `blocked` prevents broader
activation, but `ready` never establishes sufficient-for-job, self-standing,
commercial, human-approved, or audit-grade status. `unassessed` preserves
legacy compatibility without claiming sufficiency.

Do not substitute “supported” for “verified,” or “valid JSON” for
“audit-grade.” MDP is a decision/context layer, not execution infrastructure.

## Public-Safety Vocabulary

Prefer: local-first, customer-controlled, private workflow, review support,
gap surfacing, unsupported-claim detection, synthetic fixture, and sanitized
artifact.

Do not describe MDP as an AI SDR, CRM, sequencer, enrichment provider,
scraper, BI tool, proposal management system, compliance approval system, or
fully automated proposal writer. Do not position it as a graph database, agent
runtime, orchestration framework, persistent memory layer, or universal company
graph, and do not claim that its hashes or traces prove source truth.
