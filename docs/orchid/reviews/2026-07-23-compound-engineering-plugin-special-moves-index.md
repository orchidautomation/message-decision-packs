---
title: Compound Engineering Plugin Special Moves Index
date: 2026-07-23
source_repo: https://github.com/EveryInc/compound-engineering-plugin
source_release: https://github.com/EveryInc/compound-engineering-plugin/releases/tag/compound-engineering-v3.20.0
source_tag: compound-engineering-v3.20.0
source_commit: 5c7cb347d0686663743b87cd7227246ba24f7fa7
artifact_type: external-plugin-pattern-index
---

# Compound Engineering Plugin Special Moves Index

## Purpose

This is a pattern index of the actual [`EveryInc/compound-engineering-plugin`](https://github.com/EveryInc/compound-engineering-plugin) at release [`compound-engineering-v3.20.0`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0). It breaks apart what they do that is special: product primitives, runtime skill patterns, workflow mechanics, packaging strategy, evaluation methods, and safety/authority moves.

Use this as a field guide for studying them and deciding what is worth adapting to MDP.

## Fast map: what is special at a glance

| Layer | Special thing | Why it is special |
|---|---|---|
| Product philosophy | Compounding loop | Every run is meant to make later runs easier through artifacts and learnings. |
| Skill architecture | Skills as contracts | A skill is not a prompt; it is activation, authority, workflow, references, scripts, output contract, and fallback behavior. |
| Context design | Artifacts as APIs | Plans, requirements, handoffs, PR bodies, receipts, and solution docs carry state between agents. |
| Planning | Requirements/plans with IDs | Stable identifiers let downstream agents cite decisions, requirements, acceptance examples, and units. |
| Review | Findings as structured evidence | Findings have severity, confidence, action class, provenance, owner, and routing. |
| Autonomy | Claim-act-confirm state loops | Autonomous watchers do not mark work done merely because they saw it; completion requires evidence. |
| Cross-model | Requested vs verified identity | They distinguish what model was requested from what actually served, and only trust receipts. |
| Distribution | Author once, adapt explicitly | One skill tree is packaged across many harnesses with platform-specific manifests and writers. |
| Testing | Mechanical contracts in CI | Stable strings, schemas, path safety, manifests, and helper scripts are tested deterministically. |
| Learning | `docs/solutions/` | Solved problems become reusable knowledge that future skills read. |

## 1. Product-level special moves

### 1.1 The compounding workflow loop

**Where:** [`README.md`](https://github.com/EveryInc/compound-engineering-plugin/blob/compound-engineering-v3.20.0/README.md), [`docs/skills/README.md`](https://github.com/EveryInc/compound-engineering-plugin/blob/compound-engineering-v3.20.0/docs/skills/README.md)

They define a full engineering loop: strategy -> ideate -> brainstorm -> plan -> work -> simplify/review -> commit/PR -> babysit -> compound. The unusual part is the final arrow back into future work. Learning capture is not a nice-to-have; it is the reason the system compounds.

**Why it matters:** most agent workflows optimize one task. CE optimizes a sequence of tasks over time.

**MDP translation:** pack creation/review should leave reusable context-pattern learnings, not just a generated pack.

### 1.2 Skill family as product surface

**Where:** [`skills/`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills)

They ship 31 skills, each owning a distinct job. The plugin is not one giant assistant. It is a vocabulary of workflow stages and tools.

**Why it matters:** the user can invoke the workflow at the right altitude: idea, plan, debug, review, PR, handoff, learning, etc.

**MDP translation:** define only distinct MDP jobs as skills: source extract, pack build, pack review, copy eval, proposal compliance check, etc.

### 1.3 Phase separation as a safety mechanism

**Where:** `ce-ideate`, `ce-brainstorm`, `ce-plan`, `ce-work`, `ce-code-review`

They deliberately separate discovery, scoping, planning, implementation, and review. Each phase has its own artifact and decision boundary.

**Why it matters:** it prevents one agent from silently deciding product scope while writing code, or inventing implementation details while brainstorming requirements.

**MDP translation:** keep context/profile decisions separate from message writing and message evaluation.

### 1.4 Human learning is preserved

**Where:** [`ce-explain`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills/ce-explain)

They recognize that agent-written code can make humans learn less. `ce-explain` creates a teaching artifact and optional active-recall check-in.

**Why it matters:** it treats developer education as part of the workflow, not an accidental byproduct.

**MDP translation:** after complex pack/copy decisions, add compact “why this angle works” explainers so operators build judgment.

## 2. Skill-authoring special moves

### 2.1 Outcome spine before process

**Where:** [`docs/solutions/skill-design/portable-agent-skill-authoring.md`](https://github.com/EveryInc/compound-engineering-plugin/blob/compound-engineering-v3.20.0/docs/solutions/skill-design/portable-agent-skill-authoring.md)

Their authoring guide asks: what result is produced, who consumes it, what is done, and why does this skill exist? Only then add workflow detail.

**Why it matters:** skills avoid becoming long “be thorough” prompts with no observable contract.

### 2.2 Protocol vs judgment split

**Where:** same guide plus most mature `SKILL.md` files

They encode stable fields, gates, enums, and failure behavior as protocol. They leave synthesis and tradeoff judgment to the model.

**Why it matters:** it gives weaker/different models a floor while preserving high-end model reasoning.

### 2.3 Capability before tool names

**Where:** `AGENTS.md`, portable authoring guide, cross-harness references

They describe the capability needed, then name tools as adapters. A missing binary is not automatically proof the capability is impossible.

**Why it matters:** this is how one authored skill can run in Claude, Codex, Grok, Cline, Devin, etc.

### 2.4 Runtime reference loading

**Where:** `skills/*/references`

Large or conditional details live in references loaded only at the point of need. The top-level skill keeps route and gates inline.

**Why it matters:** reduces token load and keeps skill bodies reviewable.

### 2.5 Skill-local specialist prompts instead of public agents

**Where:** `skills/*/references/agents`, `skills/*/references/personas`

They no longer expose standalone agents. Skills seed generic subagents with local specialist prompt assets.

**Why it matters:** specialist personas become implementation details controlled by the workflow that needs them.

### 2.6 Manual-only flags for risky workflows

**Where:** frontmatter in `ce-dogfood`, `ce-polish`, `ce-product-pulse`, `ce-promote`, `ce-setup`, `ce-sweep`, `ce-test-xcode`

They mark powerful or user-owned workflows as manual-only.

**Why it matters:** descriptions alone are not enough to prevent accidental auto-invocation.

### 2.7 Stable invocation rendering by harness

**Where:** release fixes, `AGENTS.md`, skill docs

They account for different invocation syntax across hosts (`/skill`, `$skill`, plugin namespaces). User-facing command rendering is treated as an output seam.

**Why it matters:** cross-harness polish requires small details like this.

## 3. Artifact-system special moves

### 3.1 Requirements-only plan before implementation plan

**Where:** [`ce-brainstorm`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills/ce-brainstorm)

Brainstorming produces a product/requirements artifact, not an implementation plan by default.

**Why it matters:** it avoids premature architecture and preserves product intent.

### 3.2 Plans as downstream contracts

**Where:** [`ce-plan`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills/ce-plan)

Plans include stable units, verification scenarios, requirements links, decisions, scope boundaries, risks, and handoff guidance.

**Why it matters:** a fresh implementation or review agent can consume the plan without reconstructing context.

### 3.3 Stable IDs across workflow stages

**Where:** `ce-brainstorm`, `ce-plan`, `ce-work`, `ce-code-review`

They use IDs for requirements, assumptions, flows, acceptance examples, and implementation units.

**Why it matters:** downstream skills can cite exactly what they satisfy or challenge.

### 3.4 Session-settled decision annotations

**Where:** `ce-brainstorm/references/settled-decisions.md`, `ce-plan/references/settled-decisions.md`, `lfg`, `ce-work`, `ce-code-review`

Decisions examined and chosen in the conversation are carried forward with visible provenance. Later stages should not casually re-litigate them.

**Why it matters:** it solves the “fresh agent reopens settled choices” failure mode.

**MDP translation:** mark user-approved messaging decisions inside packs so later copy/review skills know what is settled vs inferred.

### 3.5 Handoff as immutable continuity artifact

**Where:** [`ce-handoff`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills/ce-handoff)

Handoffs point to authoritative artifacts and current state rather than copying whole transcripts.

**Why it matters:** fresh sessions get enough continuity without treating raw conversation as durable source of truth.

### 3.6 PR descriptions as teaching artifacts

**Where:** [`ce-commit-push-pr`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills/ce-commit-push-pr)

PR bodies scale with review decision cost and can teach newly introduced concepts.

**Why it matters:** reviewer comprehension is treated as part of shipping.

### 3.7 Solution docs as compounding memory

**Where:** [`ce-compound`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/skills/ce-compound), [`docs/solutions/`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/docs/solutions)

Solved problems become structured solution docs with metadata and discoverability checks.

**Why it matters:** future agents can find and reuse prior decisions/patterns.

## 4. Planning and discovery special moves

### 4.1 Evidence-first ideation

**Where:** `ce-ideate`

Ideas require grounding: code, docs, learnings, web/prior art, issue trackers, or explicit reasoning.

**Why it matters:** it attacks “AI slop” directly.

### 4.2 Basis tags for every idea

**Where:** `ce-ideate`

Each idea needs a basis category: direct evidence, external prior art, or reasoned argument.

**Why it matters:** ideas become auditable rather than merely plausible.

### 4.3 Six-frame divergent generation

**Where:** `ce-ideate`

They use multiple conceptual lenses to prevent one model path from dominating.

**Why it matters:** encourages breadth and non-obvious candidates.

### 4.4 Topic-axis decomposition

**Where:** `ce-ideate`

The topic is decomposed into axes that ideation agents must cover.

**Why it matters:** prevents every generated idea from clustering around the most obvious part of the topic.

### 4.5 Rejection summary

**Where:** `ce-ideate`

Rejected candidates are summarized with reasons.

**Why it matters:** users see what was considered and why it failed.

### 4.6 Product Pressure Test

**Where:** `ce-brainstorm`

They run named gap lenses before writing requirements.

**Why it matters:** the requirements artifact is stress-tested, not just transcribed.

### 4.7 One-question-at-a-time interaction

**Where:** `ce-brainstorm`, `ce-plan`, `ce-pov`

They bias toward blocking one important ambiguity at a time.

**Why it matters:** reduces branching confusion and respects user attention.

### 4.8 Synthesis Summary checkpoint

**Where:** `ce-brainstorm`

They summarize before locking the requirements artifact.

**Why it matters:** this is the last cheap moment for the user to correct trajectory.

### 4.9 Confidence check and deepening

**Where:** `ce-plan`

After writing a plan, the skill evaluates confidence and can deepen thin sections.

**Why it matters:** planning quality is checked before implementation consumes it.

### 4.10 Approach altitude

**Where:** `ce-plan`

For hard deliverables, the skill can produce a plan for how to approach making the plan/deliverable.

**Why it matters:** it avoids false precision when the method itself is unknown.

## 5. Review special moves

### 5.1 Diff-aware reviewer selection

**Where:** `ce-code-review`

The review roster changes based on diff size, files, risks, and plan context.

**Why it matters:** avoids wasting effort on trivial diffs while still covering risky ones.

### 5.2 Orthogonal severity and autofix class

**Where:** `ce-code-review`

A finding’s severity and safe-fix category are separate.

**Why it matters:** a low-severity finding might be easy to auto-fix; a high-severity one might require human judgment.

### 5.3 Report authority separate from apply authority

**Where:** `ce-code-review`, `ce-doc-review`

Review can report findings without permission to edit.

**Why it matters:** prevents review workflows from silently mutating code/docs.

### 5.4 Plan discovery during code review

**Where:** `ce-code-review`

The reviewer tries to find a plan so it can review against intended requirements, not just code taste.

**Why it matters:** code review becomes intent-aware.

### 5.5 Findings must be self-contained

**Where:** `ce-code-review`, `ce-doc-review`, release fixes

Findings include enough evidence/provenance for a reader without the full document or transcript.

**Why it matters:** review output survives handoff.

### 5.6 Decision primer for doc review

**Where:** `ce-doc-review`

The doc-review process suppresses repeated rounds on already-decided issues.

**Why it matters:** review does not become an endless loop of the same feedback.

### 5.7 Bulk-action preview

**Where:** `ce-doc-review`

Before mass doc changes, the user sees a preview.

**Why it matters:** safe apply behavior for high-volume edits.

### 5.8 Residual Work Gate

**Where:** `ce-code-review`

Unresolved findings can become explicit residual work rather than silent “done.”

**Why it matters:** shipping decisions preserve known gaps.

### 5.9 Settled-decision triage

**Where:** `ce-code-review`

A preference against a settled decision is treated differently from a real defect.

**Why it matters:** avoids review agents undoing user-approved choices while still surfacing bugs.

## 6. Execution and autonomy special moves

### 6.1 Plan-aware execution

**Where:** `ce-work`

`ce-work` treats the plan as guardrails and works out implementation details with the code in front of it.

**Why it matters:** it honors the WHAT/HOW boundary.

### 6.2 Idempotent re-execution

**Where:** `ce-work`

Re-running should detect completed/partial work rather than blindly duplicate effort.

**Why it matters:** critical for interrupted agent work.

### 6.3 Engine/workspace/scheduling separation

**Where:** `ce-work`

Implementation engine, workspace isolation, and scheduling strategy are separate decisions.

**Why it matters:** avoids conflating “use another model” with “must use another worktree” or “run in parallel.”

### 6.4 Cross-model implementation with host-owned integration

**Where:** `ce-work/references/cross-model-execution.md`

External workers can author bounded units; the host owns canonical verification, integration, commits, and shipping.

**Why it matters:** this is the safest architecture for external write delegation.

### 6.5 Unit packets instead of whole-context egress

**Where:** `ce-work`

External workers receive bounded packets for one implementation unit, not the whole conversation or plan by default.

**Why it matters:** reduces context leakage and scope drift.

### 6.6 Synthetic transport commits

**Where:** `ce-work` scripts/references

External output is terminalized as an inspectable Git change set before host integration.

**Why it matters:** worker prose is not accepted as proof of work.

### 6.7 Transactional fold-in

**Where:** `ce-work/scripts/unit_workspace_transaction.py`

Canonical mutation is guarded by verification and rollback/restore semantics.

**Why it matters:** external output cannot simply dirty the main checkout and call itself done.

### 6.8 Comments-before-CI in PR watching

**Where:** `ce-babysit-pr`

Review feedback is handled before CI fixes, then state is rechecked so stale CI failures are not fixed against a dead SHA.

**Why it matters:** collapses async review/CI timelines.

### 6.9 Stateless resumable PR ticks

**Where:** `ce-babysit-pr/scripts/pr-snapshot`

Watch state lives on disk; each tick can be resumed by any driver.

**Why it matters:** survives harness limitations and session interruptions.

### 6.10 Claim-act-confirm dedup

**Where:** `ce-babysit-pr/references/watch-loop.md`

An item is not marked handled just because it was observed. It is silenced only after an action or remote truth confirms it.

**Why it matters:** prevents crashes from losing work.

### 6.11 Quiet-time readiness

**Where:** `ce-babysit-pr`

A PR must be quiet long enough before it “looks ready.” Stalled bot/reviewer signals are bounded.

**Why it matters:** avoids “green now, surprise feedback later.”

### 6.12 Branch currency with intent preservation

**Where:** `ce-babysit-pr`

Base-branch movement is treated as its own attention stream, and conflicts are resolved only when mechanical intent is clear.

**Why it matters:** being mergeable is not enough; the update must preserve intended behavior.

### 6.13 Full autonomous pipeline to PR

**Where:** `lfg`

`lfg` chains planning, implementation, simplification, review, browser testing, commit/PR, and PR babysitting.

**Why it matters:** it shows how individual skills compose into an autonomous workflow while preserving stage responsibilities.

## 7. Cross-model special moves

### 7.1 Model elevation for reasoning-heavy stages

**Where:** `ce-plan/references/reasoning-elevation.md`, `ce-brainstorm/references/reasoning-elevation.md`

Planning/brainstorming can route heavy reasoning to a chosen model/harness.

**Why it matters:** uses stronger/different reasoning without changing the workflow contract.

### 7.2 Cross-model adversarial review

**Where:** `ce-code-review/references/cross-model-review.md`

A separate provider/model can run an adversarial review pass.

**Why it matters:** diversity improves review when independence is real.

### 7.3 Cross-model doc judgment

**Where:** `ce-doc-review/references/cross-model-review.md`

Plans/specs can receive cross-model judgment, not just code.

**Why it matters:** upstream artifacts get adversarial scrutiny too.

### 7.4 Project-grounded model panels

**Where:** `ce-pov/references/cross-model-panel.md`

`ce-pov` can consult peers/oracle panels that independently inspect project context.

**Why it matters:** panels are not abstract debate; they are repository-grounded.

### 7.5 Requested vs actual model receipts

**Where:** `CONCEPTS.md`, cross-model references, solution docs

They explicitly separate requested model, actual model, route, intermediary, and receipt status.

**Why it matters:** without receipts, “I asked another model” is not evidence of independent review.

### 7.6 Cross-model failures degrade explicitly

**Where:** cross-model review/work scripts and references

If a peer route fails, the skill reports degradation instead of pretending the panel ran.

**Why it matters:** missing peer evidence should lower confidence visibly.

## 8. Packaging and platform special moves

### 8.1 Root-native plugin layout

**Where:** root `plugin.json`, `.codex-plugin/plugin.json`, `.kimi-plugin/plugin.json`, `.grok-plugin/plugin.json`, `.devin-plugin/plugin.json`

Where platforms support it, they point directly at the canonical `skills/` tree.

**Why it matters:** fewer generated copies and less drift.

### 8.2 Many manifests, one source

**Where:** platform metadata directories

They keep platform-specific manifests while preserving one canonical authored skill tree.

**Why it matters:** realistic cross-platform support needs explicit metadata per host.

### 8.3 Empirical target specs

**Where:** [`docs/specs/`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/docs/specs)

They document what each harness actually accepts, sometimes based on CLI probing.

**Why it matters:** agent platform docs are often incomplete or changing.

### 8.4 Converter/writer architecture

**Where:** [`src/converters`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/src/converters), [`src/targets`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/src/targets)

Claude-style content is parsed once, converted explicitly, and written by target-specific writers.

**Why it matters:** platform differences are localized.

### 8.5 Managed install manifests

**Where:** target writers/tests

Generated installs track what the tool wrote so future installs can clean/update without clobbering user content.

**Why it matters:** plugin installers must be safe in user config directories.

### 8.6 Symlink and unmanaged-path preservation

**Where:** writer tests, release fixes

User-managed symlinks and non-owned paths are preserved on install/update.

**Why it matters:** respects advanced user customization and prevents destructive upgrades.

### 8.7 Local Codex development workflow

**Where:** `scripts/codex-dev.ts`, docs/solutions/developer-experience

They provide a workflow to link a worktree’s live skills into Codex for development while removing shadowing plugin installs.

**Why it matters:** plugin caching makes skill iteration tricky; they built tooling for it.

## 9. Testing/eval special moves

### 9.1 Deterministic contracts in CI

**Where:** [`tests/`](https://github.com/EveryInc/compound-engineering-plugin/tree/compound-engineering-v3.20.0/tests)

They test what can be tested: schemas, converters, manifests, stable strings, path safety, helper scripts.

**Why it matters:** prompt-heavy repos still need serious CI.

### 9.2 Behavioral evals are separate from mechanical tests

**Where:** `AGENTS.md`, solution docs

They explicitly distinguish deterministic CI from LLM behavior eval evidence.

**Why it matters:** avoids false confidence from brittle string tests and avoids ignoring behavior entirely.

### 9.3 Parity tests for shared references

**Where:** tests for settled decisions, reasoning elevation, cross-model receipts

Shared runtime contracts copied across skills are parity-tested.

**Why it matters:** when two skills depend on identical semantics, drift is a bug.

### 9.4 Script-first validation for state machines

**Where:** `ce-babysit-pr-snapshot.test.ts`, sweep-state tests, peer-job-runner tests

Stateful behavior is pushed into scripts where it can be tested.

**Why it matters:** model instructions are poor state machines; scripts are better.

### 9.5 Red-team style eval concepts

**Where:** solution docs for paired old-vs-new evals, fake CLI harnesses, benchmark peer models

They use targeted eval patterns to prove a skill prose change actually changed behavior.

**Why it matters:** skill edits should be validated against the failure they claim to fix.

## 10. Release and maintenance special moves

### 10.1 Release Please multi-component setup

**Where:** `.github/release-please-config.json`, `src/release`

The root plugin and marketplaces are release components with synced versions/metadata.

**Why it matters:** plugin manifests drift easily without automation.

### 10.2 Release metadata validation

**Where:** `scripts/release/validate.ts`, `src/release/metadata.ts`

Validation checks versions, descriptions, marketplace plugin lists, skill path declarations, and platform parity.

**Why it matters:** packaging correctness is part of CI.

### 10.3 Strict plugin schema validation

**Where:** `package.json` scripts, GitHub CI

They validate Claude marketplace and plugin schema strictly.

**Why it matters:** catches schema drift before release.

### 10.4 Stale artifact cleanup registries

**Where:** `src/data/plugin-legacy-artifacts.ts`, cleanup code/tests

Removed/renamed generated artifacts are tracked so upgrades can clean stale copies.

**Why it matters:** otherwise users keep dead skills/commands after updates.

### 10.5 Contributor rules as runtime-protection memory

**Where:** `AGENTS.md`

Repo instructions encode lessons about authoring, release, testing, scratch, plugin validation, and skill changes.

**Why it matters:** the project teaches future agents how to work on the project.

## 11. Safety and authority special moves

### 11.1 Local scratch with ownership checks

**Where:** `AGENTS.md`, solution docs

Predictable `/tmp` roots are user-scoped and ownership-checked.

**Why it matters:** shared temp directories can be prompt-injection or state-poisoning surfaces.

### 11.2 External egress sanctioning

**Where:** `ce-work`, cross-model references

Before repo material leaves to an external route, the skill records route, recipient, material, restrictions, and authority.

**Why it matters:** cross-model work is treated as an egress/trust boundary.

### 11.3 Host-owned canonical commits

**Where:** `ce-work`

External workers never own the canonical commit. The host verifies and commits.

**Why it matters:** keeps final authority in the invoking environment.

### 11.4 Needs-human as first-class outcome

**Where:** `ce-babysit-pr`, `ce-resolve-pr-feedback`, `ce-doc-review`

Ambiguous or semantic decisions are parked for a human rather than forced.

**Why it matters:** autonomy is safer when “stop and ask” is a designed outcome.

### 11.5 Read-only invariant for pulse/POV-style skills

**Where:** `ce-product-pulse`, `ce-pov`

Some skills explicitly analyze without mutating.

**Why it matters:** keeps decision support separate from action.

### 11.6 Draft-only promotion

**Where:** `ce-promote`

Promotion copy is drafted but not posted.

**Why it matters:** external communication stays human-owned.

## 12. Per-skill special index

This section indexes each released skill’s distinctive mechanics. The descriptions are paraphrased from the runtime sources and user docs.

### `ce-ideate`

- Grounds before generating ideas.
- Requires a basis for each idea.
- Uses multiple conceptual frames for divergent thinking.
- Decomposes topics into coverage axes.
- Runs adversarial filtering and records rejection reasons.
- Supports software, software-product, and non-software ideation.
- Has a “surprise me” mode where subjects can emerge from grounding.
- Can mine issue trackers for patterns.

### `ce-brainstorm`

- Uses blocking questions one at a time.
- Scales ceremony by work tier.
- Runs named pressure-test lenses.
- Requires non-obvious approach exploration.
- Uses visual probes when seeing beats reading.
- Adds a synthesis checkpoint before writing requirements.
- Emits stable IDs for downstream planning.
- Supports non-software brainstorming.
- Keeps implementation out of the product contract by default.
- Has a blindspot pass for unfamiliar territory.
- Carries session-settled decisions into requirements.

### `ce-plan`

- Writes guardrails instead of implementation choreography.
- Uses stable implementation unit IDs.
- Traces origin IDs from brainstorm artifacts.
- Defines unit-level test scenarios.
- Runs confidence/deepening checks.
- Dispatches multi-agent research in parallel.
- Plans non-software work too.
- Supports approach-altitude planning.
- Carries settled decisions rather than re-asking.

### `ce-work`

- Executes against plan guardrails.
- Re-runs idempotently.
- Separates engine, workspace, and scheduling choices.
- Anchors work to U-IDs.
- Requires test evidence before done.
- Has explicit residual handling after review.
- Runs operational validation by default.
- Triage small bare prompts without forcing a plan.
- Treats settled decisions as not the implementer’s choice to improve.
- Supports cross-model implementation with host-owned integration.

### `ce-code-review`

- Selects reviewers based on diff and risk.
- Adds cross-model adversarial review when appropriate.
- Separates severity from autofix action class.
- Separates report authority from apply authority.
- Has a quick-review short circuit for small diffs.
- Merges/dedupes/promotes/routes findings in synthesis.
- Discovers plans to review against requirements.
- Tracks residual work.
- Protects special artifacts from overzealous edits.
- Triage settled-decision preferences differently from defects.

### `ce-doc-review`

- Chooses personas based on document content.
- Synthesizes findings into decision/action tiers.
- Uses decision primers to avoid repeated review loops.
- Offers controlled interaction modes.
- Shows bulk previews before mass changes.
- Supports interactive and headless modes.
- Bounds parallelism with backpressure.
- Reports coverage transparently.
- Can run cross-model judgment.
- Protects settled decisions.

### `ce-debug`

- Requires causal chain before fixing.
- Uses predictions for uncertain causal links.
- Audits assumptions.
- Escalates intelligently when stuck.
- Reads issue/PR history for prior context.
- Encourages test-first fixes.
- Runs post-fix polish/review tail.
- Adds defense-in-depth only when justified.
- Escalates to brainstorming when a bug reveals a design problem.

### `ce-simplify-code`

- Uses multiple reviewer angles: quality, reuse, efficiency.
- Detects scope from user target, diff, or recent edits.
- Requires behavior-preservation verification.
- Uses cost-aware model tiering.
- Honors caller-passed structure pins.

### `ce-commit`

- Detects commit conventions.
- Avoids blanket `git add -A`.
- Splits logically at file level when needed.
- Handles detached HEAD and default-branch hazards.
- Uses robust multi-line commit messages.
- Focuses subject line on why/value, not file-list summary.

### `ce-commit-push-pr`

- Has separate modes for full flow, PR body update, and description generation.
- Scales PR descriptions with review cost.
- Splits commits when appropriate.
- Uses a branch-state decision tree.
- Writes PR bodies through body files to avoid shell quoting/empty body failures.
- Detects repository conventions.
- Integrates evidence and related references.
- Confirms before rewriting existing PRs.
- Adds concept-teaching when the PR introduces something new.
- Adds settled-decision provenance when a labeled plan exists.

### `ce-babysit-pr`

- Handles comments before CI, then cancels stale CI work.
- Uses a resumable tick instead of one fragile loop.
- Can sustain an in-session watch through background wake when the harness allows.
- Uses quiet-time bounds for unreliable reviewer/bot signals.
- Treats branch currency as intent-preservation, not just mergeability.
- Uses claim-act-confirm for crash-safe dedup.
- Ends with outcome-first summaries.

### `ce-resolve-pr-feedback`

- Defaults toward fixing unless a tripwire says not to.
- Judges comments by merit, not author/source/form.
- Uses multiple verdicts with different actions.
- Distinguishes new feedback from already-handled feedback.
- Centralizes judgment before parallel fixes.
- Avoids file collisions among fixers.
- Runs combined validation after fixes.
- Replies with quoted context.
- Relocates outdated comments when possible.
- Uses a two-pass loop with escalation.
- Supports full and targeted modes.

### `ce-compound`

- Chooses full vs lightweight mode.
- Separates bug-track and knowledge-track learning shapes.
- Detects overlap to update existing docs rather than duplicate.
- Checks discoverability.
- Validates claims against the current tree before compounding.
- Triggers selective refresh when needed.
- Runs specialized post-review.
- Probes session history automatically.
- Defines auto-invoke triggers.

### `ce-compound-refresh`

- Uses explicit outcomes: keep, update, consolidate, replace, delete.
- Supports interactive and headless modes.
- Analyzes document sets, not just individual docs.
- Uses subagents for replacement context isolation.
- Marks stale docs when evidence is insufficient.
- Requires conditions before auto-delete.
- Classifies inbound links as decorative vs substantive.
- Matches docs to current reality, not the other way around.
- Prefers deletion over indefinite archiving when obsolete.

### `ce-pov`

- Requires subject-aware project grounding.
- Confirms ambiguous framing before grounding.
- Grounds against the repository in ways generic research cannot.
- Uses scout dossiers to keep verdict context clean.
- Supports cold and warm invocation.
- Scales effort by reversibility tier.
- Has distinct output contracts for adoption verdicts, document takes, and approach-set positions.
- Supports independent bounded cross-model panels.
- Offers next steps only when shaped by the verdict.

### `ce-explain`

- Creates a durable teaching artifact for the human.
- Supports concept, diff, idea, and recap shapes.
- Can offer active-recall exercises/check-ins.
- Uses predict-then-reveal for diffs when warranted.
- Keeps exercises in session, not embedded in artifacts.

### `ce-handoff`

- Has explicit create and resume directions.
- Uses frontmatter as a discovery index.
- Points to authoritative artifacts instead of copying everything.
- Preserves user control: creating a handoff does not auto-continue; selecting a source does not imply authority.

### `ce-optimize`

- Uses layered evaluation: hard gates, judge, diagnostics.
- Supports LLM-as-judge for qualitative outputs.
- Treats disk artifacts as source of truth.
- Runs parallel experiments in isolated worktrees.
- Can cherry-pick file-disjoint runner-up changes.
- Compresses learnings into strategy digests.
- Supports crash recovery/resume.
- Requires hard gates before later phases.

### `ce-product-pulse`

- Forces a short single-page output.
- Asks founder-style judgment questions rather than only thresholds.
- Seeds setup from strategy.
- Stays read-only.
- Uses trailing buffers for signal completeness.
- Saves PII-free reports.
- Mixes parallel and serial query dispatch.
- Pushes back on weak metrics.
- Supports disciplined sample-quality scoring.
- Builds memory through saved reports.

### `ce-sweep`

- Sweeps feedback sources with cursors/state.
- Acknowledges source items explicitly.
- Uses leases to avoid concurrent writer corruption.
- Handles media feedback through analysis.
- Verifies whether fixes actually merged.
- Reconciles findings into an `lfg`-ready plan.
- Supports headless scheduled runs.

### `ce-polish`

- Detects dev server/frameworks automatically.
- Allows launch overrides.
- Hands off to the browser/IDE context.
- Uses conversational iteration rather than checklist review.
- Starts background server with health probes.
- Is manual-only.

### `ce-dogfood`

- Maps flows before testing matrices.
- Judges function and experience.
- Can autonomously fix small issues through a size gate.
- Treats escalation as a valid outcome.
- Is resumable.
- Requires a suite check before ready.

### `ce-test-browser`

- Prefers host-native browser drivers, with portable fallback.
- Maps changed files to routes.
- Supports manual and pipeline modes.
- Detects ports through a cascade.
- Separates browser visibility from orchestration.
- Requires human verification for external flows.
- Can fix or skip failures based on scope.
- Emits structured test summaries.

### `ce-test-xcode`

- Uses XcodeBuildMCP as substrate.
- Runs a structured simulator test flow.
- Requires human verification for platform-specific capabilities.
- Documents known platform limitations.
- Handles fix-now vs skip decisions.
- Emits structured summaries.
- Is explicit/manual by design.

### `ce-worktree`

- Detects existing isolation before creating anything.
- Defers to native worktree tools when available.
- Falls back to portable Git behavior.
- Checks `.gitignore`/safety before creating paths.
- Provides naming guidance for upstream callers.

### `ce-strategy`

- Pushes back during strategy interview.
- Updates durable strategy in place.
- Is read by downstream skills as grounding.
- Uses a compact diagnosis/guiding-policy/action style.
- Bounds section count.
- Tracks staleness in frontmatter.

### `ce-proof`

- Wraps Proof’s agent contract in a CE-friendly workflow.
- Treats one-way publish as primary.
- Manages owner credential lifecycle.
- Uses mutation discipline for hosted docs.
- Makes pull-to-local a separate explicit action.
- Maintains consistent agent identity.

### `ce-promote`

- Uses voice tooling only as an optional enhancement.
- Encodes multi-channel copy gotchas.
- Produces drafts only.
- Focuses on user value rather than implementation internals.

### `ce-riffrec-feedback-analysis`

- Routes by length and intent.
- Keeps raw artifacts private by default.
- Accepts multiple recording/bundle shapes through one analyzer.
- Chooses output location by context.
- Emits structured CE feedback format for extensive analysis.
- Hands extensive analysis into brainstorming rather than skipping scope.
- Loads references lazily.

### `ce-setup`

- Checks optional tool capabilities.
- Bootstraps safe local config.
- Keeps project-local settings gitignored.
- Documents missing capabilities instead of hard failing.

### `lfg`

- Composes the full autonomous pipeline.
- Carries stage-specific routing directives.
- Delegates each stage to the owning skill rather than reimplementing it.
- Runs review/fix/browser-test/commit/PR/babysit tail.
- Offers fresh-session handoff for separately planned follow-up areas.

## 13. Meta-patterns to copy into MDP

### Copy these directly as principles

1. **Artifact contracts beat chat memory.**
2. **Skill frontmatter is routing, not marketing.**
3. **Runtime references should load at the point of use.**
4. **Schemas and stable strings should be tested.**
5. **Host/platform differences deserve explicit docs.**
6. **Stateful workflows belong partly in scripts, not only prompts.**
7. **Public artifacts need explicit safety boundaries.**
8. **Learning docs need refresh/prune workflows.**
9. **User-settled decisions should be visible to later agents.**
10. **Manual-only workflows should be marked, not merely described.**

### Copy only with caution

1. Cross-model write delegation.
2. Autonomous PR/state watchers.
3. Large multi-skill autonomous pipelines.
4. Broad platform support before user demand exists.
5. Very long skill contracts for simple jobs.

### Do not copy into MDP

1. Software implementation execution as a product surface.
2. PR babysitting or CI repair workflows.
3. Generic coding-agent review loops unless scoped to MDP pack/docs changes.
4. Any behavior that makes MDP look like a sequencer, CRM, scraper, enrichment provider, or generic automation system.

## Closing read

Compound Engineering is special because its creators are doing all the unglamorous engineering around agent workflows:

- stable artifacts;
- skill boundaries;
- release validation;
- platform manifests;
- state machines;
- testable helper scripts;
- review schemas;
- cross-model receipts;
- compounding memory;
- conservative authority;
- lots of written product decisions.

That is the real frontier. The prompts matter, but the system around the prompts matters more.
