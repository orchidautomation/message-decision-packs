# MDP-339 — Executive Decision Card Projection Plan

**Date:** 2026-09-02  
**Issue:** MDP-339  
**Repository:** `orchidautomation/message-decision-packs`  
**Base branch:** `main`  
**Planning baseline:** `8d00027723f1e26d07e0653b83d39f55fdb3ef27` (`v0.1.110`)  
**Risk:** Elevated — public CLI/schema behavior, privacy-safe evidence projection, and decision-authority presentation  
**Consumer:** A future Orchid implementation session after an explicit implementation start

## 1. Context And Current Behavior

MDP already has two relevant read-only projection surfaces:

1. `mdp trace` projects supported fit, route, brief, validated prompt-output, verified run, and conformance artifacts into `mdp.decision-trace.v1`. `cli/src/commands/decision_trace.rs` owns source parsing, authority state, bounded designed graphs, and observed paths. `cli/src/commands/decision_trace/render.rs` owns bounded Mermaid rendering. The trace deliberately omits raw customer/prospect bodies and remains projection-only.
2. `mdp render-brief` projects selected `mdp.message-brief.v0` and `mdp.proof-output.v0` artifacts into `mdp.human-brief.v0`. `cli/src/commands/human_brief.rs` owns template-specific sections and Markdown rendering. The existing templates are `gtm-prospect`, `proposal-review`, and `proof-report`.

The current operator must still reconcile the authoritative result, trace, evidence receipts, gaps, and action gate across multiple JSON files. The MDP-for-MDP 1Password/Liam Ehrlich dogfood run demonstrated that the underlying fit result and person-specific trace are useful, but the hand-authored `RESULT.md` is not a productized operator surface.

Confirmed repository behavior:

- `DecisionTrace` already distinguishes `designed_graph` from `observed_path` and carries projection authority, source SHA-256, truncation, and limitations.
- `project_fit` exposes exact matched rule IDs and missing/disqualifying reasons without copying the prospect body.
- `render_human_brief_markdown` already provides a safe Markdown serialization pattern, but `mdp.human-brief.v0` does not require a source digest or a canonical trace projection.
- `cli/src/output.rs` is the typed source of truth for `--json` and human-only presentation compatibility. Any new format selector must be added there rather than handled ad hoc.
- `cli/src/commands/capabilities.rs` and `SchemaTarget` expose public command and schema discovery.
- Model/provider selection does not belong in this projection. Existing run request, bundle, audit, receipt, and trace artifacts retain model, prompt, driver, and execution provenance.
- Pack-owned source markers and source-binding adapter metadata remain separate from this work. MDP-339 does not change `source_kind`, Decision Input Contracts, source bindings, or normalization.

## 2. Objective

Add one obvious, schema-backed `mdp decision-card` command that renders a faithful executive review of one supported decision artifact in Markdown or JSON. The card must answer:

1. What subject was evaluated?
2. What was decided and what gate applies?
3. Which exact safe rule/evidence identifiers support the decision?
4. What is missing, conflicting, or limited?
5. What is allowed, blocked, or still human-controlled?
6. Where are the authoritative source and bounded person/record-specific trace?

The card is a read-only operator projection. It never runs policy, invokes a model, mutates a pack, creates evidence, upgrades assurance, or grants drafting/sending authority.

## 3. Scope

### In scope

- Add `mdp decision-card` as a first-class read-only CLI command.
- Add `mdp.decision-card.v1` as a closed, discoverable JSON Schema.
- Accept the same mutually exclusive authority sources as `mdp trace`:
  - `--file` for supported saved fit, route, or brief results;
  - `--file --dir --prompt-output [--validation-input ...]` for an exact validated prompt-output receipt;
  - `--bundle --receipt [--artifact-root]` for verified run authority;
  - `--file --artifact-root` for supported conformance roots.
