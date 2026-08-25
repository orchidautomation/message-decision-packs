# MDP CLI, Skills, and MCP Product Usability Audit

Date: 2026-08-24
Status: issue-ready audit
Audited release: `v0.1.74`
Repository commit: `c998d94`
Scope: CLI, five authored skills, skill evals and packaging, profile-neutral and proposal MCP adapters, validation workflows, temporary artifacts, and a fresh Sanity GTM pack.

## How to use this document

This document is the durable handoff for creating Linear work. It contains the audit evidence, product recommendation, priority order, issue boundaries, acceptance criteria, dependencies, and validation expectations. An issue-creation agent should:

1. create one initiative or parent issue named **MDP Productization and Golden Path**;
2. create the issue drafts in [Linear-ready backlog](#linear-ready-backlog) without combining them unless an owner explicitly chooses a smaller delivery slice;
3. preserve the stated non-goals so usability work does not weaken MDP authority or safety;
4. link every issue back to this document and the cited code locations;
5. add new issue identifiers to the dependency map after creation;
6. treat the priority labels as recommendations, not as evidence that external deployment or release has been authorized.

No issue in this audit authorizes publishing, merging, releasing, enabling provider calls, or transmitting source material.

## Executive verdict

MDP has a production-grade validation kernel inside a pre-productized operator experience.

The system is unusually strong at preserving authority, rejecting unsupported claims, validating references, bounding model input, and producing deterministic artifacts. The weak point is not core correctness. The weak point is that users and agents must understand too much of the internal assurance model before they experience a useful result.

| Dimension | Score | Verdict |
| --- | ---: | --- |
| Deterministic core robustness | 8/10 | Strong, heavily tested, fail-closed |
| Fresh-install CLI usability | 5/10 | Fast and predictable, but too flat and inconsistent |
| Turnkey first useful result | 4/10 | Starter validates, yet the advertised target-aware path fails or blocks |
| Agent Skills format compliance | 9/10 | Formally excellent |
| Skill execution usability | 5/10 | Rigorous but dense, quiet, and over-specified |
| Skill evaluation maturity | 5/10 | Excellent fixture design; behavioral execution not demonstrated |
| Canonical MCP local robustness | 7/10 | Strong transport hardening for trusted local stdio |
| MCP production safety | 5/10 | Needs approved-root and per-call consent controls |
| Product clarity / “magic” | 4/10 | Power is visible; payoff is buried |

### The shortest diagnosis

MDP currently asks the user to learn its proof system before it gives them the emotional payoff of a decision pack.

The product should lead with:

1. what is being built;
2. what source material will become durable authority;
3. the smallest validation loop;
4. one visible useful result;
5. advanced lineage, receipts, conformance, MCP, and proof machinery only when the workflow requires them.

## Product principles for follow-up work

Every issue created from this audit should preserve these constraints:

- The Rust CLI remains the authority for execution, validation, assurance, artifact hashes, and receipts.
- Usability projections may summarize authority; they may not silently weaken, bypass, or reinterpret it.
- A blocked or no-draft result must remain fail-closed and must never be presented as successful generation.
- MDP remains local/offline by default. Provider access is explicit, bounded, and disabled by default.
- The canonical MCP remains an optional adapter, not a second implementation of MDP semantics.
- Public fixtures must remain synthetic or sourced from approved public material.
- Beginner paths should disclose advanced proof machinery only when the selected job requires it.

## Missing product abstraction: seamless file choreography

Temporary-file safety is necessary but not sufficient. MDP must also keep its file choreography out of the user's way.

The useful precedent from workflow plugins such as Compound Engineering is not a runtime dependency or a reason to build a new artifact platform. It is the experience: the workflow chooses safe locations, distinguishes scratch material from durable handoffs, passes pointers between stages, validates before handoff, and tells the user what was retained.

MDP already has much of the durable execution primitive. A successful `mdp run` stages a private transaction and atomically publishes a self-verifying directory containing `run-bundle.json`, `run-receipt.json`, and the allowed artifacts. The missing layer is for skills and beginner workflows to use that bundle consistently instead of making the user manually compose and shuttle normalized-input, source-audit, binding, routed-context, decision, and receipt paths.

The desired experience is deliberately smaller than a general artifact manager:

1. **Pack authoring remains visible.** The pack tree is durable product authority, human-readable, and versionable; it is not hidden behind a registry.
2. **Intermediate work remains private.** A skill resolves one unique scratch location, freezes approved bytes, composes lineage artifacts, and removes scratch state on normal completion.
3. **Execution publishes one bundle.** The existing run bundle/receipt directory is the durable handoff rather than a new global artifact database.
4. **Skills pass one pointer.** Downstream review or resume consumes the explicit bundle/receipt path or another existing typed artifact—not five paths pasted by the user and never an ambient “latest.”
5. **Closeout explains retention.** The user sees the decision, durable bundle, discarded scratch state, unresolved gaps, and next allowed action.
6. **Explicit paths remain available.** Advanced and automation callers retain the precise low-level CLI without making it the ordinary UX.

Do not add `mdp artifacts list|show|clean` or a persistent global artifact index until observed workflows prove that a self-contained bundle and skill conventions are insufficient. Keep the solution transport-neutral and host-portable. Compound Engineering is a UX precedent, not a dependency.

## What is already production-grade

- The exact audited commit had green GitHub checks for CLI, Pluxx, and the shipped example.
- `cargo test` completed with **693 passed, 0 failed** in about 190 seconds.
- The current generic starter passed `doctor`, `validate`, 35 eval fixtures, and route-budget validation.
- Normal local operations were fast: most checks completed in 8–70 ms; 35 eval fixtures took about one second.
- JSON errors are structured, bounded, and accompanied by meaningful nonzero exit codes in most validation commands.
- Run publication has strong defenses: output containment, safe leaves, exclusive claims, private staging, cleanup, and atomic publication.
- The canonical MCP freezes request bytes, rejects symlinks and unsafe path shapes, limits request/response sizes, constrains child environments, applies timeouts, and preserves blocked/no-draft authority.
- All five skills pass local structural, contract, packaging, and plugin validation.
- The repository has a sophisticated skill-eval corpus: 72 trigger cases, 43 output cases, 122 assertions, train/validation splits, explicit collision pairs, and null routes.

## Highest-priority findings

### P0 — `mdp init` can leave a partially written pack

`init` writes most of the pack before checking a late example-file collision. A reproduction with a pre-existing `examples/clay-row.json` exited with a write conflict but left 67 generated files behind.

Evidence: `cli/src/commands/init.rs:484-556`.

Why it matters: initialization is the trust-forming moment. A failed initializer must leave either a complete pack or the original directory, not an ambiguous partial product.

Recommendation: preflight every destination, stage the complete tree, validate the staged tree, then atomically publish it.

### P0 — Global `--json` can emit Markdown

`mdp --json verify-output --readable ...` exits successfully while emitting Markdown rather than JSON.

Evidence: `cli/src/app.rs:461-477`; global JSON promise at `cli/src/cli.rs:9-16`.

Why it matters: this breaks the most important agent-facing output guarantee.

Recommendation: reject conflicting flags or return a JSON envelope containing the readable artifact.

### P0 — Agent-readable capabilities drift from the actual CLI

The capabilities contract omits `run-preflight`, omits `run --transport-timeout-ms`, and usually provides an undifferentiated argument list instead of required, optional, repeatable, and conflicting arguments.

Evidence: `cli/src/commands/capabilities.rs:281-318` versus `cli/src/cli.rs:328-348`.

Recommendation: derive capabilities from the Clap graph or enforce exact parity with generated contract tests.

### P0 — The target-aware starter advertises commands that fail

The Sanity pack returned these `next_commands` after successful initialization:

- `mdp ... fit --dir PACK --prospect ...`
- `mdp ... brief --dir PACK --prospect ...`

Both failed with `governed_job_required` because a target-aware multi-job pack requires `--job`. Adding `--job prospect-fit-or-brief` still produced a no-draft result because governed jobs require normalized input plus lineage artifacts.

Evidence: next-command generation at `cli/src/commands/init.rs:1008-1012`; governed selection at `cli/src/commands/routing.rs:404`.

Why it matters: the initializer presents commands as the next successful experience. They currently teach a path the same release rejects.

Recommendation: generate commands from the actual selected job contract. For a governed job, either generate the complete safe workflow or give one agent-oriented next action such as “Build the required normalized input with the pack skill.”

### P0/P1 — Credentialed MCP calls lack approved input-root enforcement

Both MCP adapters accept caller-selected local paths. The proposal compatibility path can send staged source bytes to a provider when a credential and process-level enable flag are present. The source-intake approval ledger is structurally checked but is not a separately trusted or tamper-evident authorization channel.

This becomes a serious exfiltration risk if an MCP caller is prompt-injected or otherwise untrusted and can both read/write local files and invoke the credentialed adapter.

Evidence: `scripts/mdp-proposal-mcp-server.mjs:567-646`, `scripts/mdp-proposal-runner.mjs:648-895`, `scripts/mdp-run-mcp-server.mjs:395-508`.

Recommendation:

- configure canonical pack, input, approval-ledger, and output roots at server startup;
- require realpath containment inside those roots;
- bind native-call permission to a specific approved request rather than one process-wide flag;
- require out-of-band confirmation or tamper-evident approval for real source transmission;
- keep native calls disabled by default.

## Sanity dogfood journey

Official Sanity material was used as public evidence for a scratch GTM pack. Sanity describes a structured-content foundation consisting of Content Lake, AI-first tools, and APIs/SDKs, and frames content operations as a coordinated system rather than one-off publishing work.

### What worked

- Target-aware initialization completed in 25 ms.
- The generated pack was structurally valid and its five starter evals passed.
- The generated pack correctly remained activation-blocked while product evidence was missing.
- Adding public source receipts and bounded authority for product identity, actors, problems, claims, and proof boundaries made `requirements` and `skills --job prospect-fit-or-brief` report ready.
- `readme refresh`, strict validation, evals, and route-budget checks all passed afterward.
- The system correctly refused to turn thin synthetic input into outreach-ready context.

### Where the magic disappeared

1. `doctor` and `validate` said the untouched target-aware pack was valid and healthy while `skills` and `requirements` said activation was blocked. These are individually defensible semantics, but there is no single user-level answer to “Can I use this pack?”
2. `route-budget` described six routes as ready even while profile activation was blocked. Again, this is structurally explainable but experientially contradictory.
3. The initializer returned broken next commands.
4. The generated example prospect could not be consumed by the governed job.
5. The dedicated `rebind-synthetic-chain` helper generated four artifacts and a valid chain, but the resulting synthetic persona and segment did not match the target-aware pack, so fit remained `insufficient-context`.
6. Using the generated chain required five artifact flags plus a prompt argument. Passing the full prompt path caused the CLI to duplicate the prompt directory; only `--prompt normalize-prospect.yaml` worked. The help does not explain that the value is relative to `.mdp/prompts/`.
7. A small MVP required editing the source ledger, five card kinds, product-foundation bindings, activation metadata, and the README inventory. This is powerful but not turnkey.
8. `readme refresh` correctly updated the owned inventory only. The human-written thesis, source list, and gaps remained stale, and validation did not identify that semantic drift.

### Product implication

MDP needs an explicit distinction between:

- **structurally valid**;
- **ready for a selected job**;
- **ready with the supplied input**;
- **safe to draft or act**.

Those states already exist internally. The product needs one concise projection that explains them in order and gives the next safe action.

## CLI usability audit

### Information architecture

The top-level help exposes roughly 33 commands, while capabilities advertises 37 contracts. Beginner actions, authoring helpers, audit-grade execution, conformance, receipts, traces, schemas, and proof compilation occupy one flat namespace.

Recommendation: preserve backward-compatible commands but introduce a progressive surface:

- Core: `init`, `check`, `fit`, `brief`, `check-claims`, `test`.
- Build/inspect: `pack`, `route`, `requirements`, `skills`, `gaps`, `readme`.
- Advanced execution: `run ...` and `proof ...` groups.
- Qualification/audit: `conformance ...` and trace/receipt operations.

The most valuable addition would be a single read-only `mdp check --dir PACK [--job JOB] [--input INPUT]` projection that composes:

1. installation/version;
2. structural validity;
3. profile and job readiness;
4. context-budget readiness;
5. supplied-input readiness;
6. the next safe command.

### Diagnostics and output

- `doctor` exits 0 for a missing pack and can report a parseable wrong-format manifest as healthy while `validate` rejects it.
- `doctor --summary` contains null count fields that the underlying health output does not provide.
- Default `eval` output for 35 fixtures was about 1 MB; practical usage depends on discovering `--summary`.
- Input/output flag vocabulary is fragmented across `--out`, `--out-dir`, `--routed-context-out`, `--file`, `--prospect`, `--draft`, and `--request`.
- Help generally describes flags but provides few working examples or next actions.

Recommendation: every blocked result should carry a stable `next_action` object with a human summary, whether it is retryable, and the exact smallest safe command when one exists.

### Crash and temporary-artifact behavior

Strengths:

- `run` uses private staging, exact transaction claims, atomic publication, and normal cleanup.
- Synthetic-chain writes have rollback tests.
- MCP request freezing uses private temporary directories and frozen bytes.

Risks:

- SIGKILL can strand hidden output claims; the direct CLI only reports `output-directory-claimed`, while validated recovery logic exists in the MCP supervisor.
- Make validation uses shared fixed `/tmp/mdp-*.json`, `/tmp/mdp-skill-evals`, proposal smoke directories, and evidence-harness paths. Parallel or nested invocations can overwrite evidence or race.
- Abrupt MCP termination can leave private request-freeze directories in the system temp root.

Recommendation:

- add `mdp run recover` or a validated, actionable recovery instruction;
- allocate one `mktemp` validation workspace per invocation and clean it with a trap;
- add strict owner/mode/age-based cleanup for stale MCP freeze directories;
- never automatically delete customer-controlled proposal workdirs.

## Agent Skills specification audit

### Formal compliance

All five skills comply with the core Agent Skills format:

- correct `SKILL.md` and YAML frontmatter;
- names match parent directories and naming rules;
- descriptions are concise and below 1024 characters;
- all main files remain below 500 lines;
- direct skill-local links resolve;
- reference files are generally focused and one level deep.

### Progressive disclosure

The official guidance recommends keeping activated instructions below roughly 5,000 tokens and moving conditional mechanics into focused references. MDP has the right directory structure but underuses it.

- `mdp-pack-builder/SKILL.md`: 410 lines, 25.5 KB, 3,163 words. It includes GTM, proposal, decision-input v1/v2, budget, runner, MCP, receipt, and conformance mechanics.
- `mdp/SKILL.md`: 377 lines, 20.8 KB, 2,596 words. It repeats detailed execution contracts despite having `references/cli-operator.md`.
- `mdp-gtm-brief` makes every mode ingest a long shared normalization/lineage gate before selecting its specific reference.
- `mdp-proposal-review` loads an evidence-path reference and then repeats much of that path in the entrypoint.

Recommendation: keep only the universal safety invariants, mode selection, minimal golden path, and user communication contract in each main file. Move conditional protocol mechanics to references with explicit load conditions.

### Trigger descriptions

The descriptions are concise, imperative, and contain good negative boundaries. The main collision risk is the coordinator:

> “Use when the user names MDP...”

That catches almost every specialized request where the user happens to name the product. Narrow `mdp` to installation, CLI/operator explanation, contract inspection, and genuinely mixed workflows. Clarify that builder owns edits while review owns read-only diagnosis.

### Communication UX

Each skill has a good final response contract. None reliably tells the agent to orient the user before work or communicate major stages during work.

Add one shared communication contract to all five skills:

1. **Orient:** In 2–4 sentences, state what is being built/reviewed, the selected job or mode, the evidence boundary, and the useful artifact the user will receive.
2. **Plan:** Name only 3–5 meaningful phases, not every command.
3. **Progress:** Announce phase changes, blockers, and decisions; do not narrate routine file reads.
4. **Translate:** Explain CLI results in product language: structurally valid, job-ready, input-ready, or no-draft.
5. **Close:** Show artifacts, the decision, unresolved gaps, and the next allowed action.

Example opening:

> I’ll turn the approved Sanity sources into a small GTM decision pack. First I’ll capture product and audience authority, then I’ll validate one `prospect-fit-or-brief` path, and finally I’ll show whether the pack is structurally valid, ready for that job, and what evidence is still missing. MDP will not enrich prospects, invent proof, or send anything.

### Portability

Several skills reference shared `scripts/...` paths that are outside the skill directory. That can work inside the Pluxx plugin using `${PLUGIN_ROOT}`, but it is not portable Agent Skills behavior where paths are relative to the skill root.

Some references also point to repo-only docs not present in installed skill bundles. Add `compatibility` frontmatter describing the CLI/plugin requirements, make required references self-contained, and either expose helper behavior through the CLI or bundle skill-local scripts.

## Skill eval audit

The corpus design is excellent. The execution evidence is not yet sufficient to support claims that the skills themselves improve agent behavior.

The current harness validates fixture shape, routing expectations, assertion inventory, and 13 CLI cases. Its report explicitly returned:

- `observed: null`
- `comparison: null`

It does not execute fresh agents with and without the skill, capture outputs and traces, grade assertions with evidence, measure tokens/time, perform blind comparisons, or incorporate human review.

Recommendation:

1. Materialize an AgentSkills-compatible `evals/evals.json` view for generic tooling while keeping the stronger shared MDP corpus as canonical.
2. Add real file-bound cases rather than synthetic identifiers alone.
3. Run fresh-context with-skill and baseline/previous-version trials.
4. Run trigger cases repeatedly and measure trigger rate.
5. Capture tokens, duration, grading evidence, and transcripts.
6. Add communication assertions: plan explained, mode/job named, actions not taken stated, artifacts and next action reported.
7. Blind-review holistic usability and retain a human feedback field.

## MCP audit

### Why the MCP exists

The profile-neutral MCP is an optional local stdio adapter. It gives MCP-capable hosts typed discovery and structured file/path calls without requiring the host to construct shell commands. It adds transport safety, cancellation for execution, schema discovery, and structured result handling. It does **not** add MDP authority; the CLI remains authoritative.

For coding agents that already have safe shell access, the incremental value is limited. Therefore MCP should be positioned as an optional host adapter, not a concept every MDP user must understand.

### Transport decision: keep local stdio

Do **not** replace the local stdio adapter with HTTP. The accepted runtime architecture specifies thin local stdio MCP and plugin adapters (`docs/orchid/decisions/2026-08-03-unified-clean-context-runtime.md:9-23`), and the canonical server identifies the CLI as the sole authority (`scripts/mdp-run-mcp-server.mjs:285-380`).

The MCP transport specification recommends stdio support whenever possible. A local Streamable HTTP server would introduce a listening service, port and process lifecycle, authentication, `Origin` and `Host` validation, DNS-rebinding defenses, sessions, and concurrency semantics without fixing MDP's current approved-root, consent, discoverability, or error-contract problems.

HTTP is a deferred optional adapter, not current roadmap work. Reconsider it only when a named consumer cannot launch subprocesses, multiple clients need one persistent server, browser/cross-container access becomes a requirement, or measured process startup cost materially harms the experience. Any future spike must preserve one transport-neutral tool core, bind only to loopback by default, use an ephemeral port and per-launch authentication, validate origin and host, retain approved-root enforcement, and keep native calls disabled by default.

External authority: [MCP transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).

### Why there are two MCPs

- `mdp-run-mcp-server.mjs` is the canonical profile-neutral adapter.
- `mdp-proposal-mcp-server.mjs` is a v0 compatibility wrapper around the older proposal orchestration stack.

Recommendation: remove the proposal MCP from default discovery and beginner documentation. Keep it explicitly labeled compatibility-only until consumers migrate. Do not maintain two first-class MCP stories.

### Canonical MCP strengths

- bounded JSON-RPC input and output;
- request byte freezing;
- symlink/hard-link and output-path protections;
- child environment allowlist;
- timeouts and output limits;
- exact CLI contract validation;
- authority-preserving blocked/no-draft handling.

### MCP usability and protocol gaps

- Cancellation reaches `mdp_run` but not prepare or verify; the proposal server has no cancellation handler.
- Normal requests are globally serialized, so a long operation can block all later tools and pings.
- Both initialize handlers echo arbitrary client protocol versions rather than negotiating a supported version.
- Documentation and code disagree about whether proposal authority refusals set MCP `isError`.
- The canonical server exposes four tools, but the main skill and runner documentation omit `mdp_prepare_run`.
- Error contracts usually lack phase, retryability, and the next safe action.
- Proposal errors can return absolute local paths to the MCP client.

Recommended canonical story:

1. `mdp_run_tools`
2. `mdp_prepare_run`
3. `mdp_run`
4. `mdp_verify_run`

Show the artifact created at each stage and keep this entire section hidden unless the host is using MCP-native tools.

## Recommended product shape

### The magical beginner path

The user should be able to say:

> Build an MVP Message Decision Pack for Sanity from these approved public sources.

The skill should answer:

> I’ll capture the smallest source-backed product, audience, problem, claim, and guardrail set needed for one GTM job. Then I’ll run MDP’s structural and job-readiness checks and show you the resulting pack, remaining gaps, and one safe example decision. Nothing will be sent or enriched.

Internally, the agent can run many exact commands. Externally, the user should experience four phases:

1. **Sources approved**
2. **MVP authority authored**
3. **Pack checked**
4. **One job demonstrated**

### CLI golden path

```bash
mdp init --template gtm --target-name "Sanity" --dir ./sanity-mdp
mdp check --dir ./sanity-mdp --job prospect-fit-or-brief
mdp test --dir ./sanity-mdp --job prospect-fit-or-brief
```

`check` and `test` are proposed product-level commands. Existing precise commands remain available to agents and advanced operators.

### What should remain advanced

- source-binding validation;
- prompt-output receipts;
- run receipts and verification;
- conformance qualification;
- decision traces;
- proof-output compilation;
- MCP setup;
- provider-specific execution.

These are differentiating capabilities. They should be progressive disclosure, not the front door.

## Linear-ready backlog

### Recommended initiative

**Title:** MDP Productization and Golden Path
**Outcome:** A new user or agent can initialize an MVP pack, understand its readiness, demonstrate one governed job, and recover from ordinary failures without learning the internal assurance architecture first. Core authority, offline defaults, and fail-closed behavior remain intact.
**Suggested success measures:** zero broken commands in generated starter guidance; median time from init to understandable readiness result under five minutes; every machine-readable error remains parseable; observed skill evals show orientation and correct handoff; credentialed MCP cannot read outside startup-approved roots.

### Issue 1 — Make `mdp init` transactional

**Priority / size:** P0 / M
**Problem:** A late collision can return failure after writing most of a starter pack, leaving an ambiguous partial tree.
**Scope:** Preflight the entire destination set or stage the complete starter in a private sibling directory; validate before publication; publish atomically where the filesystem permits; clean staging on all handled failures.
**Non-goals:** Changing starter content, overwriting user files, or weakening collision detection.
**Acceptance criteria:**

- A collision at any generated destination leaves the original destination byte-for-byte unchanged.
- Successful initialization produces the same canonical starter tree as `v0.1.74` unless another approved issue changes that content.
- `--force` behavior is explicit, tested, and cannot create a mixed old/new tree after failure.
- Tests cover early collision, late `examples/clay-row.json` collision, staged validation failure, cleanup, and success.
- JSON and human results identify whether anything was published.

**Implementation evidence:** `cli/src/commands/init.rs:484-556`.
**Validation:** targeted init tests, starter-tree golden diff, full Rust suite.
**Dependencies:** none; do before Issue 4.

### Issue 2 — Enforce the global JSON output invariant

**Priority / size:** P0 / S
**Problem:** `mdp --json verify-output --readable` emits Markdown, breaking agent parsing.
**Scope:** Define conflict semantics for `--json` plus readable output; preferably reject the combination with structured JSON or return readable content inside a JSON envelope; audit every command for the same invariant.
**Non-goals:** Redesigning all human output.
**Acceptance criteria:**

- Every invocation with global `--json` writes exactly one valid JSON value to stdout on success and failure.
- Conflicting presentation flags produce a stable machine-readable diagnostic and nonzero exit.
- A table-driven test exercises every command/presentation flag combination exposed by Clap.
- Capabilities accurately describe any conflicts.

**Implementation evidence:** `cli/src/app.rs:461-477`, `cli/src/cli.rs:9-16`.
**Validation:** JSON parse tests plus CLI contract tests.
**Dependencies:** coordinate with Issue 3.

### Issue 3 — Generate agent capabilities from the CLI contract

**Priority / size:** P0 / M
**Problem:** Capabilities omit commands and flags and do not expose enough argument structure for reliable construction.
**Scope:** Derive command/argument metadata from the Clap graph or build an exact parity generator and test; expose required, optional, repeatable, defaulted, enum, and conflicting arguments.
**Non-goals:** Removing existing commands or promising semantic success from syntactic capabilities.
**Acceptance criteria:**

- `run-preflight` and `run --transport-timeout-ms` appear correctly.
- Every public Clap command and option is represented or explicitly marked human-only with a tested reason.
- Required/optional/repeatable/conflicting semantics match Clap.
- CI fails on future drift.
- The capabilities schema is versioned and backward compatibility is documented.

**Implementation evidence:** `cli/src/commands/capabilities.rs:281-318`, `cli/src/cli.rs:328-348`.
**Validation:** generated snapshot plus exact graph-parity tests.
**Dependencies:** Issue 2 conflict metadata; informs Issue 6.

### Issue 4 — Generate truthful target-aware starter next actions

**Priority / size:** P0 / M
**Problem:** Target-aware initialization advertises `fit` and `brief` commands that the selected governed job rejects.
**Scope:** Generate next actions from actual profile/job requirements; include `--job` where sufficient; where lineage is required, point to a complete safe workflow or an agent-oriented builder action instead of a doomed command.
**Non-goals:** Bypassing governed input or fabricating lineage so a demo appears successful.
**Acceptance criteria:**

- Every generated command for each shipped template is executable against that starter and either succeeds or returns the explicitly predicted readiness state.
- Governed jobs never advertise raw-input commands that cannot satisfy their contract.
- Human and JSON init results describe prerequisites before the command.
- Starter docs and examples use the generated canonical path.
- Regression coverage includes target-aware multi-job GTM and proposal templates.

**Implementation evidence:** `cli/src/commands/init.rs:1008-1012`, `cli/src/commands/routing.rs:404`.
**Validation:** fresh-directory end-to-end starter matrix.
**Dependencies:** Issue 1; align output with Issues 5 and 6.

### Issue 5 — Add one authoritative readiness projection

**Priority / size:** P0 / L
**Problem:** `doctor`, `validate`, `route-budget`, `requirements`, and `skills` expose individually correct but experientially contradictory readiness signals.
**Scope:** Add a read-only `mdp check` command or equivalent projection that composes installation, structural validity, profile activation, selected-job readiness, route budget, optional supplied-input readiness, safe-to-draft state, and the next safe action.
**Non-goals:** Replacing specialist validators, changing their authority, or treating structural validity as activation.
**Acceptance criteria:**

- Output distinguishes `structurally_valid`, `job_ready`, `input_ready`, and `safe_to_draft_or_act` without collapsing unknown into false.
- Human output explains the first blocking gate in plain language.
- JSON output cites the contributing contracts/results and contains no invented authority.
- The command is read-only and works offline.
- Generic, target-aware blocked, governed-input missing, budget-blocked, and fully ready fixtures are covered.
- Help provides one working beginner example.

**Validation:** contract tests, fixture matrix, CLI snapshot, dogfood against a fresh public-source GTM pack.
**Dependencies:** uses Issue 6; should inform Issue 4 and Issue 7.

### Issue 6 — Standardize actionable CLI diagnostics

**Priority / size:** P1 / M
**Problem:** Blocked/error results inconsistently expose phase, retryability, and the smallest safe next action.
**Scope:** Define a versioned `next_action`/diagnostic shape with phase, code, retryability, human summary, prerequisites, and exact command only when safe and complete; apply it first to init, doctor, validate, readiness, run claims, and governed routing.
**Non-goals:** Generating speculative repair commands or hiding underlying diagnostics.
**Acceptance criteria:**

- Targeted commands emit the same diagnostic shape in JSON.
- Commands are omitted rather than partial when prerequisites cannot be represented safely.
- Human output translates the same object without semantic drift.
- Paths and messages are bounded and do not leak unnecessary private absolute paths.
- Contract and compatibility tests cover the schema.

**Dependencies:** Issue 3 capabilities; consumed by Issues 4, 5, 10, and 13.

### Issue 7 — Add a shared skill communication contract

**Priority / size:** P0 / M
**Problem:** Skills specify strong closeouts but do not reliably orient users or communicate meaningful progress, making MDP feel opaque and agent-dependent.
**Scope:** Add the five-part orient/plan/progress/translate/close contract to all five authored skills; tailor mode/job language; avoid noisy command-by-command narration; add communication assertions.
**Non-goals:** Streaming internal chain of thought, exposing secrets, or claiming a blocked pack is ready.
**Acceptance criteria:**

- Each skill opens with the selected job/mode, evidence boundary, expected artifact, and actions it will not take.
- Each skill announces only meaningful gates, blockers, and decisions.
- Closeout reports artifacts, readiness state, unresolved gaps, and next allowed action.
- Shared evals assert orientation, boundaries, progress, translation, and handoff.
- Existing safety, authority, and routing contract tests remain green.

**Implementation surfaces:** `plugin/skills/*/SKILL.md`, `plugin/skill-evals/*`.
**Validation:** skill contract/packaging validators plus observed eval work from Issue 10.
**Dependencies:** consume Issue 5 terminology when available; may land earlier with existing terms.

### Issue 8 — Refactor skills for progressive disclosure and trigger clarity

**Priority / size:** P1 / L
**Problem:** Main skill entrypoints carry too much conditional runner, lineage, MCP, and mode detail; the coordinator description collides with specialist skills.
**Scope:** Shrink `mdp`, `mdp-pack-builder`, `mdp-gtm-brief`, and `mdp-proposal-review`; move conditional mechanics to focused one-level references; narrow coordinator routing; distinguish edit-intent builder work from read-only review.
**Non-goals:** Removing safety invariants or changing the authoritative job mapping.
**Acceptance criteria:**

- Main files retain universal safety invariants, mode selection, minimal golden path, and response contract.
- Conditional details load only for the selected mode.
- `mdp` no longer triggers merely because a specialized request names the product.
- Builder and reviewer descriptions have explicit, non-overlapping ownership.
- All structural, trigger, collision, and output eval fixtures remain valid or are intentionally versioned.
- Entry files meet the Agent Skills recommendation of a concise body; exact token measurements are recorded when tooling is available.

**Validation:** official/local skill validators, trigger corpus, repeated observed trials from Issue 10.
**Dependencies:** Issue 7 communication contract.

### Issue 9 — Make installed skills portable and self-contained

**Priority / size:** P1 / M
**Problem:** Skills reference plugin-root scripts and repository-only docs that are unavailable under portable Agent Skills path semantics.
**Scope:** Add concise `compatibility` frontmatter; make required references self-contained; expose required helpers through `mdp` or ship skill-local scripts; eliminate required repo-only document dependencies from installed bundles.
**Non-goals:** Adding ornamental assets or experimental `allowed-tools` metadata without need.
**Acceptance criteria:**

- Every required relative resource resolves from the installed skill root or an explicitly documented plugin compatibility layer.
- Skills can explain missing CLI/Node/MCP prerequisites with actionable errors.
- Installed bundles contain every required execution reference.
- Packaging tests validate both plugin-host and portable skill layouts.
- No required one-level reference links to an unavailable repo document.

**Validation:** packaging validator, installed-bundle smoke tests, link checker.
**Dependencies:** coordinate with Issue 8 file moves.

### Issue 10 — Execute behavioral skill evals with evidence

**Priority / size:** P1 / L
**Problem:** The eval corpus is rigorous, but current reports validate inventory rather than observed agent behavior.
**Scope:** Materialize AgentSkills-compatible eval views; bind real synthetic/public input files; run fresh isolated contexts with skill, baseline, and previous version; capture trigger rate, outputs, tokens, duration, grading evidence, blind comparison, and human feedback.
**Non-goals:** Publishing unsupported benchmark claims or using private/customer data.
**Acceptance criteria:**

- Every selected trial records prompt, input bindings, skill version, host/model metadata, output, timing, token use when available, assertion evidence, and grader result.
- Trigger validation uses repeated positive, negative, near-miss, collision, and typo cases with held-out validation prompts.
- With-skill results are compared against baseline or previous version in clean contexts.
- Communication assertions from Issue 7 are graded.
- Reports clearly separate observed results from deterministic corpus validation.
- A documented human review path exists for holistic usability.

**Validation:** reproducible eval command and sanitized committed aggregate fixture/report.
**Dependencies:** Issues 7 and 8; AgentSkills guidance linked below.

### Issue 11 — Enforce MCP approved roots and per-call provider consent

**Priority / size:** P0 / L
**Problem:** Credential-capable adapters accept caller-selected paths without startup-approved root containment, and native-call permission is process-wide rather than request-specific.
**Scope:** Configure canonical pack/input/approval/output roots at startup; realpath and regular-file containment; bind native-call authorization to the exact frozen request/source digest; require an out-of-band or tamper-evident approval mechanism; keep provider calls disabled by default.
**Non-goals:** Turning MDP into a hosted service, granting the MCP new authority, or trusting an approval file solely because it sits beside untrusted input.
**Acceptance criteria:**

- Attempts to access outside approved roots, including traversal, symlink, hard-link, rename, and time-of-check/time-of-use cases, fail closed before provider invocation.
- Approval binds the exact request and source bytes and cannot be supplied solely through ordinary tool arguments.
- A process-level credential or enable flag alone cannot authorize an individual provider call.
- Output roots are separately constrained and cannot overwrite existing customer data.
- Denials are bounded, redact private paths where unnecessary, and include a safe recovery action.
- Security tests prove no provider process/request starts on denial.

**Implementation evidence:** `scripts/mdp-proposal-mcp-server.mjs:567-646`, `scripts/mdp-proposal-runner.mjs:648-895`, `scripts/mdp-run-mcp-server.mjs:395-508`.
**Validation:** adversarial path/consent matrix, process-spawn spy, existing MCP tests.
**Dependencies:** Issue 6 diagnostics. Blocks any claim that credentialed MCP is production-ready.

### Issue 12 — Publish one canonical MCP story

**Priority / size:** P1 / M
**Problem:** A canonical profile-neutral adapter and a proposal v0 compatibility adapter appear as competing MCP products.
**Scope:** Make `mdp-run-mcp-server.mjs` the documented default; present `mdp_run_tools`, `mdp_prepare_run`, `mdp_run`, and `mdp_verify_run` as the complete path; remove proposal MCP from beginner/default discovery and label it compatibility-only with migration guidance.
**Non-goals:** Deleting compatibility code before known consumers migrate or replacing stdio with HTTP.
**Acceptance criteria:**

- Beginner docs and skill routing describe only the canonical four-tool story.
- Every stage names its input and produced artifact.
- Proposal MCP is explicitly versioned/deprecated or hidden from default discovery with a consumer migration note.
- CLI authority and MCP non-authority are stated consistently.
- Installed plugin metadata, docs, and tool discovery agree.

**Validation:** docs/link checks, installed plugin smoke, MCP discovery snapshots.
**Dependencies:** Issue 11 for production-safety wording; no dependency on HTTP.

### Issue 13 — Harden MCP lifecycle and protocol behavior

**Priority / size:** P1 / L
**Problem:** Cancellation, concurrency, protocol negotiation, and error semantics differ across tools and adapters.
**Scope:** Add cancellation parity for prepare/verify, define bounded concurrency so pings/control messages are not starved, negotiate only supported MCP protocol versions, standardize `isError` and error metadata, and redact unnecessary absolute paths.
**Non-goals:** Adding remote HTTP transport or changing CLI decision authority.
**Acceptance criteria:**

- Every long-running tool can be cancelled and cleans private staging.
- A long tool call does not prevent required control/cancellation handling.
- Unsupported protocol versions receive a spec-compliant bounded error.
- Documentation and tests agree on `isError` for protocol failures versus valid blocked/no-draft tool results.
- Errors include phase, retryability, and safe next action without leaking avoidable local paths.
- Canonical and compatibility adapters share behavior where compatibility permits.

**Validation:** MCP process tests for cancellation, concurrency, version mismatch, cleanup, and error snapshots.
**Dependencies:** Issue 6 diagnostic schema; coordinate with Issue 12.

### Issue 14 — Add validated direct-run crash recovery

**Priority / size:** P1 / M
**Problem:** SIGKILL can strand a hidden transaction claim, after which direct CLI runs only report `output-directory-claimed`; safer recovery logic exists only in the MCP supervisor.
**Scope:** Add `mdp run recover` or equivalent validated recovery guidance; inspect ownership, type, age, transaction metadata, and destination state before removal; preserve customer workdirs.
**Non-goals:** Automatically deleting unknown or customer-controlled directories.
**Acceptance criteria:**

- A simulated killed run can be diagnosed and safely recovered through the CLI.
- Recovery refuses ambiguous ownership, links, unsafe modes/types, recent live claims, or inconsistent transaction metadata.
- Dry-run/diagnostic output explains exactly what would be removed.
- Ordinary run errors provide the recovery command through the standardized next action.
- Recovery never deletes a published bundle or customer workdir.

**Validation:** kill/restart integration tests and adversarial filesystem cases.
**Dependencies:** Issue 6 diagnostics.

### Issue 15 — Isolate validation and MCP temporary workspaces

**Priority / size:** P1 / M
**Problem:** Fixed `/tmp` paths can race across parallel/nested validation, and abrupt MCP termination can strand private freeze directories.
**Scope:** Allocate one private `mktemp` root per validation invocation; trap handled exits; thread paths through Make and harnesses; implement strict owner/mode/age-based cleanup for MDP-owned stale freeze directories.
**Non-goals:** Cleaning arbitrary system temp content or customer proposal workdirs.
**Acceptance criteria:**

- Two full validation invocations can run concurrently without shared paths or overwritten evidence.
- Every temp root uses restrictive permissions and unpredictable names.
- Normal success/failure cleans owned temporary data.
- Stale cleanup requires positive MDP ownership markers, safe type/mode, and minimum age.
- Tests prove unrelated temp directories are never removed.

**Implementation surfaces:** `Makefile`, validation/eval harnesses, `scripts/mdp-run-mcp-server.mjs`.
**Validation:** parallel smoke, interruption test, cleanup safety matrix.
**Dependencies:** coordinate with Issues 1, 13, and 14 but can ship independently.

### Issue 16 — Improve CLI summaries, examples, and flag ergonomics

**Priority / size:** P2 / M
**Problem:** Default eval output is enormous, help lacks working examples, and path/input flags use inconsistent vocabulary.
**Scope:** Make concise output discoverable or default where compatibility permits; add examples and next-step help to core commands; document prompt paths as relative to `.mdp/prompts/`; define a compatibility plan for flag vocabulary.
**Non-goals:** Breaking existing scripts by abruptly removing flags or hiding full diagnostic payloads.
**Acceptance criteria:**

- `eval` guides users to a concise summary and keeps full results explicitly available.
- Core help includes executable generic and target-aware examples.
- Prompt/path semantics state whether values are pack-relative, prompt-root-relative, or absolute.
- Aliases preserve compatibility for any normalized flag names.
- Help, capabilities, and docs are generated/tested for agreement where practical.

**Validation:** help snapshots, example execution, compatibility tests.
**Dependencies:** Issues 3, 4, and 5 should establish canonical commands first.

### Issue 17 — Align `doctor` health semantics and exit behavior

**Priority / size:** P1 / M
**Problem:** `doctor` can exit zero with envelope `ok: true` for a missing/invalid pack, omit meaningful counts in summary output, and describe a parseable wrong-format manifest as healthy even when `validate` rejects it.
**Scope:** Define whether `doctor` reports environment reachability, pack validity, or both; separate those states in its contract; use checked exit behavior for an explicitly requested pack; populate summary counts from real diagnostics; align help with actual guarantees.
**Non-goals:** Making every blocked activation state a structurally invalid pack or replacing the richer readiness projection in Issue 5.
**Acceptance criteria:**

- Missing, unreadable, wrong-format, structurally invalid, structurally valid-but-activation-blocked, and ready packs have distinct tested states.
- The top-level `ok` value and exit code follow the documented contract for each state.
- Summary fields never contain unexplained null counts.
- `doctor` cannot call a manifest healthy when the structural validator rejects its format.
- Help distinguishes local installation health, structural validity, and job readiness.

**Implementation evidence:** `cli/src/app.rs:175`, `cli/src/app.rs:845-867`, `cli/src/commands/health.rs:69-119`, `cli/src/output.rs:175-182`.
**Validation:** table-driven CLI tests across the state matrix; JSON and human snapshots.
**Dependencies:** align terminology with Issue 5.

### Issue 18 — Ship a coherent target-aware demonstration fixture

**Priority / size:** P1 / M
**Problem:** The generated target-aware example and `rebind-synthetic-chain` artifacts can validate structurally while using persona/segment assumptions that do not match the initialized target, so the advertised demonstration ends in `insufficient-context`.
**Scope:** Make generated synthetic inputs explicitly compatible with the starter's selected target/job or clearly label them as schema-only fixtures; provide one end-to-end public/synthetic example that demonstrates the expected safe decision state without inventing proof.
**Non-goals:** Auto-enrichment, fabricated customer evidence, or forcing a positive fit decision.
**Acceptance criteria:**

- A fresh target-aware starter includes or can generate one job-coherent, explicitly synthetic input chain.
- The example's persona, segment, product assumptions, and prompt lineage agree with the initialized target authority.
- The example reaches its documented deterministic outcome; a no-fit or insufficient-evidence outcome is acceptable when intentionally explained.
- Generated source/normalization/decision artifacts pass all lineage validation.
- Starter documentation distinguishes schema smoke fixtures from meaningful product demonstrations.

**Validation:** end-to-end target-aware init → chain → readiness → fit/brief smoke using public or synthetic data.
**Dependencies:** Issues 4 and 5 define the canonical path.

### Issue 19 — Detect CLI, plugin, and skill-bundle version skew

**Priority / size:** P1 / M
**Problem:** The CLI can be current while an installed host plugin/skill bundle remains older, leaving users with mismatched instructions and runtime contracts without a clear diagnostic.
**Scope:** Define compatibility metadata between CLI and plugin bundle; surface installed component versions and actionable skew in setup/doctor guidance; keep release-owned installation responsibilities explicit.
**Non-goals:** Silently upgrading host plugins, publishing a release, or assuming every portable skill install has plugin metadata.
**Acceptance criteria:**

- The CLI and installed plugin expose machine-readable version/compatibility metadata where the host supports it.
- Setup or doctor reports compatible, unknown, older-plugin, older-CLI, and unsupported combinations distinctly.
- Diagnostics explain the host-specific upgrade action without performing it automatically.
- Portable skill installs degrade gracefully when plugin version metadata is unavailable.
- Release/package tests prevent publishing a bundle whose declared CLI compatibility contradicts its authored skills.

**Validation:** version-matrix fixtures and installed-bundle smoke tests.
**Dependencies:** Issue 9 compatibility metadata; coordinate with distribution documentation.

### Issue 20 — Clarify README refresh ownership and semantic drift

**Priority / size:** P2 / S
**Problem:** `readme refresh` correctly updates its owned inventory, but human-authored thesis, source, and gap sections can remain stale without an explicit warning, leading users to assume the whole README was reconciled.
**Scope:** Mark generated versus human-owned regions clearly; report which sections were refreshed and which were not; optionally detect referenced cards/sources that no longer exist without rewriting prose.
**Non-goals:** Generating or silently editing human product claims, thesis, or source interpretation.
**Acceptance criteria:**

- Refresh output lists changed generated regions and untouched human-owned regions.
- The README format clearly identifies ownership boundaries.
- Validation can warn on objectively stale references from human sections without attempting semantic authorship.
- Warnings remain non-authoritative and cannot invent replacement prose.

**Validation:** refresh snapshots for fresh, changed-inventory, deleted-reference, and human-only edits.
**Dependencies:** none.

### Issue 21 — Introduce progressive CLI command grouping without breaking aliases

**Priority / size:** P2 / L
**Problem:** Roughly 33 top-level commands mix beginner actions with conformance, receipts, traces, proof compilation, and other advanced operations, making the product appear more complicated than its golden path.
**Scope:** Define and implement progressive command groups for core, pack/build, run/proof, and conformance/audit surfaces; retain tested compatibility aliases; update help so the beginner path appears first.
**Non-goals:** Removing expert operations, changing their authority, or making a large flag migration in one release.
**Acceptance criteria:**

- Top-level help leads with `init`, readiness/check, one decision path, and testing.
- Advanced run/proof/conformance operations are discoverable under coherent groups.
- Existing public command spellings remain functional aliases for a documented compatibility window.
- Capabilities identify canonical names and aliases unambiguously.
- Documentation and skills use canonical beginner commands while advanced references retain precise operations.

**Validation:** help/capabilities snapshots, alias parity tests, docs command smoke.
**Dependencies:** Issues 3, 5, and 16 should establish canonical metadata and golden-path names.

### Issue 22 — Make skills hand off one self-contained workflow bundle

**Priority / size:** P0 / M
**Problem:** MDP already publishes a strong self-verifying run directory, but ordinary workflows still expose too much intermediate path choreography to users and agents.
**Scope:** Define shared skill conventions that use one unique private scratch root, compose existing typed intermediates internally, invoke the existing transactional run path, and hand off the explicit published run bundle/receipt directory with a concise retention summary. Add only the smallest CLI affordance needed if the existing bundle cannot support a demonstrated path.
**Non-goals:** Building a global artifact registry, adding generic artifact-management commands, hiding the visible pack tree, inventing a second workflow manifest, selecting an ambient “latest,” automatically committing content, or depending on Compound Engineering.
**Acceptance criteria:**

- A normal pack workflow requires the user to identify the pack, selected job, and approved sources/input—not manually assemble every intermediate file path.
- Skills use a private, unique scratch root and pass intermediate paths internally without copying artifact bodies through chat.
- Successful execution hands off the existing explicit run directory containing its bundle, receipt, and allowed artifacts.
- Downstream review/resume accepts that explicit durable pointer and verifies it before use; no ambient “latest” selection is introduced.
- Scratch material cannot become durable authority without the existing validation and transactional publication gates.
- Normal success, handled failure, timeout, and cancellation remove MDP-owned scratch state while preserving allowed diagnostics and customer-controlled inputs.
- Advanced callers can still supply explicit low-level paths with unchanged authority semantics.
- Human closeout names the decision, durable bundle, discarded scratch state, retention limitations, unresolved gaps, and next permitted handoff.
- The implementation remains transport-neutral and works without MCP or a particular agent host.

**Implementation evidence to reuse:** `cli/src/run_runtime.rs:1429-1534`, `cli/src/run_runtime.rs:1617-1755`, `cli/src/run_runtime.rs:2189-2193` already stage, publish, and bind the run directory.
**Validation:** end-to-end multi-stage workflow with no user-supplied intermediate paths; explicit bundle resume/review; concurrent workflow isolation; explicit-path parity; private-data and cleanup safety tests.
**Dependencies:** Reuse the existing transactional run publication; Issue 5 supplies user-level readiness; Issue 6 supplies handoff diagnostics; Issues 14 and 15 supply recovery and scratch lifecycle. This issue should shape Issues 4, 7, 12, 18, and 21 rather than being added after them.

### Issue 23 — Make multi-file pack authoring previewable and failure-safe

**Priority / size:** P1 / L
**Problem:** Transactional execution is strong, but a skill authoring or updating a pack may edit many durable files directly. A mid-workflow failure, concurrent edit, or validation error can leave a partially updated pack even when `init` itself is fixed.
**Scope:** Define a candidate-change workflow for existing packs: capture expected hashes, stage the intended multi-file change set outside the live pack, validate the complete candidate, show a bounded summary/diff, then apply only if the live inputs still match. Preserve unrelated user changes and support safe rollback of MDP-owned writes when publication fails.
**Non-goals:** Requiring Git, automatically committing changes, hiding pack files, overwriting concurrent edits, or making generative content authoritative without review.
**Acceptance criteria:**

- A multi-file authoring pass can be previewed without changing the live pack.
- The complete candidate pack passes the same structural/job checks before apply.
- Apply refuses if any expected live input changed after staging and reports the conflicting files.
- Publication cannot leave a mixture of old and new MDP-owned files after a handled failure.
- Unrelated user files and edits are preserved.
- Closeout lists created, changed, unchanged, refused, and rolled-back paths without dumping private content.
- The workflow functions in a non-Git directory; when Git is present, its diff may be used for presentation but not correctness.

**Validation:** fault injection at each write, concurrent-edit test, validation-failure test, non-Git smoke, unrelated-file preservation, and successful candidate/live tree comparison.
**Dependencies:** Issue 1 establishes starter transaction semantics; Issue 7 owns communication; Issue 22 owns scratch and handoff conventions.

### Issue 24 — Provide an explicit pack compatibility and upgrade path

**Priority / size:** P1 / M
**Problem:** Version-skew diagnostics help identify an old CLI or plugin, but users also need an obvious answer when a durable pack uses an older manifest, profile, prompt, or artifact contract. Current guidance can tell users to “explicitly migrate” without one coherent previewable workflow.
**Scope:** Define compatibility status for the pack against the current CLI; distinguish compatible, deprecated-but-readable, migration-available, unsupported, and target-retargeting cases; provide a dry-run migration plan and failure-safe application path for supported migrations.
**Non-goals:** Silently rewriting packs, retargeting one company’s pack to another, upgrading evidence authority, or guaranteeing indefinite support for every historical contract.
**Acceptance criteria:**

- Readiness/setup reports the pack contract versions and current compatibility state in human and JSON output.
- A supported migration can be previewed with exact files/contracts affected and no writes.
- Migration applies through the candidate/change safeguards from Issue 23 and revalidates the complete pack.
- Unsupported or authority-changing migrations fail closed with preservation guidance.
- Target changes remain a new-pack or separately explicit operation, never an incidental migration.
- Compatibility and migration fixtures cover every supported released contract boundary.

**Validation:** version-matrix fixtures, dry-run/apply/rollback tests, target-retarget refusal, full validation after migration.
**Dependencies:** Issue 19 detects component skew; Issue 23 supplies safe multi-file application.

### Deferred candidate — Optional local Streamable HTTP adapter

**Decision:** Do not create this issue unless a named consumer satisfies a reversal trigger.
**Reversal triggers:** a host cannot spawn stdio; multiple clients require one persistent process; browser/cross-container access is required; or measured process churn materially harms usability.
**Required spike constraints:** transport-neutral shared tool core, stdio remains default, loopback-only default binding, ephemeral port, per-launch authentication, strict `Origin`/`Host` validation, approved roots, disabled-by-default native calls, lifecycle and session threat model, and no added MDP authority.

## Dependency and delivery map

| Wave | Issues | Rationale |
| --- | --- | --- |
| 1 — Trust and workflow foundation | 1, 2, 3, 11, 22 | Correctness, security, and seamless bundle handoff should precede polish claims. |
| 2 — Golden path | 4, 5, 6, 7, 17, 18 | Create one understandable readiness result, coherent demonstration, and communicative workflow over self-contained bundles. |
| 3 — Simplify and harden | 8, 9, 12, 13, 14, 15, 19, 23, 24 | Reduce cognitive load and make adapters, installs, authoring, upgrades, temporary state, and recovery predictable. |
| 4 — Prove and polish | 10, 16, 20, 21 | Measure actual behavior, then tune discoverability, documentation ownership, and information architecture. |

Parallelism guidance:

- Issues 1, 2, 3, and 11 can begin independently.
- Issue 4 should consume the transaction behavior from Issue 1 and align terminology with Issue 5 if timing permits.
- Issues 7 and 8 should not edit the same skill entrypoints concurrently without explicit ownership.
- Issues 12 and 13 both touch MCP documentation and servers and should be sequenced or divided by file ownership.
- Issue 10 should evaluate the settled communication/trigger changes rather than benchmark a moving target.
- Issue 22 should define the shared scratch and bundle-handoff convention before multiple golden-path issues independently invent incompatible path behavior.
- Issue 23 should precede any skill change that turns multi-file pack authoring into a more automatic experience.

## Finding-to-issue coverage

| Audit finding | Disposition |
| --- | --- |
| Partial pack after failed init | Issue 1 |
| Global JSON emits non-JSON | Issue 2 |
| Capabilities drift | Issue 3 |
| Broken target-aware next commands | Issue 4 |
| Contradictory readiness surfaces | Issues 5 and 17 |
| Missing actionable diagnostics | Issue 6 |
| Quiet/disconnected skill experience | Issue 7 |
| Oversized skill entrypoints and trigger collisions | Issue 8 |
| Non-portable skill resources and missing compatibility metadata | Issue 9 |
| Fixture-only skill eval evidence | Issue 10 |
| MCP approved-root and request-consent gap | Issue 11 |
| Two competing MCP stories | Issue 12 |
| MCP cancellation, concurrency, negotiation, and error gaps | Issue 13 |
| Stranded run claims after hard termination | Issue 14 |
| Fixed validation temp paths and stale MCP freezes | Issue 15 |
| Huge eval output, unclear prompt paths, inconsistent flags, sparse help | Issue 16 |
| `doctor` exit/health/summary ambiguity | Issue 17 |
| Target-aware synthetic chain mismatch | Issue 18 |
| Installed CLI/plugin/skill version skew | Issue 19 |
| README human-section semantic drift | Issue 20 |
| Flat top-level command namespace | Issue 21 |
| Users manually shuttle intermediate artifact paths | Issue 22 |
| Skills do not consistently expose the existing self-contained run bundle as the handoff | Issue 22, implemented with Issues 14 and 15 |
| Multi-file pack authoring can expose partial or conflicting edits | Issue 23 |
| No coherent previewable pack-contract upgrade path | Issue 24 |
| Replace local stdio MCP with HTTP | Rejected for now; deferred candidate only after a named reversal trigger |

## Validation performed for this audit

- Installed and source CLI were both `v0.1.74` at the audited commit; source and installed capabilities were byte-identical.
- Generated generic starter trees from source and installed CLIs diffed cleanly.
- `cargo test --manifest-path cli/Cargo.toml`: 693 passed, 0 failed.
- Current generic starter: doctor, validate, skills, 35 eval fixtures, and route-budget smoke passed.
- Skill validation, contract tests, eval-corpus validation, packaging validation, and plugin validation passed.
- Sanity scratch pack: initialization, public-source authoring, strict validation, eval, requirements, skills, routes, synthetic-chain, fit, and brief flows exercised.
- MCP syntax checks passed.
- Credentialed provider execution was intentionally not performed; no paid/API request was made.

## Audit limitations

- Managed-environment process behavior prevented trustworthy end-to-end MCP pipe testing during the audit; code, syntax, and existing harnesses were inspected instead.
- The current skill harness does not execute and compare live agent behavior, so trigger and output quality scores remain provisional until Issue 10.
- The Sanity pack was a scratch public-source dogfood fixture, not customer data and not a production campaign.
- Line references describe commit `c998d94` and may move as issues are implemented.

## External references

- [Agent Skills overview](https://agentskills.io/home)
- [Agent Skills specification](https://agentskills.io/specification)
- [Agent Skills best practices](https://agentskills.io/skill-creation/best-practices)
- [Description optimization](https://agentskills.io/skill-creation/optimizing-descriptions)
- [Skill evaluation guidance](https://agentskills.io/skill-creation/evaluating-skills)
- [Script guidance](https://agentskills.io/skill-creation/using-scripts)
- [MCP transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- [Sanity product introduction](https://www.sanity.io/docs/getting-started/the-sanity-content-operating-system-an-introduction)
- [Sanity content operations](https://www.sanity.io/docs/getting-started/what-is-content-operations)
- [Sanity product site](https://www.sanity.io/)
