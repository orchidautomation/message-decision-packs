---
title: MDP-41 Profile Builder Skill Workflow - Plan
type: docs
date: 2026-07-01
topic: mdp-profile-builder-skill-workflow
execution: knowledge-work
linear_project: MDP: Domain Profile Foundation
linear_issues:
  - MDP-36
  - MDP-37
  - MDP-38
  - MDP-39
  - MDP-40
  - MDP-41
  - MDP-50
origin:
  - docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md
  - docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md
  - docs/plans/2026-07-01-003-docs-account-context-icp-normalization-plan.md
  - docs/plans/2026-07-01-004-docs-profile-validation-eval-gates-plan.md
source_note: Linear referenced an "AI Pack Builder Workflow" source, but no matching local repo file or Linear document was found during this planning pass.
---

# MDP-41 Profile Builder Skill Workflow - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Specify the profile-builder agent workflow before implementing new plugin behavior. |
| Product authority | This artifact resolves MDP-41 from the accepted domain-profile foundation, card-extensibility, account-context, and validation/eval-gate plans. |
| Core decision | The profile builder should be an orchestrator over existing MDP skills and CLI gates, not a parallel pack authoring system or execution platform. |
| Skill stance | Add a future `mdp-profile-builder` skill only as a workflow coordinator. It should call into `mdp-source-extract`, `mdp-icp-builder`, `mdp-create-pack`, `mdp-pack-review`, and `mdp-pack-eval` by phase. |
| Review stance | AI-extracted profile judgment is draft material until human review accepts the source ledger, primitive coverage, boundaries, eval plan, and activation gate results. |
| Stop condition | Do not implement skill files, CLI behavior, template metadata, or profile eval fixtures from this planning branch. |

---

## Product Contract

### Summary

The profile builder is the workflow that turns messy source context into a reviewed, local MDP profile candidate.
It is the "machine that builds the machine" path, but it still has the same MDP boundary:

```text
source material -> source ledger -> primitive coverage -> draft pack/profile -> human review -> validation/evals -> bounded activation
```

The builder must not treat plausible generated card content as authority.
It should make provenance, assumptions, missing evidence, and unsafe requests visible before any pack or profile is used by downstream agents.

### Boundary

The profile builder works only on local/offline decision context and routing contracts.

It does not:

- scrape, enrich, browse private sessions, or collect source material on its own;
- update CRMs, sequencers, Clay, Deepline, proposal systems, applicant tracking systems, or support systems;
- send, schedule, submit, approve, or execute downstream work;
- turn a draft profile into activated behavior without review and validation gates;
- hide weak evidence by smoothing it into confident rules.

If a user asks for execution infrastructure, the skill should redirect to a separate exact-action system outside MDP and require explicit approval.
Inside MDP, the answer is a boundary, a gap, or a handoff note, not an execution side effect.

### Workflow Stages

| Stage | Purpose | Existing skill or CLI reused | Output |
|---|---|---|---|
| 0. Classify request | Decide whether the user wants a profile, GTM pack improvement, source extraction, review, or execution-platform work. | `mdp-lfg`, future `mdp-profile-builder` routing | Route decision and refusal/handoff if needed. |
| 1. Source intake | Inventory user-provided or public source material and privacy limits. | `mdp-source-extract` | Source ledger plan, provenance, missing source gaps. |
| 2. Profile selection | Choose an existing profile, draft a new approved-domain profile, or park the request. | Future `mdp-profile-builder` | Profile intent, domain boundary, vocabulary, required primitives. |
| 3. Primitive extraction | Map source material into universal decision primitives. | `mdp-source-extract`, `mdp-icp-builder` for GTM | Candidate primitive coverage table and gaps. |
| 4. Pack assembly | Write or propose manifest, cards, prompts, jobs, and eval fixtures in slices. | `mdp-create-pack` plus future profile guidance | Draft `.mdp/` changes or a review artifact. |
| 5. Human review | Force review of assumptions, boundaries, evidence, and proposed primitive mappings. | `mdp-pack-review` | Review packet with accept/change/block decisions. |
| 6. Validation/evals | Run structural checks, strict checks when available, and profile activation gates when implemented. | `mdp validate`, `mdp eval`, `mdp-pack-eval` | Validation/eval result and activation readiness. |
| 7. Activation handoff | State what is safe to use, what remains draft, and which jobs are blocked. | Future profile gates | Bounded activation summary and next implementation work. |