- Emit Markdown by default and JSON with `--format json` or global `--json`.
- Reuse the canonical `DecisionTrace`; do not implement a second source-resolution or graph algorithm.
- Include bounded sections for subject, decision/gate, classifications when declared, rule matches, accepted evidence IDs/receipts, gaps/limitations, next action, authority, and trace.
- Include either the canonical trace object in JSON or its bounded Mermaid rendering in Markdown.
- Add synthetic fixtures and tests for fit, insufficient-context, disqualified/no-draft, message brief, and verified run states.
- Update capabilities, schema discovery, CLI docs, decision-trace docs, getting started, and the operator runtime skill reference.

### Out of scope

- Changing fit, route, brief, normalization, source-binding, or pack-policy decisions.
- Adding a model/provider default to a pack.
- Exposing chain-of-thought, prompt bodies, raw provider responses, or unrestricted prospect/customer prose.
- Adding arbitrary source-marker values or changing source-kind extensibility.
- Drafting or sending outreach, mutating CRM/HeyReach, publishing, or contacting anyone.
- Hosted dashboards, graph persistence, cross-run analytics, or a new authority store.
- Broad CLI cleanup already owned by MDP-202.
- Changing `mdp.human-brief.v0` or its domain-specific templates unless a shared Markdown helper can be extracted without altering their output.

## 4. Decisions And Assumptions

### D1. Use a first-class command and contract

Implement `mdp decision-card`, not another `render-brief` template. The card consumes the same authority shapes as `trace`, while `render-brief` currently consumes one domain artifact plus a pack directory. A first-class command avoids overloading GTM/proposal brief vocabulary and gives the generic projection a closed `mdp.decision-card.v1` schema.

### D2. Compose the canonical trace

Source resolution must be extracted once from the `Commands::Trace` dispatch path and reused by `Commands::DecisionCard`. The card builder receives a completed `DecisionTrace` plus a sanitized, allowlisted view of the exact source artifact where the source contract permits it. It must never independently infer graph nodes, rule matches, or verification state.

### D3. Keep source authority explicit

The card records the source artifact contract and SHA-256 from the trace. `authority.projection_only` is always `true`, `authority.decision_authority` mirrors the trace, and `authority.output_authority` can never be stronger than the trace. Markdown must state that the source artifact/receipt retains authority.

### D4. Closed allowlist, not generic summarization

Card content is deterministic Rust projection, not model-written prose. Per supported source contract, extract only explicitly approved scalar labels, IDs, bounded reasons, observation receipts, classification values/bases already stored in authoritative artifacts, and trace limitations. Do not recursively render arbitrary JSON.

### D5. One output object, two presentations

Build `mdp.decision-card.v1` once. Markdown is rendered from that object. Global `--json` always wins and emits one JSON envelope. `--json --format markdown` must follow the existing typed presentation-conflict contract rather than mixing Markdown into stdout.

### Assumptions to verify during implementation

- The verified-run trace contains enough artifact references to render a useful card without opening additional private artifacts. If not, the run card must say the detail is unavailable rather than adding another artifact-discovery mechanism.
- The source wrapper can be safely re-read once through the existing bounded authority parser. If reuse would duplicate TOCTOU checks, introduce a shared parsed-source result in `decision_trace.rs` rather than reading independently.

## 5. Contract Shape

Add a Rust `DecisionCard` projection with this minimum shape:

```text
contract: mdp.decision-card.v1
status: available | blocked | unavailable
subject:
  kind: person | account | opportunity | run | conformance | unknown
  display_label: optional bounded label
decision:
  outcome: bounded source-derived value
  action_gate: allow-review | needs-review | no-draft | blocked | unavailable
authority:
  projection_only: true
  decision_authority: source-artifact | verified-run | validated-prompt-output | none
  output_authority: boolean
  verification_state: verified | not-verified | failed
source:
  contract: string
  sha256: lowercase SHA-256
classifications: bounded array of taxonomy/value/basis/derived-from IDs
reasons: bounded array of rule/reason IDs and safe labels
evidence: bounded array of observation IDs, attempt IDs, source class, confidence, observed-at, and artifact refs
gaps: bounded array of field/reason identifiers
next_action: deterministic gate-derived instruction
trace: mdp.decision-trace.v1
limitations: bounded string array
truncation: explicit counts/flags
```

