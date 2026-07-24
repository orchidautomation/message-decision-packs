# MDP Docs Shipped Release Audit

Date: 2026-07-09
Issue: MDP-85
Audit baseline: `origin/main` at `21efe38` / `v0.1.36`
Reviewed release window: `v0.1.27` through `v0.1.36`

## Summary

The main README, getting-started docs, conceptual docs, public-safe guardrails, CLI usage notes, starter template docs, and plugin skills are broadly aligned with the shipped CLI and skill surface from `v0.1.27` through `v0.1.36`.

The highest-value follow-up is not a broad rewrite. It is a small docs pass for the domain-agnostic primitive model, the new Vercel BDR scout surface from `v0.1.36`, and stale distribution evidence that still describes a pre-release installer failure.

## Source Evidence

Release history was refreshed from `origin/main` before review. The current source of truth is newer than the original intake note:

- `21efe38` / `v0.1.36`: release after `feat: build Vercel-first MDP BDR scout`.
- `2bd2c0c` / `v0.1.35`: MDP source strategy skill.
- `2078601` / `v0.1.34`: generic human brief renderer CLI.
- `3fda140` / `v0.1.33`: readable proposal review and prospect brief output.
- `c876e90` / `v0.1.32`: proof-output verifier.
- `v0.1.31` through `v0.1.27`: proposal safety bindings, template/eval refresh, claim guardrails, proof/provenance hardening, prospect input validation, prompt-output readiness, account-only/no-draft ergonomics, skill resources, eval harness, and account/proposal normalization.

Validation commands run:

- `git fetch origin main --tags`: passed.
- `git log --oneline --decorate --max-count=35 origin/main`: passed; confirmed `v0.1.36` at `21efe38`.
- `cargo run --manifest-path cli/Cargo.toml -- --help`: passed; current command list includes `render-brief`, `verify-output`, `validate-prompt-output`, `agent-surface`, and current GTM/proposal commands.
- `cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic`: passed with `valid: true`, `error_count: 0`, `warning_count: 0`.
- `gh release view --repo orchidautomation/message-decision-packs`: confirmed latest release assets exist for `v0.1.36`, including installers, CLI binaries, host bundles, and `llms` files.
- `make validate`: passed after installing missing local validation prerequisites (`rustfmt` and Python `PyYAML`).

Skipped:

- `examples/mdp-bdr-scout-vercel` npm checks: attempted before dependency install and failed because `tsx` was not installed locally. This was not treated as product validation evidence.

## Surfaces Reviewed

- `README.md`
- `cli/USAGE.md`
- `docs/agent-hook-guidance.md`
- `docs/conceptual-decision-flow.md`
- `docs/distribution.md`
- `docs/getting-started.md`
- `docs/new-codex-user-journey.md`
- `docs/pluxx-distribution-evaluation.md`
- `docs/prompt-extraction-contract.md`
- `docs/skill-evals.md`
- `docs/what-this-repo-is.md`
- relevant `docs/orchid/*` plans and decisions for recent shipped context
- `llms.txt` and `llms-full.txt`
- `plugin/skills/`
- `plugin/assets/templates/basic`
- `plugin/assets/templates/proposal/README.md`
- `plugin/assets/templates/proposal`
- `plugin/skills/mdp-create-pack/references/profile-primitives-map.md`
- `plugin/skills/mdp-lfg/references/mdp-mental-model.md`
- `examples/mdp-bdr-scout-vercel/README.md`
- `examples/profound-gtm-vetting/flue-webhook-agent/README.md`

## Findings

### F1: New Vercel BDR scout is shipped but underrepresented in top-level public docs

Severity: Medium

Type: missing/confusing docs

Related release: `v0.1.36`

Evidence:

- Commit `a1f7bd0` added `examples/mdp-bdr-scout-vercel/` and the Vercel-first scout plan.
- `README.md` canonical examples list MDP for MDP, Profound GTM Vetting, and the Flue webhook scaffold, but not `examples/mdp-bdr-scout-vercel/`.
- `examples/profound-gtm-vetting/flue-webhook-agent/README.md` correctly says the Flue webhook is legacy context and points to `examples/mdp-bdr-scout-vercel/`.
- `llms-full.txt` still mentions the Profound Flue webhook scaffold but does not mention the Vercel BDR scout.

Impact:

Users entering through README or agent context files will see the older webhook adapter before the current Vercel-first demo path. The boundary language inside the BDR scout README is strong, but discovery is weak.

Recommended next action:

Quick copy correction. Add `examples/mdp-bdr-scout-vercel/` to README canonical examples and regenerate/update `llms.txt` and `llms-full.txt` release context so the current demo path is visible without making MDP sound like hosted SDR infrastructure.

Recommended follow-up ticket:

Title: `Document Vercel BDR scout as the current demo path`

Acceptance:

- README canonical examples include the Vercel BDR scout and clarify it is a wrapper around MDP.
- Flue webhook remains framed as historical adapter context.
- `llms.txt` and `llms-full.txt` mention the scout only as a demo wrapper, not as MDP core behavior.
- Public-safe boundary terms remain: no CRM writes, no sending, no scraping/enrichment by MDP, no hosted SDR product claim.

