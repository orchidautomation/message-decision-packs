# MDP Greenfield Skill Taxonomy And Routing Contract

Date: 2026-07-13

Status: Accepted by Brandon on 2026-07-13

Linear: MDP-106, under MDP-105 and project `MDP: Agent Skill Surface Redesign`

## Decision

MDP should ship five public Agent Skills organized around user jobs, not the current card types or one skill per proposal review stage:

1. `mdp`
2. `mdp-pack-builder`
3. `mdp-pack-review`
4. `mdp-gtm-brief`
5. `mdp-proposal-review`

Canonical Agent Skill names are the only skill identifiers in the machine-readable contract.
MDP should not add a separate capability-ID registry.
Pack job IDs already describe the domain-specific operation and should select an internal skill mode without introducing a second global taxonomy.

The profile-scoped job IDs are a closed v1 operation vocabulary, not free-form aliases.
Each profile-sensitive skill owns an explicit set of supported job IDs; the CLI rejects an unknown job-to-skill pair.
This keeps mode selection deterministic without inventing capability IDs that merely duplicate jobs.

Each agent-routable pack job should bind directly to one canonical skill through `jobs[].skill_id`.
The job ID supplies the mode and the skill supplies the workflow.
There should be no job-to-many-skills array: prerequisite orchestration belongs inside the selected skill, following the same coordinator-and-handoff pattern that keeps SendLens at five public skills.

The current `profile.agent_surface` object and `mdp.agent-surface.v0` output should be replaced, not migrated.
The replacement CLI contract should describe packaged inventory, pack eligibility, and job routes.
It must explicitly state that host discovery is outside MDP's control and unobserved unless a host reports it.

Brandon approved the greenfield redesign and explicitly waived backwards-compatibility work because the product is pre-launch.

## Evidence And Prior Decisions

- MDP-52 introduced the current `profile.agent_surface` gate and `mdp.agent-surface.v0`. It correctly established that the CLI should make routing policy deterministic, but its four overlapping skill lists duplicate the pack's job model and operate only after host discovery.
- MDP-60 and MDP-67 established progressive resources and the initial eval harness. The resource pattern remains sound, while the current harness covers only three of twenty public skills and therefore does not yet prove catalog-level routing quality.
- The current Codex and Pluxx packaging paths contain identical twenty-skill trees, enforced by validation, but the repository describes `plugin/` as the canonical plugin package. This ADR preserves that ownership and removes the need for two manually authored trees.
- SendLens demonstrates a five-skill, job-shaped surface with explicit adjacent boundaries, coordinator routing, shallow supporting material, and per-skill evals. MDP adopts that surface discipline while retaining its own local/offline decision-context boundary.

## Why This Is The Smallest Robust Design

The current twenty-skill inventory exposes three different implementation concerns as one public surface:

- user jobs such as building or reviewing a pack;
- card-authoring helpers such as ICP, CTA, avoid-rule, and output-rule builders;
- profile-specific review stages such as bid/no-bid, compliance, proof, and red-team review.