The implementation may refine field names during coding only when required by existing enums/schema conventions. It must preserve the semantics above and update the issue/plan before changing the authority model.

## 6. Affected Files And Symbols

| File | Current responsibility | Intended change |
|---|---|---|
| `cli/src/commands/decision_trace.rs` | Bounded source projection and trace authority | Extract a reusable source-resolution input/options helper; expose only the safe trace/source metadata needed by the card; keep existing trace output byte-for-byte compatible. |
| `cli/src/commands/decision_trace/tests.rs` | Trace projection safety and fixtures | Add compatibility tests proving refactoring does not alter existing fit/run JSON or Mermaid semantics. |
| `cli/src/commands/decision_card.rs` (new) | N/A | Define `DecisionCard`, deterministic contract-specific projection, limits, source allowlists, next-action mapping, and Markdown renderer. |
| `cli/src/commands/decision_card/tests.rs` or module tests | N/A | Cover supported, blocked, unavailable, truncation, privacy, and source-binding states. |
| `cli/src/commands/mod.rs` | Command module exports | Register and export decision-card projection/rendering. |
| `cli/src/cli.rs` | Clap command and schema target definitions | Add `Commands::DecisionCard`, a `DecisionCardFormat` enum, trace-equivalent authority args, and `SchemaTarget::DecisionCardV1`. |
| `cli/src/app.rs` | CLI dispatch and artifact writing | Resolve the canonical trace once, build the card, write Markdown/JSON through existing safe output helpers, and preserve envelope behavior. |
| `cli/src/output.rs` | Typed presentation compatibility and human summaries | Add the decision-card format selector to the single presentation matrix; add concise human/summary behavior without accidental pretty JSON. |
| `cli/src/commands/schemas.rs` | Public schema dispatch | Route `DecisionCardV1` to the new closed schema. |
| `cli/src/commands/capabilities.rs` | Machine-readable command/capability inventory | Advertise `decision-card`, its contract, read-only-unless-out side effect, required/optional flags, formats, and stable diagnostics. |
| `cli/tests/cli_contract.rs` | Public CLI parsing/help contract | Cover required source group, conflicting bindings, formats, and help text. |
| `cli/tests/json_stdout_contract.rs` | Global JSON/stdout invariant | Cover JSON, Markdown conflict, summary, errors, and `--out` combinations. |
| `examples/decision-trace/fixtures/` | Synthetic trace proof inputs | Add only synthetic card fixtures if existing fit/run fixtures cannot cover all states. |
| `cli/USAGE.md` | Detailed command usage | Document copyable fit/person and verified-run card commands. |
| `docs/decision-traces.md` | Canonical trace vocabulary and safety | Explain trace versus decision card and the shared authority boundary. |
| `docs/getting-started.md` | Golden path | Add the card as the optional operator review after a saved decision. |
| `CONCEPTS.md` | Stable product vocabulary | Define decision card as an informational projection, not authority. |
| `plugin/skills/mdp/references/operator-runtime.md` | Agent/operator runtime guidance | Teach when to request a card versus JSON trace and require source artifacts for audit. |

Do not change `plugin/assets/templates/`, pack manifests, prompts, source-kind value contracts, or the MDP-for-MDP repository for this implementation.

## 7. Ordered Implementation Steps

### Step 1 — Refactor trace source resolution without behavioral change

1. Introduce a crate-private options/result type in `decision_trace.rs` representing the mutually exclusive file, validated prompt-output, verified run, and conformance inputs.
2. Move the `Commands::Trace` match currently in `app.rs` behind one reusable function.
3. Keep `project_source_file`, `project_prompt_output_validation_file`, `project_run_files`, and `project_conformance_file` as the only authority-specific implementations.
4. Add regression assertions for representative trace JSON and Mermaid fields.

**Why:** MDP-339 must reuse trace authority rather than reconstruct it.  
**Acceptance covered:** canonical trace reuse; no authority drift; compatible existing trace behavior.

### Step 2 — Add the closed decision-card schema and projection model