### Existing Skill Reuse

The future `mdp-profile-builder` skill should not duplicate current skill bodies.
It should coordinate them with stricter phase boundaries:

| Existing skill | Keep doing | Add profile-builder responsibility later |
|---|---|---|
| `mdp-source-extract` | Convert supplied or public sources into evidence-backed entries and explicit gaps. | Require primitive labels, source freshness, confidence, privacy notes, and profile relevance in the extraction review artifact. |
| `mdp-icp-builder` | Codify GTM ICP, personas, fit, signals, pains, and no-message logic. | For GTM profiles, normalize account context, persona/actor context, and relationship context together instead of collapsing company info into `fit-rules`. |
| `mdp-create-pack` | Create or improve a GTM-shaped `.mdp/` pack in slices. | Use profile metadata, `primitive_map`, input contracts, jobs, and categorized evals only after MDP-40 implementation support exists. |
| `mdp-pack-review` | Audit pack structure, routing, ICP clarity, evidence, CTAs, avoid-rules, output rules, duplication, claims, and gaps. | Add profile review: primitive coverage, mapped references, profile boundary, activation blockers, and human acceptance state. |
| `mdp-pack-eval` | Test routing and sample cases across personas, jobs, channels, prospect rows, and copy tasks. | Add profile eval categories from MDP-40 and account-context cases from MDP-50. |
| `mdp` | Use the CLI before reading YAML manually; validate, route, fit, claims, gaps, evals, and briefs. | Treat profile activation readiness as separate from structural validity when the CLI exposes it. |

### Universal Primitive Extraction

Every profile-builder pass should produce a table like this before editing pack files:

| Primitive | Builder question | Review gate |
|---|---|---|
| `actors` | Who or what organization, role, account, reviewer, owner, or recipient is involved? | Names are source-backed or clearly marked as missing/synthetic. |
| `decision-criteria` | What rules decide proceed, pause, refuse, escalate, or ask for more context? | Rules are concrete enough for an agent to apply and do not hide disqualifiers. |
| `source-signals` | Which facts matter, where did they come from, and how fresh/confident are they? | Source ledger includes provenance, freshness, confidence, and gaps. |
| `needs-requirements` | What requirement, problem, external criterion, or user need must be satisfied? | Requirements are not inferred from vague positioning unless marked as assumptions. |
| `evidence-proof` | Which approved claims, examples, references, or proof may support outputs? | Claim-bearing content has evidence or becomes a gap/avoid-rule. |
| `boundaries` | What must the agent not say, infer, promise, or do? | Execution-platform asks and unsupported claims are explicit refusal boundaries. |
| `output-contracts` | What shape may outputs take, and what formatting/style/structure constraints apply? | Output rules are deterministic where possible and separated from examples. |
| `routing-jobs` | Which named work modes select context and outputs? | Jobs declare required primitives and blocked modes. |
| `gaps` | What is missing, weak, stale, or review-needed? | Gaps remain visible and are not converted into confident copy. |
| `evals` | Which fixtures prove good, insufficient, refusal, unsafe, and routing cases? | Eval plan covers MDP-40 minimum categories before activation. |

### Company And Account Context

Company information is not one primitive.
In the GTM profile, account/company context maps across multiple primitives:

| Company/account information | Primary primitive | Notes |
|---|---|---|
| Company identity, account name, domain, segment, and organization being evaluated | `actors` | Organizations are actors alongside people/personas. |
| Website facts, hiring/funding/product/stack clues, row fields, source notes, confidence, and freshness | `source-signals` | These are source-backed account signals, not CRM ownership. |
| Target account problems, account-level requirements, adoption triggers, and evaluation needs | `needs-requirements` | Only when the source supports the need or the user labels it as an assumption. |
| Approved account-level claims, case studies, references, and proof points | `evidence-proof` | Claim use still needs source/proof boundaries. |
| Disqualifiers, no-message cases, unsupported account assumptions, and no-invented-contact rules | `decision-criteria` and `boundaries` | Fit gates run after account/person/relationship context is normalized. |
| Missing domain, missing company proof, account-only row with no person, weak segment, or stale signal | `gaps` | Account-only input can be planning context but should not produce a prospect draft under the current `prospect` contract. |

The builder must preserve MDP-50's extraction order:
account context first, source signals and gaps second, persona/actor routes third, prompt contracts fourth, and fit/readiness gates fifth.