### F2: BDR scout README contains a stale version-specific limitation

Severity: Low

Type: stale copy

Related release: `v0.1.36`

Evidence:

- `examples/mdp-bdr-scout-vercel/README.md` says current legacy example packs have prompt-contract validation issues under `mdp 0.1.35`.
- The example itself shipped in `v0.1.36`, and `cli/Cargo.toml` is `0.1.36`.
- The current root template validation for `plugin/assets/templates/basic` passes with zero errors and warnings.

Impact:

The note may make readers think the new scout example was validated against the prior release or that the current release still has the same known validation state.

Recommended next action:

Quick copy correction. Replace the version-specific `mdp 0.1.35` language with current, version-neutral guidance, or revalidate the exact legacy example pack referenced and state the current failing command precisely.

Recommended follow-up ticket:

Title: `Refresh BDR scout limitation note against v0.1.36`

Acceptance:

- BDR scout README no longer names stale `mdp 0.1.35` unless the note is explicitly historical.
- If a limitation remains, it names the exact pack and command that fails on current `v0.1.36`.
- Local sample prerequisites say dependencies must be installed before npm checks.

### F3: Distribution evaluation is linked as current evidence but still records a pre-release installer failure

Severity: Medium

Type: stale/confusing install docs

Related releases: post-`v0.1.27` release/install closeout, current `v0.1.36`

Evidence:

- `docs/distribution.md` points to `docs/pluxx-distribution-evaluation.md` as the current packaging recommendation and validation evidence.
- `docs/pluxx-distribution-evaluation.md` is dated 2026-06-24 and says latest-release QA failed because installer assets did not exist yet.
- GitHub release `v0.1.36` now contains `install.sh`, host installer scripts, CLI binaries, host bundles, checksums, `release-manifest.json`, and `llms` files.

Impact:

Readers following current distribution docs get contradictory evidence: current install docs present release installers as the public path, while the linked validation evidence ends with a pre-release blocked state.

Recommended next action:

Small docs refresh. Either append a 2026-07-09 update section to the evaluation doc with current release asset evidence and any current QA result, or relabel the 2026-06-24 file as historical and link a newer release/install closeout note from `docs/distribution.md`.

Recommended follow-up ticket:

Title: `Refresh release installer validation evidence`

Acceptance:

- `docs/distribution.md` links to current release/install validation evidence or clearly labels the 2026-06-24 evaluation as historical.
- Current release asset inventory for `v0.1.36` is captured.
- Any fresh installer smoke test result is recorded with exact command, environment, outcome, and residual risk.

### F4: Current CLI command inventory is split across README, CLI usage, and capability output

Severity: Low

Type: confusing docs / maintainability

Related releases: `v0.1.32` through `v0.1.35`

Evidence:

- README documents modern workflows for `validate-prompt-output`, `verify-output`, `render-brief`, `agent-surface`, `sample-leads`, strict validation, and profile activation.
- `cli/USAGE.md` also documents many of these details, but its stable JSON error code list omits newer codes that `mdp --json capabilities` exposes, such as `invalid_prospect`, `invalid_proof_output`, and `invalid_human_brief`.
- The canonical machine-readable source is `mdp --json capabilities`, but prose docs do not consistently point readers to it as the up-to-date command/error inventory.

Impact:

This is not blocking, because the CLI itself exposes the current contract. The risk is maintenance drift as more commands and error codes are added.

Recommended next action:

Quick copy correction. Make `cli/USAGE.md` explicitly defer the complete command and error inventory to `mdp --json capabilities`, then update or remove stale partial error-code enumeration.

Recommended follow-up ticket:

Title: `Make CLI usage docs defer command inventory to mdp capabilities`

Acceptance:

- `cli/USAGE.md` no longer carries a stale partial error-code list, or it matches `mdp --json capabilities`.
- README and getting-started docs keep workflow examples, while `capabilities` remains the canonical machine-readable command inventory.

### F5: Domain-agnostic primitive model exists, but the public docs do not teach it clearly enough

Severity: Medium

Type: missing/confusing docs

Related releases: `v0.1.27` through `v0.1.35`

Evidence:

- `README.md` and `cli/USAGE.md` list the universal primitive IDs and explain `required_primitives` / `primitive_map`, but only as terse paragraphs.
- `plugin/assets/templates/basic/.mdp/manifest.yaml` and `plugin/assets/templates/proposal/.mdp/manifest.yaml` both declare all current universal primitives: `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals`.
- The core domain abstraction is explained well in `plugin/skills/mdp-create-pack/references/profile-primitives-map.md` and `plugin/skills/mdp-lfg/references/mdp-mental-model.md`.
- The broader domain-profile foundation plan includes the clearest GTM/proposal mapping tables, but it lives under `docs/plans/` as a planning artifact rather than a current user-facing concept doc.

Impact:

