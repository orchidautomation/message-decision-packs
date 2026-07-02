---
title: MDP-12 Proposal Reference Profile Template and Review-Job Map - Plan
type: docs
date: 2026-07-01
topic: mdp-proposal-reference-profile-template
execution: knowledge-work
linear_project: MDP: Proposal AI Lab
linear_issues:
  - MDP-9
  - MDP-12
  - MDP-13
  - MDP-14
  - MDP-15
  - MDP-20
  - MDP-23
  - MDP-26
  - MDP-36
  - MDP-42
origin:
  - docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md
  - docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md
  - docs/plans/2026-07-01-003-docs-account-context-icp-normalization-plan.md
  - docs/plans/2026-07-01-004-docs-profile-validation-eval-gates-plan.md
  - docs/plans/2026-07-01-005-docs-profile-builder-skill-workflow-plan.md
source_note: Linear Proposal AI Lab documents were reconciled in MDP-42; this public artifact only includes sanitized planning decisions.
---

# MDP-12 Proposal Reference Profile Template and Review-Job Map - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Define the proposal reference-profile plan before creating proposal templates, sample packs, skills, or CLI behavior. |
| Product authority | This artifact resolves MDP-12 after MDP-42 reconciled Proposal AI Lab under the Domain Profile Foundation. |
| Core decision | Proposal should use fixed core `CardKind` families plus proposal-native card IDs and `primitive_map`; it should not add proposal-specific core kinds. |
| Opportunity stance | Proposal opportunity/company/RFP context is an input contract and source-signal lane first, not a first-class core schema object. |
| Review-job stance | Proposal review modes are profile routing jobs that produce review outputs, gaps, and no-draft decisions; MDP does not submit proposals or manage proposal workflow execution. |
| Safety stance | Public samples, fixtures, templates, and skills must wait for the MDP-23 safety gate. |
| Stop condition | Do not implement `plugin/assets/templates/proposal`, proposal skills, eval fixtures, CLI schemas, or `mdp init --template proposal` from this planning branch. |

---

## Product Contract

### Summary

Proposal AI Lab is MDP's first non-GTM reference profile.
It should prove that the profile foundation can support a domain with different nouns, review jobs, and evidence rules while preserving the same MDP boundary:

```text
supplied proposal context -> primitive map -> review job -> bounded review output or gap/no-draft
```

The proposal profile should not become a proposal platform.
It stores and validates local decision context, routing contracts, evidence boundaries, gaps, and evals.
It does not scrape opportunity portals, own customer records, submit proposals, send messages, update CRMs, manage approvals, or run a hosted proposal workflow.

### Reference Profile Rules

Proposal-specific vocabulary belongs in profile-owned IDs, titles, jobs, prompts, input contracts, eval fixtures, and `primitive_map`.
The core loader still uses the fixed `CardKind` families accepted in MDP-39.

That means a future proposal card can be named `bid-no-bid-rules` while declaring `kind: fit-rules`.
The domain meaning comes from the profile ID, card ID, and primitive mapping, not from a new Rust enum variant.

### Proposal Primitive Map

| Universal primitive | Proposal expression | Candidate profile-owned IDs |
|---|---|---|
| `actors` | Customer, agency, evaluator, buyer, proposal owner, solution owner, reviewer, incumbent, partner. | `proposal-roles`, `customer-context` |
| `decision-criteria` | Bid/no-bid gates, compliance thresholds, scoring factors, qualification and escalation rules. | `bid-no-bid-rules`, `evaluation-criteria`, `review-gates` |
| `source-signals` | RFP facts, opportunity facts, source snippets, deadlines, procurement signals, incumbent clues, source confidence. | `opportunity-context`, `requirement-signals`, `source-ledger` |
| `needs-requirements` | Stated requirements, response obligations, evaluation factors, must-answer sections, solution needs. | `requirements-matrix`, `compliance-rules` |
| `evidence-proof` | Approved claims, past performance, references, certifications, differentiators, boilerplate, case studies. | `proof-library`, `past-performance`, `approved-boilerplate` |
| `boundaries` | No-bid constraints, privacy limits, unsupported claims, compliance exclusions, confidentiality rules. | `compliance-boundaries`, `avoid-rules` |
| `output-contracts` | Review brief shape, compliance matrix shape, executive summary shape, risk/gap report shape. | `proposal-output-rules`, `executive-brief-rules`, `review-outputs` |
| `routing-jobs` | Named proposal review modes and their required primitives. | `review-gates`, `proposal-review-jobs` |
| `gaps` | Missing RFP sections, missing proof, unresolved compliance questions, weak source confidence, reviewer blockers. | `gaps` |
| `evals` | Fixtures for proceed, insufficient context, refusal/escalation, unsupported claims, and job routing. | `.mdp/evals/*.yaml` |

