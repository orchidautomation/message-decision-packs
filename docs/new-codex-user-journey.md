# New Codex User Journey

## Purpose

This document describes the first useful experience for a brand new Codex user who installs the Message Decision Packs plugin and wants to understand what it does, how much work it takes, and why it matters.

MDP is a local/offline standard, CLI, and Codex plugin for modular GTM messaging decision context. It stores decisions, evidence, routing contracts, fit rules, approved claims, avoid-rules, output-rules, gaps, and brief inputs. It does not send messages, enrich leads, update CRM, scrape data, sequence outbound, or act as an AI SDR.

## Target User

The first user is a GTM operator, PMM, founder, or GTM engineer who already asks Codex to help with positioning, ICP, outbound copy, or prospect research.

Their current workflow usually looks like this:

```text
Paste product context.
Paste prospect context.
Ask for copy or a brief.
Correct claims manually.
Repeat with slightly different context next time.
```

The pain is not that Codex cannot write. The pain is that every thread rebuilds the source of truth from scratch, so claims drift, CTAs change, fit logic is implicit, and weak prospect signals get treated as facts.

MDP changes that operating model:

```text
Create a local pack once.
Store the messaging decisions in cards.
Route only the cards needed for the current job.
Check fit before drafting.
Check claims before approving.
Keep gaps explicit.
```

## Installation Model

MDP is distributed as a GitHub Release backed by Pluxx-generated agent bundles and native `mdp` CLI binaries. The public install path for the CLI plus supported agent bundles is one command:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Users who only need the CLI can run:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

The `--agents` command should install:

1. The local `mdp` CLI binary for the user's OS and architecture.
2. Agent/plugin bundles for the supported Pluxx targets.

The release shape is:

```text
GitHub release
- mdp-aarch64-apple-darwin
- mdp-x86_64-apple-darwin
- mdp-x86_64-unknown-linux-gnu
- install.sh
- install-cli.sh
- Pluxx-generated host installer scripts, such as install-codex.sh
- Pluxx-generated plugin archives
- release-manifest.json
- SHA256SUMS.txt
```

For users on other skill-aware agents that are not Pluxx release targets, document `skills.sh` as an optional compatibility path:

```bash
npx skills add https://github.com/orchidautomation/message-decision-packs --skill '*' --agent '*' -g -y
```

or:

```bash
npx skills add https://github.com/orchidautomation/message-decision-packs --skill '*' --agent universal -g -y
```

That fallback installs skills only. It does not replace the MDP installer because it does not install the `mdp` CLI binary or release runtime scripts.

## First Run Journey

### Step 1: The user installs the plugin

The user runs:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

The installer fetches the latest MDP release, installs the local CLI, and installs the supported agent/plugin bundles.

Expected user thought:

```text
I ran one command. What can I do now?
```

What Codex should do:

```text
Detect the installed MDP skills.
Use $mdp-lfg or $mdp when the user asks about Message Decision Packs.
Avoid hijacking generic LinkedIn/email writing unless MDP is explicitly mentioned or a .mdp pack is present.
```

### Step 2: The user checks local setup

The plugin should first verify whether the CLI is installed:

```bash
command -v mdp
mdp --json doctor --dir .
```

If the CLI is missing, the agent should point back to the public installer instead of asking the user to build from source:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

Expected lift:

```text
Current public lift: run one install command, restart/open the agent if a plugin bundle was installed, check mdp is on PATH.
```

### Step 3: The user creates their first pack

The user says:

```text
Create an MDP for our outbound messaging.
```

Codex runs:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir .
mdp --json validate --dir .
mdp --json eval --dir .
```

The result is a local `.mdp/` folder:

```text
.mdp/
  manifest.yaml
  sources.yaml
  briefs/
  cards/
  evals/
  examples/
  clay-row.json
```

The starter fixture path is kept for compatibility. It represents a synthetic provider-neutral prospect/source row, not a Clay dependency.

The user now has a local source of truth for messaging decisions. It can live in their repo, be reviewed in PRs, and be tested by CI.

### Step 4: The user fills the pack

The user brings product notes, ICP notes, source docs, website copy, sales snippets, or rough positioning.

Codex helps turn that into cards:

```text
personas
sources
fit-rules
signals
pains
claims
positioning
channel-policies
hooks
ctas
avoid-rules
output-rules
copy-patterns
objections
gaps
```

The important behavior is that Codex should not smooth over missing proof. Source facts and interpretations start in `sources.yaml`, missing evidence goes into `gaps`, weak source signals stay hypotheses, and unsupported claims stay out of `claims`.

### Step 5: The user routes a task

The user asks:

```text
Give me a LinkedIn brief for this prospect.
```

Codex normalizes the prospect into ignored scratch, checks fit, and builds a brief:

```bash
mdp --json fit --dir . --prospect <prospect.json>
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel linkedin --out .mdp/briefs/<brief-name>.json
```

The prospect JSON supplies the live input for the decision tree. `trigger` is the row-level reason to write now; `persona`, `title`, `segment`, `signals`, and `background` provide the rest of the routing context.

```text
prospect.json
  |
  +-- persona or title -> persona route
  +-- trigger ---------> why this message now
  +-- segment ---------> market/account context
  +-- signals ---------> supporting facts or hypotheses
  |
  v