### Draft Pack Assembly

Pack/profile assembly should happen in reviewable slices, not one large rewrite.

For current GTM packs:

1. Source ledger, positioning, fit-rules, claims, gaps, and prospect normalization.
2. Personas/actors, signals, pains/needs, and motions/jobs.
3. Channel policies, CTAs, output rules, copy patterns, objections, and eval fixtures.

For future domain profiles:

1. Profile boundary, profile vocabulary, required primitives, and `primitive_map`.
2. Domain-native card IDs and file paths using fixed core `CardKind` families.
3. Input contracts and prompts for messy source normalization.
4. Jobs with required primitives.
5. Minimum eval categories and profile-specific fixtures.

Do not add profile metadata to templates until MDP-40 validation support exists.
Do not add arbitrary custom `kind` strings.
Do not add an `account-context` core card kind.

### Human Review Gate

The builder should create a review packet before claiming a profile is usable.
The packet should include:

- source inventory and what was excluded for privacy or access reasons;
- primitive coverage table with confidence and source references;
- assumptions that need approval;
- card/job/prompt/eval patches proposed or already written;
- boundaries and refusal cases;
- gaps that block activation;
- eval category plan;
- validation/eval command results;
- explicit "safe to use" and "not safe yet" statements.

Human review can accept a draft profile, request changes, or block activation.
Until acceptance, the profile is draft context only.

### Activation Gate

Activation depends on future MDP-40 implementation support.
The profile-builder workflow should prepare for this shape:

```text
ordinary validate:
  structural errors fail; warnings visible

strict validate/eval:
  warnings fail review/CI

profile activation:
  required primitive coverage, mapped references, jobs, input contracts,
  prompt contracts, gaps, and minimum eval categories must be satisfied
```

The skill should never say a profile is activated just because files were created.
It should say one of:

- structurally valid, activation not supported by current CLI;
- structurally valid, activation blocked by named gaps;
- structurally valid and activation-ready according to the CLI;
- structurally invalid and blocked.

---

## Planning Contract

### Key Decisions

- KTD1. Add a future `mdp-profile-builder` skill as an orchestrator, not as a replacement for current MDP skills.
- KTD2. Route fuzzy "build a domain profile" asks to the profile-builder workflow; route ordinary GTM pack creation to `mdp-create-pack`.
- KTD3. Keep `mdp-create-pack` GTM-compatible and update it later only where profile metadata and account-context behavior are implemented.
- KTD4. The profile builder must output a source-ledgered review packet before durable pack edits are treated as authoritative.
- KTD5. Primitive extraction must happen before card writing, because profile vocabulary maps to universal primitives through `primitive_map`.
- KTD6. Company/account information maps across primitives and must not collapse into `fit-rules`.
- KTD7. Human review is required before AI-extracted judgment can become accepted pack guidance.
- KTD8. Profile activation requires validation/eval gates from MDP-40 once implemented.
- KTD9. Existing packs without profile metadata remain valid and should not be forced through profile-builder behavior.
- KTD10. The builder refuses execution-platform scope inside MDP and records those requests as boundaries or external handoffs.

### Future Skill Routing

The eventual plugin routing should look like this:

| User intent | Skill route |
|---|---|
| "Create a GTM message pack from these notes" | `mdp-create-pack` |
| "Improve the ICP / fit / persona logic" | `mdp-icp-builder` |
| "Extract these docs into pack entries" | `mdp-source-extract` |
| "Review this pack" | `mdp-pack-review` |
| "Test routing/evals" | `mdp-pack-eval` |
| "Create a new domain profile / profile-aware pack" | future `mdp-profile-builder` |
| "Turn this into a CRM/scraper/sender/sequencer" | refuse inside MDP; offer external handoff only with explicit approval |

### Review Packet Shape

The implementation should use a repo-ignored scratch location for any raw review packet, source extraction JSON, validation output, or eval output.
Do not commit raw private source material.
Commit only sanitized examples, docs, template changes, or skill instructions.

The review packet should be concise enough for a human to accept or reject:

```markdown
# Profile Builder Review: <profile id>

## Decision Needed
Accept / change / block.

## Sources
Source IDs, confidence, freshness, and excluded private material.

## Primitive Coverage
Coverage table with mapped cards, prompts, input contracts, jobs, evals, and gaps.

## Proposed Pack Changes
Files and high-level edits.

## Boundaries
Unsupported claims, execution asks, privacy constraints, and refusal rules.

## Validation
Commands run and results.

## Activation Status
Not supported / blocked / ready.
```