### Company, Opportunity, And RFP Context

Company information is not one primitive.
In the proposal profile, company/opportunity/RFP context maps across the primitives below:

| Information | Primary primitive | Also touches | Rule |
|---|---|---|---|
| Customer, agency, buyer, evaluator, incumbent, partner, and internal owner identities | `actors` | `source-signals` | Treat organizations as actors only when the source identifies them. |
| RFP title, source URL, due date, procurement vehicle, budget clues, timeline, incumbent clues, and source snippets | `source-signals` | `gaps` | Preserve provenance, freshness, and uncertainty; do not infer missing facts. |
| Requirement statements, evaluation factors, submission instructions, deliverable obligations, and compliance clauses | `needs-requirements` | `decision-criteria` | Requirements become decision criteria only when they gate proceed, no-bid, compliance, or review output. |
| Bid/no-bid constraints, eligibility, fit, conflicts, security requirements, and mandatory capabilities | `decision-criteria` | `boundaries` | Use as gates; do not convert weak fit into confident pursuit advice. |
| Past performance, case studies, approved differentiators, certifications, and references | `evidence-proof` | `boundaries` | Claim-bearing output requires proof; unsupported proof becomes a gap or avoid-rule. |
| Confidentiality, protected data, no-invention rules, customer-specific restrictions, and public-sample exclusions | `boundaries` | `gaps` | Public repo artifacts must use sanitized synthetic examples. |
| Desired review format, compliance matrix format, executive brief structure, and red-team report shape | `output-contracts` | `routing-jobs` | Output shape is separate from source facts and decision gates. |
| Requested review mode, such as bid/no-bid or compliance review | `routing-jobs` | `output-contracts` | Jobs select required primitives; they do not execute external workflow steps. |
| Missing RFP text, missing evaluator criteria, missing proof, missing owner, or incomplete source ledger | `gaps` | all relevant primitives | Keep gaps visible and block overconfident output. |

### Candidate Card IDs

These IDs are planning candidates for a future `proposal` template.
They should not be added to the repo until profile-aware validation and the MDP-23 safety gate are ready.

| Candidate card ID | Nearest fixed `kind` | Primitive coverage | Notes |
|---|---|---|---|
| `proposal-roles` | `personas` | `actors` | Proposal vocabulary for people and organizations involved in the review. |
| `opportunity-context` | `signals` | `source-signals`, `actors` | Source-backed opportunity/company/RFP facts. |
| `requirement-signals` | `signals` | `source-signals`, `needs-requirements` | Extracted RFP sections, instructions, source snippets, and confidence. |
| `requirements-matrix` | `pains` | `needs-requirements` | Domain-native needs/requirements card using the closest current core family. |
| `bid-no-bid-rules` | `fit-rules` | `decision-criteria`, `boundaries` | Pursuit, pause, decline, and escalation gates. |
| `evaluation-criteria` | `fit-rules` | `decision-criteria`, `needs-requirements` | Scoring and evaluator decision rules. |
| `proof-library` | `claims` | `evidence-proof` | Approved proof points and claim-use rules. |
| `past-performance` | `claims` | `evidence-proof` | References, past examples, and proof constraints. |
| `compliance-boundaries` | `avoid-rules` | `boundaries`, `decision-criteria` | Hard limits, exclusions, and no-invention rules. |
| `proposal-output-rules` | `output-rules` | `output-contracts` | Deterministic formatting and review-output constraints. |
| `review-outputs` | `copy-patterns` | `output-contracts` | Patterns for review briefs, matrices, and risk summaries. |
| `review-gates` | `motions` | `routing-jobs`, `decision-criteria` | Named proposal review modes and sequencing. |
| `gaps` | `gaps` | `gaps` | Missing inputs and activation blockers. |

### Future Manifest Sketch

This is the shape MDP-12 recommends for later implementation after MDP-40 validation support exists.
It is not current template content.

