# MDP Concepts

This glossary is the canonical public vocabulary for Message Decision Packs.
Use the contract names below consistently in code, docs, skills, issues, and
review artifacts.

## Product Model

- **Message Decision Pack (pack)** — a local `.mdp/` directory containing
  reviewed decision context and routing contracts.
- **Primitive** — one of the ten domain-agnostic decision families: actors,
  decision criteria, source signals, needs/requirements, evidence/proof,
  boundaries, output contracts, routing/jobs, gaps, and evals.
- **Profile** — a domain-specific mapping of vocabulary and jobs onto the
  primitives. GTM and proposal review are profiles, not separate engines.
- **Card** — a reviewed, typed collection of decision entries.
- **Job** — a closed profile-owned routing intent bound to an eligible skill.
- **Decision Input Contract** — a versioned pack declaration of the attributes, source attempts, normalization evidence, and status behavior required before a job can make deterministic MDP decisions.
- **Source binding** — an integration-owned, provider-neutral mapping from one
  exact compiled job's qualified Decision Input attributes to external fields,
  pinned to portable pack and requirements digests. It is not stored in the
  pack and does not execute collection.
- **Gap** — missing or unsupported context kept explicit rather than inferred.
- **Eval** — a deterministic fixture that tests routing or policy behavior.
- **Decision trace** — a bounded, read-only `mdp.decision-trace.v1`
  projection of existing decision artifacts. The source artifacts retain all
  decision, output, and assurance authority.
- **Designed graph** — policy and gate relationships relevant to a decision.
- **Observed path** — the bounded facts recorded for one decision. A
  **decision graph** is the JSON or Mermaid visualization of the designed
  graph plus observed path.

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
- **Per-run decision:** `audit-grade`, `advisory`, `blocked`, or `not-run`.
- **Readiness:** `ready`, `advisory`, or `blocked`.

Do not substitute “supported” for “verified,” or “valid JSON” for
“audit-grade.” MDP is a decision/context layer, not execution infrastructure.

## Public-Safety Vocabulary

Prefer: local-first, customer-controlled, private workflow, review support,
gap surfacing, unsupported-claim detection, synthetic fixture, and sanitized
artifact.

Do not describe MDP as an AI SDR, CRM, sequencer, enrichment provider,
scraper, BI tool, proposal management system, compliance approval system, or
fully automated proposal writer.