Those concerns do not all deserve startup catalog entries.
Agent hosts initially load every discovered skill's name and description, so every extra public skill adds routing context and another collision boundary even when its full instructions are progressively loaded later.
The Agent Skills specification describes this three-tier discovery model and recommends concise instructions plus on-demand references rather than many narrowly fragmented skills solely to reduce body size: [Agent Skills specification](https://agentskills.io/specification), [client implementation guidance](https://agentskills.io/client-implementation/adding-skills-support).

Five front doors preserve the distinctions that materially change trigger behavior or risk:

- explicit MDP/CLI operation;
- pack authoring;
- pack quality review;
- GTM fit/copy decision support;
- proposal decision support.

Everything below those boundaries is a mode, reference, asset, CLI command, or deterministic script.
This keeps public descriptions job-shaped while retaining focused instructions through one-level progressive references.

The SendLens architecture supports this choice.
It exposes five public job-shaped skills, keeps adjacent boundaries explicit in each description, uses one coordinator for broad work, and moves specialized detail into focused references and bounded handoffs.
MDP should transfer that surface discipline without copying SendLens's execution model or agents: MDP remains local decision context and routing, not outbound or proposal execution infrastructure.

## Target Public Skill Inventory

| Skill ID | Public user job | Internal jobs or modes | Trigger boundary |
| --- | --- | --- | --- |
| `mdp` | Explain, inspect, validate, route, or operate MDP explicitly; coordinate mixed MDP requests | CLI/operator help, pack inspection, mixed-task routing | Trigger when the user names MDP, `.mdp`, the MDP CLI, or asks for a mixed MDP workflow. Do not claim generic GTM, copy, or proposal work. |
| `mdp-pack-builder` | Create or improve a pack from approved source material | source strategy, source extraction, shared pack structure, GTM authoring, proposal authoring | Trigger for pack creation or revision. Do not trigger for ordinary source research, generic messaging strategy, or proposal writing without an MDP pack objective. |
| `mdp-pack-review` | Audit, harden, validate, or test a pack | structural review, route/eval review, installed-template QA | Trigger for the pack itself. Do not absorb review of a prospect, copy draft, proposal, RFP, or opportunity merely because those artifacts may use a pack. |
| `mdp-gtm-brief` | Use a GTM pack to produce fit and pre-draft decision context or evaluate a supplied copy draft | `prospect-fit-or-brief`, `outbound-copy-brief`, `outbound-copy-review` | Require a GTM MDP context or explicit request to use one. Do not enrich, draft/send outreach, or trigger on generic cold-email requests. |
| `mdp-proposal-review` | Use a proposal pack to support a bounded review of supplied pursuit material | `bid-no-bid-review`, `compliance-review`, `proof-review`, `red-team-review` | Require proposal MDP context or an explicit request to review against one. Never certify compliance, invent proof, make final approval claims, submit proposals, or replace human proposal/compliance review. |

### Progressive Resource Shape

Each public `SKILL.md` should contain only the trigger boundary, shared gate, core workflow, mode selection rule, critical safety constraints, and output handoff.
Detailed mode procedures should be one level deep under `references/`.
Repeated parsing, validation, rendering, or comparison logic should stay in the CLI or tested `scripts/`, not be re-created by the model.

Recommended resource groups:

- `mdp-pack-builder/references/`: source planning, source extraction, GTM pack authoring, proposal pack authoring, shared boundaries.
- `mdp-pack-review/references/`: structural audit, routing/eval audit, installed-template QA.
- `mdp-gtm-brief/references/`: prospect fit/brief, pre-draft copy brief, supplied-copy review.
- `mdp-proposal-review/references/`: bid/no-bid, compliance, proof/win-theme, red-team.

References should be loaded by explicit conditions from `SKILL.md`, not linked through deep reference chains.
This follows the specification's progressive-disclosure and shallow-reference guidance and the creator guidance to use tested scripts for repeated deterministic logic: [best practices](https://agentskills.io/skill-creation/best-practices), [using scripts](https://agentskills.io/skill-creation/using-scripts).

The names deliberately keep `pack` in `mdp-pack-review` because the artifact under review is the MDP itself, not a prospect, copy draft, or proposal.
They keep `gtm` in `mdp-gtm-brief` because a generic `mdp-brief` would collide with proposal work; in this contract, a brief is the bounded decision artifact that may include fit, pre-draft guidance, or a supplied-copy assessment, never drafted or sent outreach.

## Current Skill Disposition

`Keep` means the public ID remains but its body and description may be rewritten.
`Replace` means a new public ID takes over the job and the old directory is removed.
`Merge` means the behavior becomes a mode or reference under another public skill and the old directory is removed.
No compatibility alias remains discoverable.

| Current skill | Disposition | Target | Rationale |
| --- | --- | --- | --- |
| `mdp` | Keep | `mdp` | Keep the explicit CLI/operator and mixed-work coordinator, but narrow its discovery claim so it does not compete with every specialized job. |
| `mdp-lfg` | Merge | `mdp` | A second broad orchestrator creates an indistinguishable trigger surface. Preserve useful routing logic inside `mdp`; remove the public ID. |
| `mdp-create-pack` | Replace | `mdp-pack-builder` | The new name describes the durable job and covers create plus improve across profiles. |
| `mdp-source-strategy` | Merge | `mdp-pack-builder` source-strategy mode | Source planning is a phase of evidence-grounded pack building, not a standalone MDP outcome. External collection remains outside MDP. |
| `mdp-source-extract` | Merge | `mdp-pack-builder` source-extraction mode | Extraction converts approved material into pack decisions and should share builder provenance and gap rules. |
| `mdp-icp-builder` | Merge | `mdp-pack-builder` GTM authoring reference | ICP/fit cards are one concern within a coherent GTM pack, not a separate public job. |
| `mdp-message-angles` | Merge | `mdp-pack-builder` GTM authoring reference | Message-angle cards belong to the pack-authoring workflow. |
| `mdp-cta-builder` | Merge | `mdp-pack-builder` GTM authoring reference | CTA policy is a pack decision and must stay bounded by the same source and no-send rules. |
| `mdp-avoid-rules` | Merge | `mdp-pack-builder` shared-boundaries reference | Avoid rules are cross-profile pack policy, not a user-facing workflow by themselves. |
| `mdp-output-rules` | Merge | `mdp-pack-builder` shared-output-contract reference | Output rules are cross-profile pack policy and should be authored with the pack. |
| `mdp-prospect-brief` | Replace | `mdp-gtm-brief` `prospect-fit-or-brief` mode | Prospect normalization, fit, and bounded brief generation form one GTM decision-support front door. |
| `mdp-copy-brief` | Merge | `mdp-gtm-brief` `outbound-copy-brief` mode | Pre-draft copy guidance shares the GTM profile, proof, fit, and no-send gate. |
| `mdp-copy-eval` | Merge | `mdp-gtm-brief` `outbound-copy-review` mode | Supplied-copy evaluation is the post-draft form of the same GTM decision contract. |
| `mdp-pack-review` | Keep | `mdp-pack-review` | Pack QA is a distinct user job with a clean trigger boundary. |
| `mdp-pack-eval` | Merge | `mdp-pack-review` routing/eval mode | Review and eval inspect the pack itself and otherwise collide heavily. |
| `mdp-proposal-pack-builder` | Merge | `mdp-pack-builder` proposal authoring reference | Pack construction is shared; proposal-specific privacy, proof, and boundary procedures remain in a dedicated reference. |
| `mdp-proposal-bid-no-bid-review` | Replace | `mdp-proposal-review` `bid-no-bid-review` mode | The job remains explicit inside one proposal review front door. |
| `mdp-proposal-compliance-review` | Merge | `mdp-proposal-review` `compliance-review` mode | Preserve the no-certification and escalation boundary without a separate startup trigger. |
| `mdp-proposal-win-theme-proof-review` | Merge | `mdp-proposal-review` `proof-review` mode | Proof-grounded theme review shares the proposal source, evidence, and human-review gate. |
| `mdp-proposal-red-team-gap-review` | Merge | `mdp-proposal-review` `red-team-review` mode | Red-team review is another output mode over the same proposal decision context. |

The result removes fifteen public IDs while preserving all intentional user jobs.
The downstream rebuild should delete every replaced or merged directory after its necessary behavior and eval coverage is represented under the target skill.

## Routing And Identifier Contract

### Canonical IDs

The five Agent Skill names are the canonical `skill_id` values.
They must match their directory names and `SKILL.md` frontmatter names, as required by the Agent Skills specification.

There is no separate `capability_id`.
There is no separate global capability registry.
Pack `jobs[].id` values are profile-scoped canonical operation identifiers and select the branch inside the bound skill.
The supported pairs are closed for v1:

- `mdp-gtm-brief`: `prospect-fit-or-brief`, `outbound-copy-brief`, `outbound-copy-review`;
- `mdp-proposal-review`: `bid-no-bid-review`, `compliance-review`, `proof-review`, `red-team-review`.

`mdp`, `mdp-pack-builder`, and `mdp-pack-review` are shared workflow skills rather than job-bound profile modes.
They route from explicit user intent and pack/profile state and do not require synthetic `jobs[]` entries.
Adding a new agent-routable job ID or supported pair is an intentional schema-contract change; packs may not introduce custom routable IDs.

An agent-routable job has one primary skill:

```yaml
jobs:
  - id: prospect-fit-or-brief
    label: Prospect row to fit decision or brief
    skill_id: mdp-gtm-brief
    required_primitives:
      - actors
      - decision-criteria
      - source-signals
      - evidence-proof
      - boundaries
      - output-contracts
      - routing-jobs
      - gaps
    input_contracts:
      - prospect
```

After the skill selects a canonical job ID from explicit user intent, the CLI validates the pair and returns that `job_id` as the selected mode.
The CLI does not interpret natural language.
The skill may execute CLI prerequisites or route internally, but the manifest does not list a chain of skills.

### Validation Rules

For an active profile:

- every agent-routable job must contain exactly one `skill_id`;
- every `skill_id` must exist in the packaged canonical catalog;
- every job-to-skill pair must be one of the closed profile-scoped pairs above;
- a job-bound profile skill is eligible only when at least one active job references it;
- shared skills (`mdp`, `mdp-pack-builder`, `mdp-pack-review`) remain eligible for valid packs;
- missing, unknown, duplicate, or profile-incompatible bindings are validation errors, not warning-first legacy fallbacks;
- no alias or old ID is accepted because MDP is pre-launch and this is a clean contract replacement.

## CLI, Pack, And Skill Responsibilities

| Layer | Owns | Must not own |
| --- | --- | --- |
| CLI | Canonical packaged skill catalog; schema validation; job-to-skill resolution; primitive/input readiness; deterministic fit, route, brief, claim, and output checks; machine-readable eligibility and reroute reasons | Host discovery or visibility claims; judgment-heavy source interpretation; prose workflow duplication |
| Pack | Profile ID; job IDs and labels; one `skill_id` per agent-routable job; required primitives; input contracts; approved evidence, boundaries, output rules, gaps, and eval fixtures | Copies of the global skill catalog; recommended/allowed/blocked skill lists; host-specific installation state; a separate capability registry |
| `SKILL.md` | Trigger-first discovery description; CLI-first gate; job-bound mode selection from a validated `job_id`; shared-skill routing from explicit user intent and pack/profile state; safety boundaries; explicit conditions for loading references | Reimplementing deterministic CLI logic; hard-coded copies of every profile manifest; claims that the host hid ineligible skills |
| References/assets/scripts | Focused mode instructions, templates, synthetic fixtures, and reusable deterministic helpers | Additional public routing IDs or deep chains that the main skill cannot reliably discover |

This division removes the current duplication between `allowed_skills`, `recommended_skills`, `blocked_skills`, `job_skills`, and the separate `jobs` array.
The pack declares the job once and binds it to one public skill.

## Discovery Versus Eligibility

Discovery and eligibility are different facts:

- **Packaged inventory** is the set of skill directories shipped in the installed MDP bundle.
- **Host discovery** is what the current agent host scanned and exposed to the model. MDP cannot prove or change this after the host has discovered the bundle.
- **Pack eligibility** is MDP policy for the active pack and job.
- **Recommendation** is the resolved `job_id -> skill_id` route for the user's requested job.

The CLI can compute the first, third, and fourth facts from its release and the active pack.
It cannot literally hide a skill that a host already discovered.
When describing CLI policy state, the replacement command and documentation must use `eligible` and `ineligible`.
Use `visible` or `hidden` only to explain that host discovery is separate and cannot be controlled by MDP; never claim that MDP changed host visibility.

The proposed replacement surface is one command with three deterministic forms:

```bash
mdp --json skills
mdp --json skills --dir <pack>
mdp --json skills --dir <pack> --job <job_id>
```

- Without `--dir`, it returns the release-declared canonical inventory and makes the three shared skills bootstrap-eligible.
- With `--dir`, it validates the pack and returns all valid profile routes.
- With `--dir --job`, it validates one closed job-to-skill route selected from explicit user intent and returns one recommendation. It does not choose a job from prose.
- For a missing or invalid pack, it returns the canonical inventory, structured validation diagnostics, and bootstrap eligibility for `mdp`, `mdp-pack-builder`, and `mdp-pack-review`; it emits no profile job routes until a valid active profile exists.

Its contract should be `mdp.skills.v1` and should contain at least:

```json
{
  "contract": "mdp.skills.v1",
  "pack": {},
  "profile": {},
  "packaged_skill_ids": [
    "mdp",
    "mdp-pack-builder",
    "mdp-pack-review",
    "mdp-gtm-brief",
    "mdp-proposal-review"
  ],
  "host_discovery": {
    "status": "unobserved",
    "managed_by": "agent-host",
    "guidance": "MDP eligibility does not hide skills already discovered by the host."
  },
  "eligibility": {
    "eligible_skill_ids": [
      "mdp",
      "mdp-pack-builder",
      "mdp-pack-review",
      "mdp-gtm-brief"
    ],
    "ineligible_skills": [
      {
        "skill_id": "mdp-proposal-review",
        "reason": "No active proposal review job binds this skill."
      }
    ]
  },
  "job_routes": [
    {
      "job_id": "prospect-fit-or-brief",
      "skill_id": "mdp-gtm-brief",
      "pack_ready": true,
      "missing_primitives": [],
      "required_input_contracts": ["prospect"]
    }
  ]
}
```

`pack_ready` means the pack has the declared primitives needed for the route; it does not claim that invocation-time inputs have been supplied.
Input payload validation remains the responsibility of the invoked CLI operation.
There is no fallback skill: `mdp` is recommended only for explicit MDP/operator or genuinely mixed MDP intent, never to bypass an invalid or ineligible specialized route.
The exact display ordering may change during MDP-107, but these semantics and field meanings should not.

Every profile-sensitive public skill must call this CLI surface before acting on a pack.
If the host activated an ineligible skill, the skill should stop or reroute using the CLI reason.
This self-gate is defense in depth; the authoritative calculation remains the CLI.

## Trigger And Failure-Mode Model

The five descriptions should be optimized and tested as a set, not independently.
The Agent Skills description guide treats the frontmatter description as the primary activation mechanism and recommends realistic positive, negative, and near-miss evals with held-out validation cases: [optimizing descriptions](https://agentskills.io/skill-creation/optimizing-descriptions), [evaluating skills](https://agentskills.io/skill-creation/evaluating-skills).

Minimum semantic cases:

| Case | Expected route |
| --- | --- |
| "Explain this `.mdp` pack and validate it with the CLI." | `mdp` |
| "Turn these approved positioning and proof notes into a new GTM MDP." | `mdp-pack-builder` |
| "Audit this pack for routing gaps and weak eval coverage." | `mdp-pack-review` |
| "Use this GTM pack and supplied prospect row to decide fit and produce a brief." | `mdp-gtm-brief` |
| "Review this supplied opportunity against the proposal pack for bid/no-bid." | `mdp-proposal-review` |
| "Write a cold email from these notes." | No MDP skill unless the user asks to use an MDP pack; do not hijack generic writing. |
| "Find prospects and enrich their contact details." | No MDP skill for execution; MDP is not enrichment or prospecting infrastructure. |
| "Certify that this proposal is compliant." | `mdp-proposal-review` may trigger only to refuse certification and provide bounded review support when proposal MDP context exists. |
| Host activates `mdp-proposal-review` for a GTM pack | CLI returns ineligible; skill stops and reroutes rather than proceeding. |
| Host activates `mdp-gtm-brief` for a proposal pack | CLI returns ineligible; skill stops and points to proposal review jobs. |

Output evals should test the selected job mode and safety boundary, while installed-artifact checks should verify that the host bundle contains exactly the five public skills.

## Single Authored Skill Source

`plugin/skills/` should be the only manually authored skill source.
That preserves the repository's declared canonical plugin package and keeps each `SKILL.md` beside the plugin metadata and assets that ship it.

The repository-root `skills/` tree should be removed if Pluxx can consume `plugin/skills/` directly; otherwise it may exist only as generated host staging.
`pluxx.config.ts` and every other packager should consume the canonical tree directly or a verified generated copy.
Contributors and agents must not edit generated copies directly.

Generation must copy the full skill package, including:

- `SKILL.md`;
- `references/`;
- `scripts/`;
- `assets/`;
- `evals/`;
- `agents/openai.yaml` or other required host metadata.

Validation must fail on missing, unexpected, stale, or divergent generated files and on any installed inventory that differs from the five-skill source catalog.

## Rejected Alternatives

### Keep All Public Skill IDs As The Complete Contract

This has no extra identifier layer, but the current twenty IDs encode card types and review stages as public user jobs.
It produces broad description overlap (`mdp` versus `mdp-lfg`, review versus eval, several pack builders), adds startup routing context, and requires profile manifests to repeat long allow/block lists.
Progressive disclosure does not solve catalog-level collisions because names and descriptions are all discovered before activation.

Rejected because the public surface is larger and less user-shaped than the product requires.

### Add Stable Capability IDs Mapped To Skills And Modes

This would allow capability names to remain stable while skill organization changes.
It could be justified if many hosts, plugins, or external integrations depended on capabilities independently of skills.

MDP has no such compatibility obligation, and the existing pack job IDs already name the domain operations that need stable local references.
A capability registry would add a catalog, mappings, validation, documentation, and eval fixtures without resolving a current user problem.

Rejected as an unnecessary abstraction.

### Use One Giant `mdp` Skill With Every Mode

This is the smallest inventory count but creates one description that must claim pack building, pack QA, GTM fit/copy support, and high-risk proposal review.
Once activated, its core instructions would either become large or depend on fragile internal routing before the correct safety procedure loads.
It also weakens the most valuable discovery boundary: proposal review must not be confused with GTM briefing or generic MDP operation.

Rejected because one skill is smaller numerically but less reliable operationally.

### Publish Separate GTM And Proposal Plugin Bundles

Separate bundles could make host discovery equal profile eligibility, but they would multiply installation, packaging, release, and source-of-truth paths.
Users may work with both pack types in one host, and MDP does not control per-directory plugin activation consistently across supported hosts.

Rejected because the five-skill bundle plus deterministic pack eligibility is simpler and portable.

## Consequences

### Benefits

- Startup catalog drops from twenty MDP skills to five.
- Public skills align with recognizable user jobs.
- Pack manifests stop duplicating allowed, recommended, blocked, and job-skill lists.
- Profile crossing remains deterministic through CLI eligibility and skill self-gates.
- Proposal safety remains a first-class public boundary without five nearly identical proposal skills.
- One authored source under the canonical plugin package supports Codex and Pluxx-generated hosts.
- The final inventory is small enough for exhaustive positive, negative, near-miss, and profile-crossing eval coverage.

### Costs And Risks

- `mdp-pack-builder` and `mdp-proposal-review` must have disciplined mode routing and explicit reference-load conditions.
- A direct job-to-skill binding makes skill renames contract changes; this is acceptable pre-launch and should become intentional after public release.
- Shared source rules must not be weakened when source strategy and extraction stop being public skills.
- Merged proposal modes need output schemas or assertions precise enough to prevent one review type from bleeding into another.
- The CLI and installed plugin catalog must be generated or verified from the same five-ID source to avoid false eligibility results.

## Implications For Downstream Issues

### MDP-107 — Replace CLI Agent Surface

- Replace `profile.agent_surface` and `mdp.agent-surface.v0`; do not add a parallel v1 compatibility path.
- Add `jobs[].skill_id` and validation against the five-ID canonical catalog.
- Add the no-pack, pack-catalog, and single-job forms of `mdp --json skills` with `mdp.skills.v1` semantics described above.
- Keep the supported profile-scoped job-to-skill pairs closed and reject custom routable job IDs.
- Derive eligibility and job recommendations instead of storing `recommended_skills`, `allowed_skills`, `blocked_skills`, and `job_skills`.
- Delete legacy fallback behavior and tests that exist only for unpublished pack compatibility.

### MDP-109 — Single Source And Generated Bundles

- Make `plugin/skills/` authoritative.
- Point Pluxx and supported host packagers at it directly where possible; generate staging copies only where a packager requires them.
- Verify full resource fidelity and exact five-skill inventory in source, generated artifacts, release assets, and installed bundles.
- Update eval and plugin validators to read the authoritative tree or a verified generated tree deliberately.

### MDP-108 — Rebuild The Public Skill Surface

- Implement exactly the five public skills in this ADR.
- Delete the fifteen replaced or merged public directories and all obsolete name references.
- Move necessary procedures into shallow references under the target skill.
- Rewrite templates, docs, OpenAI metadata, hooks, and fixtures to the new IDs and direct job bindings.
- Do not ship aliases, compatibility shims, migration commands, or discoverable deprecated skills.

### MDP-92 — Final Semantic And Installed Evals

- Rewrite the coverage manifest for five public skills and their job modes.
- Give every public description positive, negative, near-miss, and adjacent-skill collision cases with train/validation separation.
- Add GTM-to-proposal and proposal-to-GTM eligibility failures, including cases where a host discovered an ineligible skill.
- Add risk-tiered output assertions for GTM copy review and all four proposal review modes.
- Benchmark the installed release and require exact equality between CLI packaged inventory and host bundle inventory.
- Do not spend eval coverage on the deleted prototype names or migration behavior.

## Approval Gate

Brandon must approve this ADR before MDP-107, MDP-109, MDP-108, or MDP-92 begins implementation.
Approval confirms the five public IDs, the closed profile-scoped `job_id -> skill_id` pairs, no capability-ID layer, `plugin/skills/` as the authored source, and the discovery-versus-eligibility semantics.