```yaml
profile:
  id: proposal
  label: Proposal Review
  profile_version: mdp.profile.v0
  boundary: decision-pack-not-execution

required_primitives:
  - actors
  - decision-criteria
  - source-signals
  - needs-requirements
  - evidence-proof
  - boundaries
  - output-contracts
  - routing-jobs
  - gaps
  - evals

primitive_map:
  actors:
    cards:
      - proposal-roles
    input_contracts:
      - opportunity
  source-signals:
    cards:
      - opportunity-context
      - requirement-signals
    input_contracts:
      - opportunity
    prompts:
      - normalize-opportunity
  needs-requirements:
    cards:
      - requirements-matrix
      - requirement-signals
  decision-criteria:
    cards:
      - bid-no-bid-rules
      - evaluation-criteria
      - review-gates
  evidence-proof:
    cards:
      - proof-library
      - past-performance
  boundaries:
    cards:
      - compliance-boundaries
      - bid-no-bid-rules
  output-contracts:
    cards:
      - proposal-output-rules
      - review-outputs
  routing-jobs:
    cards:
      - review-gates
    jobs:
      - bid-no-bid-review
      - compliance-review
      - win-theme-proof-review
      - red-team-gap-review
      - executive-brief
  gaps:
    cards:
      - gaps
  evals:
    fixtures:
      - evals/bid-no-bid-good.yaml
      - evals/compliance-insufficient-context.yaml
      - evals/proof-unsupported-claim.yaml
      - evals/red-team-route.yaml

input_contracts:
  - id: opportunity
    schema_ref: mdp.input.proposal-opportunity.v0
    prompt: prompts/normalize-opportunity.yaml
    normalizes:
      - customer
      - opportunity
      - requirements
      - evidence
      - relationship
```

The `opportunity` input contract should remain profile-owned until MDP-26 has evidence that opportunity/pursuit context needs a reusable core schema.

### Review Jobs

Proposal review jobs are routing contracts.
They should produce review outputs, gaps, and refusal/escalation decisions, not external workflow side effects.

| Job ID | Purpose | Required primitives | Output contract | Blocked when |
|---|---|---|---|---|
| `bid-no-bid-review` | Decide proceed, pause, decline, or ask for more context. | `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `boundaries`, `gaps` | Bid/no-bid brief with rationale and required follow-ups. | Missing opportunity context, missing hard gates, or unsupported pursuit assumptions. |
| `compliance-review` | Check requirements, mandatory clauses, and response obligations. | `source-signals`, `needs-requirements`, `decision-criteria`, `boundaries`, `gaps` | Compliance matrix or gap list. | Missing RFP text, ambiguous requirement, or unsupported compliance claim. |
| `win-theme-proof-review` | Connect requirements to approved proof and differentiators. | `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `gaps` | Proof-backed theme brief. | Missing proof, invented differentiator, or claim outside approved evidence. |
| `red-team-gap-review` | Surface weaknesses, contradictions, missing inputs, and risk. | `source-signals`, `decision-criteria`, `boundaries`, `gaps`, `evals` | Risk and gap report. | Review asks for ungrounded speculation or private data exposure. |
| `executive-brief` | Summarize status, risks, proof, and decision options for a stakeholder. | `actors`, `source-signals`, `decision-criteria`, `evidence-proof`, `output-contracts`, `gaps` | Executive summary with next decision needed. | Missing source ledger, unresolved requirements, or no accepted output contract. |

### Eval Gate Plan

Proposal activation should reuse MDP-40's minimum eval categories:

| Eval category | Proposal case |
|---|---|
| `proceed` | Complete sanitized opportunity routes to the expected review job and produces the expected review output shape. |
| `insufficient-context` | Missing RFP text, missing due date, missing evaluator criteria, or missing proof produces gaps and no overconfident recommendation. |
| `refusal` | No-bid, confidentiality, execution-platform request, or unsafe/prohibited proposal behavior refuses or escalates. |
| `unsafe-output` | Unsupported proof, invented case study, or non-source-backed compliance claim fails. |
| `job-routing` | Bid/no-bid, compliance, proof review, red-team, and executive-brief jobs route the intended cards and exclude unrelated cards. |

Proposal-specific fixtures should be synthetic and public-safe.
Real customer opportunities, proposal text, transcripts, portal exports, private references, or identifying details must stay out of the repo.

### Safety Gate Before Public Assets

MDP-23 should run before MDP-13, MDP-14, MDP-15, or MDP-20 create public proposal artifacts.
That gate should decide:

- what synthetic proposal examples are safe for a source-available repo;
- which terms, customer types, and references are too close to private work;
- how to label generated fixtures as synthetic;
- which raw source formats are never committed;
- what a proposal skill must refuse or redact.

---

## Planning Contract

