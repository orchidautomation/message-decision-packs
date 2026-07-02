---
title: MDP-23 Proposal Privacy and Public-Repo Guardrails - Plan
type: docs
date: 2026-07-01
topic: mdp-proposal-privacy-public-repo-guardrails
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
origin:
  - docs/plans/2026-07-01-006-docs-proposal-reference-profile-template-plan.md
source_note: Based on the Proposal AI Lab security/confidentiality boundary document; this public artifact omits private names and source details.
---

# MDP-23 Proposal Privacy and Public-Repo Guardrails - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Codify proposal-specific public repo, skill, template, sample, and eval safety rules before proposal artifacts are implemented. |
| Product authority | This artifact resolves MDP-23 from the accepted MDP-12 proposal reference-profile plan and the Proposal AI Lab security boundary source. |
| Core decision | Public proposal artifacts must be generic, synthetic, or explicitly sanitized. Real customer proposal materials stay private unless explicitly approved. |
| Positioning stance | MDP can support local, customer-controlled proposal review and gap surfacing; it does not make compliance claims or replace legal, procurement, security, proposal management, or compliance review. |
| Skill stance | Future proposal skills must refuse or redact raw confidential material, avoid invented proof/compliance/past performance, and route weak evidence to gaps. |
| Stop condition | Do not implement proposal templates, proposal skills, eval fixtures, or `mdp init --template proposal` from this planning branch. |

---

## Product Contract

### Summary

Proposal work has a higher privacy and confidentiality risk than the neutral GTM starter.
Proposal context often includes non-public RFP material, customer strategy, pricing, past performance, certifications, references, procurement constraints, and reviewer judgment.

The public MDP repo can codify the generic shape of proposal review.
It must not become a place where raw customer proposal context, private Linear discussion, access-controlled source material, or regulated data is copied for convenience.

### Core Boundary

MDP is a local/offline decision-context and routing-contract layer.
For proposal workflows, MDP may help structure:

- source-ledgered opportunity context;
- bid/no-bid rules;
- compliance and requirement gaps;
- approved proof boundaries;
- review jobs;
- output contracts;
- eval fixtures.

MDP does not:

- submit proposals;
- manage proposal approvals;
- replace proposal management software;
- replace legal, security, procurement, or compliance review;
- certify that a team is compliant;
- bypass customer data-handling policy;
- collect or scrape RFP/proposal sources.

### Safer Positioning

Use language such as:

- local-first;
- customer-controlled;
- private workflow;
- supports review and governance;
- helps surface gaps and unsupported claims;
- uses sanitized or approved source material for workshops;
- avoids uploading sensitive proposal context into a new platform by default.

Avoid language such as:

- CMMC compliant;
- NIST compliant;
- guaranteed secure;
- approved for CUI;
- legal/procurement bypass;
- replaces compliance review;
- replaces proposal management software;
- fully automated proposal writing.

---

## Public Repo Policy

### Public-Safe

The following can be committed when written generically:

| Artifact | Public-safe rule |
|---|---|
| Proposal template structure | Use generic card IDs, profile metadata, jobs, and file layout. |
| Synthetic sample pack | Use fictional companies, fictional opportunities, synthetic RFP snippets, and obvious `synthetic-example` provenance. |
| Sanitized example | Commit only when intentionally reviewed and stripped of customer-identifying or confidential detail. |
| Generic skill instructions | Teach agents how to handle proposal profile work without exposing private customer context. |
| Validation commands | Include CLI commands, eval categories, and generic fixture names. |
| Local-first docs | Describe private workflow boundaries without making compliance guarantees. |

### Not Public-Safe

Do not commit:

- raw proposal documents;
- non-public RFPs or source documents behind access controls;
- customer names without approval;
- private customer packs;
- customer-specific pricing strategy;
- confidential win themes;
- screenshots of private systems;
- private transcripts or private messages;
- tokens, cookies, browser sessions, local auth files, or local-only paths;
- CUI, regulated content, or material that a customer policy would restrict from external AI tools.

### Sanitization Standard

A sanitized proposal artifact is public-safe only when it removes:

- customer names, agency names, partner names, and evaluator names unless explicitly approved;
- unique project names, solicitation numbers, account IDs, URLs, and dates that identify the real opportunity;
- pricing, deal strategy, win themes, incumbency, internal politics, or competitive strategy;
- references, certifications, and past performance claims that are not generic or approved;
- source snippets that are copied from private RFPs, portals, email, meetings, or documents.

When in doubt, use a synthetic example instead of a sanitized real one.

---

## Skill And Agent Rules

Future proposal skills should include these rules before they can be public:

| Rule | Required behavior |
|---|---|
| Raw source handling | Keep raw proposal/RFP/customer material in ignored private scratch unless the user explicitly approves a sanitized commit. |
| Source provenance | Preserve whether content is synthetic, sanitized, user-provided, public, or private scratch. |
| Proof | Do not invent proof, certifications, compliance status, past performance, references, or customer results. |
| Compliance | Avoid compliance certification language; surface compliance questions as gaps or review-needed items. |
| Claims | Run claim-checking or equivalent review before claim-bearing output is accepted. |
| Gaps | Keep missing requirements, missing proof, missing source text, and weak evidence visible. |
| Refusal/escalation | Refuse or escalate requests to bypass policy, expose private material, claim unsupported compliance, or automate external proposal submission. |
| Outputs | Produce review briefs, matrices, risk reports, and gap lists; do not submit or send proposal artifacts. |

The proposal profile builder should specialize the generic profile-builder workflow from MDP-41.
It should not become a parallel pack builder that silently smooths raw proposal source into confident guidance.

---

## Sample And Eval Rules

Proposal samples and evals should be synthetic by default.

Required fixture markers:

- `source_kind: synthetic-example` for fully fictional fixtures;
- `synthetic: true` when the fixture represents a made-up company, opportunity, evaluator, or requirement;
- `sanitized-example` only when the example was intentionally reviewed and redacted;
- `do_not_submit: true` or equivalent proposal-domain marker when a fixture could be mistaken for a real opportunity.

Minimum unsafe cases for MDP-14:

| Eval category | Required proposal case |
|---|---|
| `insufficient-context` | Missing RFP text, missing evaluation criteria, missing proof, or missing customer approval blocks confident output. |
| `refusal` | Request to bypass legal/procurement/security review refuses or escalates. |
| `unsafe-output` | Unsupported compliance, certification, past performance, or customer-result claim fails. |
| `job-routing` | Proposal jobs route expected cards without loading unrelated proof or output patterns. |
| `public-safety` | A fixture containing private-looking source content is rejected or treated as private scratch only. |

---

## Workshop Safety Checklist

Before using proposal material in a workshop or pilot, confirm:

- what source material is approved for use;
- whether any AI tools are allowed with the source material;
- whether any content is CUI, regulated, confidential, or access-controlled;
- whether examples must be synthetic or anonymized;
- whether the generated pack can leave the session;
- where pack files will live;
- who can access the pack;
- what claims, proof, certifications, and past performance are approved.

If the answers are unclear, use synthetic data for the session and keep any real pack private.

---

## Downstream Issue Routing

| Issue | MDP-23 guardrail |
|---|---|
| MDP-13 | Build only synthetic or explicitly sanitized sample packs. |
| MDP-14 | Include unsafe/private/unsupported-claim fixtures before proposal evals are accepted. |
| MDP-15 | Proposal builder skill must include raw-source, proof, compliance, and refusal/escalation rules. |
| MDP-20 | `mdp init --template proposal` must not ship before the public template and samples satisfy these guardrails. |
| MDP-26 | Opportunity/pursuit schema evidence must come from sanitized summaries or private approved evidence, not public raw customer artifacts. |

---

## Planning Contract

### Key Decisions

- KTD1. Public proposal artifacts must be generic, synthetic, or intentionally sanitized.
- KTD2. Raw proposal/RFP/customer material belongs in private scratch or private customer packs, not public repo paths, public PRs, or public issue text.
- KTD3. Proposal positioning can say local-first, customer-controlled, review support, and gap surfacing.
- KTD4. Proposal positioning must not claim compliance certification, guaranteed security, legal/procurement bypass, replacement of compliance review, or fully automated proposal writing.
- KTD5. Proposal skills must emphasize claim-checking, gap reporting, unsupported-claim detection, and refusal/escalation.
- KTD6. Public proposal evals need unsafe/private/unsupported-claim cases, not only happy-path routing.
- KTD7. MDP remains decision context and routing contracts; external proposal workflow execution stays out of scope.

### Implementation Surface For Later PRs

| Surface | Future change | Notes |
|---|---|---|
| `plugin/skills/` | Add these guardrails to any proposal or profile-builder skill that handles proposal source material. | Same PR as the skill. |
| `plugin/assets/templates/proposal` | Use synthetic examples and public-safe card content only. | Wait for MDP-13 and MDP-23 closeout. |
| `plugin/assets/templates/proposal/.mdp/evals` | Include unsafe/private/unsupported-claim fixtures. | Coordinate with MDP-14. |
| Public docs | Keep proposal language local-first and review-support oriented. | Avoid compliance guarantees. |
| `AGENTS.md` | Keep public-artifact guardrails visible to repo agents. | Added by this planning PR. |

### Validation Strategy

For this docs/instruction PR:

```bash
git diff --cached --check
```

No CLI/template validation is required because this PR does not change CLI behavior, plugin runtime behavior, template assets, or skill bundles.

---

## Sources

- MDP-23 Linear issue.
- Proposal AI Lab Security and Confidentiality Boundaries Linear document.
- MDP-12 proposal reference-profile plan: `docs/plans/2026-07-01-006-docs-proposal-reference-profile-template-plan.md`.
- Existing repo public safety instructions: `AGENTS.md`.
- Existing synthetic fixture guidance in `README.md`, `docs/getting-started.md`, and `plugin/skills/mdp-pack-eval/SKILL.md`.