1. Create `decision_card.rs` with size/count constants no larger than the existing trace bounds.
2. Define the serialized contract and schema with `additionalProperties: false` at every owned object.
3. Build the card from `DecisionTrace` plus contract-specific allowlisted fields.
4. Map trace/source states monotonically: unavailable remains unavailable, blocked remains blocked, and output authority never increases.
5. Preserve explicit truncation and limitations.

**Why:** a deterministic schema-backed object is required before Markdown polish.  
**Acceptance covered:** schema validity; source identity/hash; safe reasons/evidence/gaps; fail-closed behavior.

### Step 3 — Implement contract-specific safe projections

1. Fit: include fit status, named subject when safely present, persona resolution, match/disqualifier IDs, missing/invalid field identifiers, and lineage-qualified accepted/rejected observation receipts.
2. Route/brief: include draft status, safe persona/job/channel identifiers, summarized selection counts/refs, gaps, and no-draft reason codes; never copy generated message text.
3. Validated prompt output: include validation/authority state and declared classification values with cited attempt IDs only when the canonical validated trace grants authority.
4. Verified run: include terminal/verification state, driver/model/prompt artifact references already exposed by the trace, and published output authority; do not open unreferenced files.
5. Conformance: include journey state and public-safe artifact refs already exposed by the canonical projection.

**Why:** each source type has different privacy and authority semantics.  
**Acceptance covered:** supported source matrix; exact declared evidence; no raw private payload leakage.

### Step 4 — Render intentional Markdown

1. Render the card object into outcome-first sections: Decision and Gate, Subject, Classification, Reasons, Evidence, Gaps and Limitations, Allowed Next Action, Decision Path, and Authority.
2. Use the canonical `render_mermaid(&card.trace)` output for Decision Path.
3. Escape Markdown/frontmatter values and omit empty optional sections without hiding blockers.
4. State the projection/source authority notice prominently.

**Why:** operators need a one-minute review surface, not pretty-printed JSON.  
**Acceptance covered:** human readability; person-specific path; explicit human-approval boundary.

### Step 5 — Wire CLI, presentation, schema, and capabilities

1. Add `Commands::DecisionCard` with the trace-equivalent source group, `--format markdown|json`, and `--out`.
2. Dispatch through the shared trace resolver, then the card projector.
3. Register the format selector in `output.rs` so global JSON behavior is mechanically enforced.
4. Add `decision-card-v1` to `SchemaTarget`, schema dispatch, and schema-name tests.
5. Add the command to capabilities with stable `invalid_decision_card`/unavailable diagnostics only if new diagnostics are actually needed.

**Why:** public CLI behavior must be discoverable and consistent.  
**Acceptance covered:** one documented command; JSON invariant; stable schema/capabilities discovery.

### Step 6 — Add automated and manual proof

1. Unit-test every supported source state and the contract schema.
2. Add adversarial fixtures containing private prose, email/phone-like fields, absolute paths, oversized labels, Mermaid directives, and unsupported contracts; assert they are absent or sanitized.
3. Add subprocess tests for stdout/stderr/exit status and presentation combinations.
4. Exercise the synthetic person-level fit path and compare rule/evidence IDs against the source result and trace.
5. Update docs and the operator skill only after the CLI contract is final.

**Why:** the polish surface is high-risk precisely because it can conceal omitted blockers or leak evidence.  
**Acceptance covered:** privacy, fail-closed behavior, one-minute comprehension, documentation consistency.

## 8. Acceptance Mapping

| Acceptance criterion | Implementation steps | Validation |
|---|---|---|
| Documented read-only card command emits JSON and Markdown | 2, 4, 5 | CLI contract and subprocess tests; copyable docs smoke |
| Card is projection-only and identifies source hash/authority | 1, 2 | Schema tests and state-monotonicity unit tests |
| Fit card exposes exact matched rules/evidence/gaps/action gate | 3 | Synthetic fit and insufficient-context fixtures |
| No-draft/disqualified paths never expose usable copy or authority | 2, 3, 6 | Negative fixtures; absence assertions; output-authority assertions |
| Existing person-specific designed graph and observed path are reused | 1, 4 | Compare embedded trace with `mdp trace` output |
| Global JSON output remains one valid envelope | 5 | `cli/tests/json_stdout_contract.rs` |
| Unsupported/unverifiable sources fail closed | 2, 3 | malformed, unsupported, tampered, and unbound fixtures |
| Content derives only from declared source fields | 3, 6 | allowlist tests with injected private/arbitrary fields |
| Docs distinguish source, trace, and card authority | 6 | documentation review and skill packaging checks |
| No external side effects | all | capabilities side-effect classification plus code review |