### Key Decisions

- KTD1. Proposal is the first reference profile, not a new core product identity.
- KTD2. Use fixed `CardKind` values and proposal-native card IDs; do not add proposal-specific core card kinds.
- KTD3. Use `primitive_map` as the authority for proposal semantics.
- KTD4. Treat opportunity/company/RFP context as a profile input contract and source-signal lane first.
- KTD5. Do not add first-class opportunity/pursuit schema until MDP-26 has template, sample, eval, and pilot evidence.
- KTD6. Proposal review jobs are bounded routing jobs; they do not submit, send, approve, or manage proposals.
- KTD7. Public proposal samples and evals require the MDP-23 privacy/safety gate.
- KTD8. Proposal builder work in MDP-15 should specialize the generic profile-builder workflow from MDP-41 instead of creating a parallel builder.
- KTD9. `mdp init --template proposal` in MDP-20 should wait for validator support, sample/eval fixtures, and safety approval.

### Downstream Issue Routing

| Issue | MDP-12 output | Required before work starts |
|---|---|---|
| MDP-13 | Build sanitized proposal sample pack around the candidate card IDs and input contract. | MDP-23 safety gate, synthetic source plan, profile validation stance. |
| MDP-14 | Add proposal eval fixtures for routing, gaps, unsupported claims, and no-draft behavior. | MDP-40 eval metadata support or an accepted interim fixture convention. |
| MDP-15 | Create proposal pack-builder skill as a specialization of `mdp-profile-builder`. | MDP-13/14 sample and eval shape plus MDP-23 guardrails. |
| MDP-20 | Add `mdp init --template proposal` only after the template validates cleanly with profile-aware support. | MDP-13/14, validator support, docs, skills, and starter/template sync. |
| MDP-23 | Define privacy and public-repo guardrails. | Needed before any public proposal samples, skills, or template files. |
| MDP-26 | Revisit opportunity/pursuit schema only if evidence shows the profile-owned input contract cannot carry the repeated use case. | Template/sample/eval evidence and at least one pilot or repeated workflow. |

### Implementation Surface For Later PRs

| Surface | Future change | Notes |
|---|---|---|
| `plugin/assets/templates/proposal/.mdp/manifest.yaml` | Add proposal profile metadata, card refs, input contract refs, jobs, and primitive map. | Wait for profile-aware validation. |
| `plugin/assets/templates/proposal/.mdp/cards/*.yaml` | Add sanitized proposal reference cards using fixed kinds and proposal-owned IDs. | Wait for MDP-23. |
| `plugin/assets/templates/proposal/.mdp/prompts/normalize-opportunity.yaml` | Normalize supplied opportunity/RFP/company context into source ledger, requirements, evidence, and gaps. | Profile-owned prompt contract first. |
| `plugin/assets/templates/proposal/.mdp/evals/*.yaml` | Add synthetic profile eval fixtures. | Align with MDP-40 categories. |
| `plugin/skills/` | Add or update proposal/profile-builder skills in the same PR as behavior/template changes. | Feature change hygiene requires skills and templates to match. |
| `cli/src/commands/health.rs` and schemas | Validate proposal profile metadata through generic profile support, not proposal-specific branches. | Keep MDP generic. |
| `cli/src/starter.rs` | Add proposal starter only after the proposal template is accepted. | Avoid `mdp init --template proposal` before the template validates. |

### Validation Strategy

For this docs-only planning PR:

```bash
git diff --cached --check
```

Future implementation PRs should use:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

When a proposal template exists, add a proposal-template validation command beside the basic-template validation.

---

## Sources

- MDP-42 reconciliation in Linear.
- Proposal AI Lab Linear docs reviewed during MDP-42: Project Brief, Execution Roadmap and Issue Index, AI Pack Builder Workflow, Proposal Pack Domain Model, Technical Architecture, and Security Boundaries.
- MDP-37/MDP-38 foundation artifact: `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- MDP-39 accepted card-extensibility artifact: `docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md`.
- MDP-50 account/context artifact: `docs/plans/2026-07-01-003-docs-account-context-icp-normalization-plan.md`.
- MDP-40 validation/eval artifact: `docs/plans/2026-07-01-004-docs-profile-validation-eval-gates-plan.md`.
- MDP-41 profile-builder artifact: `docs/plans/2026-07-01-005-docs-profile-builder-skill-workflow-plan.md`.
- Current basic template manifest: `plugin/assets/templates/basic/.mdp/manifest.yaml`.