### Implementation Surface For Follow-Up Issues

| Surface | Future change | Notes |
|---|---|---|
| `plugin/skills/mdp-profile-builder/SKILL.md` | Add the coordinator skill after profile metadata and activation gates are usable enough to guide behavior. | New skill, but it should delegate by phase to existing skills. |
| `plugin/skills/mdp-lfg/SKILL.md` | Route profile/domain-profile asks to `mdp-profile-builder`. | Keep ordinary GTM pack creation routed to `mdp-create-pack`. |
| `plugin/skills/mdp-source-extract/SKILL.md` | Add primitive-label, privacy, confidence, freshness, and review-packet expectations. | Same behavior PR as the profile-builder skill. |
| `plugin/skills/mdp-icp-builder/SKILL.md` | Clarify account/person/relationship context and avoid collapsing company info into fit rules. | Coordinate with MDP-50 behavior changes. |
| `plugin/skills/mdp-create-pack/SKILL.md` | Add profile-aware slices after manifest/profile support lands. | Do not add unsupported metadata early. |
| `plugin/skills/mdp-pack-review/SKILL.md` | Add profile coverage, primitive_map, activation blockers, and human-review checks. | Should use `--strict` and activation summaries when available. |
| `plugin/skills/mdp-pack-eval/SKILL.md` | Add MDP-40 profile eval categories and account-context eval cases. | Keep existing GTM eval behavior unchanged. |
| `plugin/skills/mdp/SKILL.md` | Document activation readiness as separate from structural validity when CLI support exists. | Preserve MDP boundary language. |
| `plugin/assets/templates/basic/.mdp/manifest.yaml` | Add GTM profile metadata only after validator support lands. | No custom core card kinds. |
| `plugin/assets/templates/basic/.mdp/evals/*.yaml` | Add categorized `profile_eval` metadata and account-context fixtures after CLI support lands. | Synthetic/sanitized fixtures only. |
| Public docs and README skill list | Mention `mdp-profile-builder` only when the skill exists. | Planning artifact alone does not require public docs changes. |

### Test Strategy For Future Implementation

After implementing profile-builder skill behavior, run:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate --strict --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/basic
make validate
```

Skill-level manual checks:

- fuzzy domain-profile asks route to `mdp-profile-builder`;
- ordinary GTM pack creation still routes to `mdp-create-pack`;
- source extraction produces primitive coverage and gaps before card edits;
- account-only company input does not invent a person or draft;
- review packet blocks activation until human review and CLI gates pass;
- execution-platform asks are refused or handed off outside MDP with explicit approval.

Planning-only changes require document review and diff hygiene only.

### Sequencing

1. Implement MDP-40 validation/eval gate behavior first.
2. Add profile manifest structs, schemas, and strict/activation summaries.
3. Add GTM profile metadata and categorized eval fixtures only after validation supports them.
4. Update account-context prompt/template/skill behavior from MDP-50.
5. Add `mdp-profile-builder` as the orchestration skill.
6. Update `mdp-lfg` and existing skills to route to the new coordinator where appropriate.
7. Use MDP-42 Proposal AI Lab as the first reference-profile proving ground.
8. Leave MDP-43 parked unless accepted behavior breaks `mdp.v0` or requires custom card kinds.

---

## Sources

- MDP-37/MDP-38 foundation artifact: `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- MDP-39 accepted plan: `docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md`.
- MDP-50 accepted plan: `docs/plans/2026-07-01-003-docs-account-context-icp-normalization-plan.md`.
- MDP-40 accepted plan: `docs/plans/2026-07-01-004-docs-profile-validation-eval-gates-plan.md`.
- Linear issue: MDP-41.
- Current plugin skills: `plugin/skills/mdp/SKILL.md`, `plugin/skills/mdp-create-pack/SKILL.md`, `plugin/skills/mdp-source-extract/SKILL.md`, `plugin/skills/mdp-icp-builder/SKILL.md`, `plugin/skills/mdp-pack-review/SKILL.md`, `plugin/skills/mdp-pack-eval/SKILL.md`.
- Current template surfaces: `plugin/assets/templates/basic/.mdp/manifest.yaml`, `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml`.
- Missing source note: Linear references "AI Pack Builder Workflow", but `rg` and Linear search did not find a matching source during this planning pass.