An operator looking for the "core 8 primitives" will not get a clean current answer from the main docs. The implementation has ten current universal primitives: the likely core eight domain primitives plus `gaps` and `evals` as first-class visibility/validation primitives. Because this explanation is scattered, readers can miss that GTM personas/pains/claims and proposal roles/requirements/proof are profile vocabulary over the same domain-agnostic model.

Recommended next action:

Small concept-doc addition, not a rewrite. Add a public-facing "Domain Primitives" section or short doc that names the current ten universal primitives, distinguishes the core domain primitives from `gaps` and `evals` if desired, and shows a compact GTM/proposal mapping table.

Recommended follow-up ticket:

Title: `Document universal primitives and GTM/proposal mappings`

Acceptance:

- Public docs explain whether the product vocabulary is "core 8" plus `gaps`/`evals`, or simply the current ten universal primitives.
- The doc includes GTM mapping examples: personas -> `actors`, fit rules -> `decision-criteria`, signals -> `source-signals`, pains -> `needs-requirements`, claims/positioning -> `evidence-proof`, avoid rules/objections -> `boundaries`, output rules/copy patterns/CTAs/hooks -> `output-contracts`, channel policies/motions -> `routing-jobs`.
- The doc includes proposal mapping examples: proposal roles -> `actors`, bid/no-bid/evaluation criteria -> `decision-criteria`, opportunity context/requirement signals -> `source-signals`, requirements matrix -> `needs-requirements`, proof library -> `evidence-proof`, proposal/compliance boundaries -> `boundaries`, proposal output rules/review outputs -> `output-contracts`, review gates -> `routing-jobs`.
- The doc states that account context and opportunity context are profile-owned vocabulary/input contracts, not new core MDP objects unless the CLI contract changes.
- README, `cli/USAGE.md`, and agent context files link to the canonical primitive explanation instead of duplicating divergent lists.

### F6: Template manifests are aligned, but template language is not surfaced as a current reference

Severity: Low

Type: missing docs / discoverability

Related releases: `v0.1.27` through `v0.1.35`

Evidence:

- The GTM starter manifest carries profile metadata, `required_primitives`, a populated `primitive_map`, account/person readiness contracts, and account-only/no-draft eval categories.
- The proposal template manifest carries proposal-native card IDs, `required_primitives`, a populated `primitive_map`, an `opportunity` input contract, profile jobs, and safety/eval coverage.
- `plugin/assets/templates/proposal/README.md` explains proposal activation and safety boundaries.
- `plugin/assets/templates/basic` has no matching README that explains the GTM template as the reference expression of the primitive model.

Impact:

The template files are correct, but users must inspect YAML or skill references to understand the GTM/proposal language choices. This can make the GTM vocabulary look like the core ontology and proposal vocabulary look like a separate system, even though both are profile expressions over the same primitives.

Recommended next action:

Quick docs addition. Add a short GTM/basic template README or public docs subsection that mirrors the proposal README's activation explanation and points to the primitive mapping.

Recommended follow-up ticket:

Title: `Add GTM template language reference`

Acceptance:

- `plugin/assets/templates/basic` has a README or public docs section explaining the GTM starter's profile metadata, input contract, `required_primitives`, `primitive_map`, and account-only/no-draft behavior.
- The GTM reference explicitly says personas, pains, CTAs, hooks, motions, and channel policies are GTM profile vocabulary, not universal core object names.
- The proposal README links back to the same primitive explanation so GTM and proposal read as two profiles over one model.

## Confirmed Aligned Areas

- Public artifact guardrails are strong. README, AGENTS, proposal skills, proposal template README, and BDR scout README all avoid claiming MDP is a sender, CRM, scraper, enrichment provider, AI SDR, BI tool, proposal management system, legal/procurement approval system, or compliance certifier.
- The implementation and templates carry the current domain-agnostic primitive set. Both GTM and proposal templates declare `required_primitives` and map profile-owned vocabulary through `primitive_map`.
- Proposal docs and skills correctly preserve the `mdp.proof-output.v0` proof boundary. They require `verify-output` before treating model-written IDs as proof and keep missing proof as gaps.
- Readable GTM and proposal artifacts are documented as review layers, not machine sources of truth or permission to send/reuse output.
- Account-only/no-draft behavior is covered across README, getting-started, conceptual flow, prompt extraction contract, starter evals, and relevant skills.
- Source strategy is covered in README, `docs/what-this-repo-is.md`, `llms` context, `mdp-lfg`, and the dedicated `mdp-source-strategy` skill. It is framed as a strategy/handoff artifact, not execution.
- Template validation for `plugin/assets/templates/basic` passes cleanly on the current CLI.

## Public-Safety Check

No remediation should introduce raw proposal documents, customer names, private GTM strategy, browser/session data, secrets, local auth material, private source text, or access-controlled evidence into public docs.

Recommended copy should keep using synthetic fixtures, sanitized examples, public-source examples, and wrapper language such as "Vercel scout example" rather than claiming MDP itself performs hosted discovery, enrichment, CRM sync, outreach, or proposal submission.