## 9. Tests And Validation

### Focused during implementation

```bash
cargo test --manifest-path cli/Cargo.toml commands::decision_card
cargo test --manifest-path cli/Cargo.toml commands::decision_trace
cargo test --manifest-path cli/Cargo.toml --test cli_contract decision_card
cargo test --manifest-path cli/Cargo.toml --test json_stdout_contract decision_card
```

Use the actual test filter names created by implementation; do not weaken assertions merely to match these illustrative commands.

### Repository regression

```bash
cargo test --manifest-path cli/Cargo.toml
make validate
```

Run the repository's documentation/skill packaging validation required by changed `plugin/skills/` surfaces.

### Manual proof

1. Save one synthetic fit result and run both `mdp trace --file ... --format mermaid` and `mdp decision-card --dir ... --file ...`.
2. Verify a reviewer can identify the subject, decision, exact matched IDs, evidence attempt IDs, gaps, next action, and source authority in under one minute.
3. Repeat for insufficient-context and disqualified/no-draft fixtures.
4. Exercise a verified synthetic run and confirm model/prompt/driver provenance is referenced without claiming the card is the receipt.
5. Search emitted Markdown/JSON for fixture-only secrets, raw source prose, emails, phone-like strings, and absolute paths.

## 10. Compatibility And Migration

- Additive command and schema only; no existing artifact contract changes.
- `mdp trace` JSON and Mermaid remain compatible.
- `mdp.human-brief.v0` and `render-brief` templates remain compatible.
- Old installations simply lack `decision-card`; capability discovery remains the supported feature test.
- The card accepts only already-supported trace authority sources. New source contracts require a separate versioned trace/card extension.
- No pack migration is required.
- No source-kind, adapter, prompt, or model-selection migration is required.

## 11. Risks And Safety Boundaries

| Risk | Mitigation |
|---|---|
| Card is mistaken for authority | Prominent projection notice; source hash and authority fields; monotonic authority tests |
| Blocking caveat omitted by polish | Closed required gate/gap/limitation fields; negative fixtures; no model summarization |
| Private prospect/customer data leaks | Contract-specific allowlists, bounded labels, adversarial absence tests, no recursive JSON renderer |
| Trace and card drift | Compose canonical `DecisionTrace`; one shared source resolver; compare outputs in tests |
| CLI JSON contract regresses | Register format in typed presentation matrix and subprocess suite |
| Graph implies workflow execution | Reuse existing trace vocabulary and docs; state visualization-only boundary |
| Scope expands into MDP-202 | Limit changes to the new command plus required shared refactor/docs |

No outreach, CRM/HeyReach mutation, provider call, deployment, release, or external contact is authorized by this plan.

## 12. Rollout, Observability, And Rollback

- Land in one feature PR linked to MDP-339 after explicit implementation authorization.
- A release is not implied by implementation; follow repository shipping instructions if Brandon separately requests release intent.
- Observable proof is CLI JSON/Markdown, schema discovery, capabilities metadata, tests, and a synthetic person-level example.
- Rollback is a normal revert of the additive command/module/docs. Because existing contracts remain unchanged, rollback requires no data migration.
- If safe subject/evidence projection cannot be implemented without reading unrestricted payloads, ship the card with those sections explicitly unavailable rather than broadening access.

## 13. Blockers And Readiness Verdict

No repository, contract, or dependency blocker remains for implementation. The command shape, contract boundary, supported authority sources, privacy strategy, files, tests, compatibility, and rollback are resolved.

Implementation remains human-gated: Brandon must explicitly authorize implementation before Orchid or another executor starts code changes.

**Readiness verdict: `READY_TO_PIN`**