fit gate
  |
  +-- no fit or missing context -> no-draft
  |
  v
persona
  |
  +-- pains for that persona
  |     |
  |     v
  |   hooks for those pains
  |     |
  |     v
  |   claims/proof allowed by sources
  |     |
  |     v
  |   CTA and channel policy
  |
  v
avoid rules
  |
  v
bounded context.entries
```

If the prospect is too thin, the result is not a draft:

```text
status: insufficient-context
draft_status: no-draft
missing: trigger, persona, segment, signals, source
```

That is a feature. It prevents Codex from writing polished copy from weak inputs.

### Step 6: The user drafts only when fit passes

When fit passes, Codex uses `context.entries` from `mdp brief --context`, not every card in the pack.

That gives a smaller, more auditable context window:

```text
Use matched persona, fit, claim, CTA, and channel entries.
Keep avoid-rule guardrails in the bounded context.
Open `full_card_required` paths only when the brief says to.
Draft from those entries plus prospect context.
```

The user-visible difference is that the answer should sound less generic and be easier to audit:

```text
Why this angle?
Which claim did it use?
What source supported it?
What gap did it surface?
What did it refuse to invent?
```

### Step 7: The user checks the draft

Before approving copy, Codex runs:

```bash
mdp --json check-claims --dir . --text "<draft copy>"
```

This catches obvious unsupported claims:

```text
guarantees meetings
improves reply rates by 30%
integrates with Salesforce or HubSpot
updates CRM
SOC 2 compliant
trusted by named customers
```

If a claim is unsupported, Codex revises the copy or asks for source evidence. It should not silently approve the message.

## Why It Matters

MDP gives Codex a durable operating contract for GTM messaging.

Without MDP:

```text
Every thread is a fresh prompt.
Claims depend on memory and vibes.
Fit is implicit.
Evidence gaps disappear.
Review is manual.
```

With MDP:

```text
Messaging decisions live in files.
Routing is deterministic.
Fit is a gate.
Claims are checked.
Gaps are explicit.
The pack can be validated in CI.
```

The value is not only better copy. The value is making Codex safer to use on GTM work because the source of truth is local, reviewable, testable, and bounded.

## User Lift

| Stage | Current lift | Target lift |
|---|---:|---:|
| Install CLI | Low: release installer | Low: release installer |
| Install plugin | Low: `--agents` install | Low: one command for supported hosts |
| Create starter pack | Low | Low |
| Make pack useful | Medium | Medium |
| Use for one prospect | Low | Low |
| Keep pack current | Medium | Medium |
| Share with team | Medium/high | Medium with repo marketplace or workspace sharing |

The main lift is not technical setup after packaging exists. The real lift is deciding the messaging truth: who the ICP is, what claims are approved, what evidence exists, and when not to draft.

## What We Need Before A Clean Public Journey

### Required

1. Keep GitHub Release automation green for CLI binaries, Pluxx bundles, installers, and checksums.
2. Keep the vanity installer URL working:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

3. Document install commands for Codex app, CLI-only users, supported agent bundles, and long-tail `skills.sh` users.
4. Re-run release QA on a fresh machine before each public release.
5. Add release validation that runs:

```bash
cargo fmt --check
cargo test
cargo run -- --json validate --dir ../plugin/assets/templates/basic
cargo run -- --json eval --dir ../plugin/assets/templates/basic
python <plugin-validator> plugin
```

### Nice To Have

1. Homebrew formula after CLI releases stabilize.
2. A short demo video or appshot showing install -> create pack -> brief -> claim check.
3. A `mdp doctor install` style command that explains missing CLI/plugin pieces.
4. A sample real-ish pack that is more substantive than the neutral starter but still contains no private customer data.
5. First-class Pluxx targets for additional agents if `skills.sh` fallback usage becomes common.

## Activation Moment

The first "this matters" moment is not pack creation. It is when Codex refuses to draft from a thin prospect and says exactly what context is missing.

The second moment is when a draft fails `check-claims` because it invented a performance claim or integration.

The third moment is when the user edits a card, reruns evals, and sees the pack behave differently in a controlled way.

That is the product story:

```text
MDP turns GTM prompting into a local, testable decision system.
```

## First User Success Criteria

A brand new Codex user succeeds when they can do all of this without asking Brandon for help:

1. Install the `mdp` CLI.
2. Install the Message Decision Packs plugin.
3. Create or open a `.mdp/` pack.
4. Validate the pack.
5. Run evals.
6. Add a prospect row without committing private data.
7. Get `fit` and `brief` output.
8. Draft only when fit passes.
9. Run `check-claims`.
10. Understand that sending or CRM updates are outside MDP.

## Recommendation

Ship the next milestone as a local/offline developer preview:

```text
GitHub Releases for CLI binaries and Pluxx plugin bundles.
Vanity installer URL for the default one-command path.
skills.sh documented as a skills-only fallback for long-tail agents.
Starter pack plus evals.
Docs for first use and boundaries.
No hosted API.
No public package registry yet.
No sending or CRM actions.
```

This keeps the promise narrow and useful. Users get a working local standard and Codex workflow without us pretending MDP is execution infrastructure.
