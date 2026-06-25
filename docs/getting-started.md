# Getting Started

Message Decision Packs (MDP) are local/offline files plus a local `mdp` CLI and agent plugin. MDP stores GTM messaging decisions, routing contracts, fit rules, approved claims, avoid-rules, and evidence gaps. It does not send messages, update CRM, enrich leads, scrape data, sequence outbound, or act as an AI SDR.

If you want the mental model first, read [Conceptual Decision Flow](conceptual-decision-flow.md). It explains how a prospect row moves through fit, persona, pains, hooks, proof, CTA policy, avoid-rules, and bounded context for drafting.

## Install

Install the CLI and supported agent bundles:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Portable shell fallback:

```bash
curl -fsSL https://mdp.orchidlabs.dev/install.sh | bash -s -- --agents -y
```

The installer fetches the latest GitHub Release, installs the `mdp` CLI for your platform, and installs Pluxx-generated bundles for supported agent hosts.

## Verify

```bash
mdp --version
mdp --json doctor --dir .
```

If `mdp` is not found, make sure the install directory printed by the installer is on `PATH`, then restart your agent host.

## Create A Starter Pack

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir ./mdp-demo --force
mdp --json validate --dir ./mdp-demo
mdp --json eval --dir ./mdp-demo
```

The starter creates:

```text
mdp-demo/
  .mdp/
    manifest.yaml
    sources.yaml
    briefs/
    cards/
    evals/
  examples/
```

## Route Context

Ask which cards matter for a persona and job:

```bash
mdp --json --summary route --entries --eval-fixture --dir ./mdp-demo --persona "PMM" --job "linkedin outbound copy"
```

Agents should load only the returned cards instead of reading the entire pack by default.

Use the returned `eval_fixture` as a scaffold for route tests. Review it before committing so evals encode intended behavior, not accidental routing noise.

## Use A Prospect Row

Keep private prospect data in ignored scratch unless you intentionally commit a sanitized example. Check fit before drafting:

```bash
mdp --json fit --dir ./mdp-demo --prospect ./mdp-demo/examples/clay-row.json
```

If a prospect row has no explicit `persona`, the CLI can use pack-owned `.mdp/manifest.yaml` `persona_mappings` to map title keywords to personas. Unmapped title fallbacks are reported as low-confidence and still require review.

If fit returns `disqualified` or `insufficient-context`, do not draft unless the user explicitly overrides.

When fit is acceptable, build the brief:

```bash
mdp --json --summary brief --dir ./mdp-demo --prospect ./mdp-demo/examples/clay-row.json --channel linkedin --out ./mdp-demo/.mdp/briefs/example-linkedin.json
```

Draft from the brief's `required_load_order`, the prospect context, and the approved pack cards. Use `--out` when the brief should exist as a file; without it, the CLI reports the artifact as stdout-only.

The generated `examples/clay-row.json` is a synthetic fixture, not a real prospect. It includes `source_kind: synthetic-example` and `synthetic: true`.

## Source Ledger

Use `.mdp/sources.yaml` before bulk card writing. Add public URLs, user-provided docs, or note identifiers, then separate direct source claims from interpretations and gaps. Cards should cite source ids, URLs, or document names from the ledger when possible.

## Check Claims

Before approving copy, run:

```bash
mdp --json check-claims --dir ./mdp-demo --text "<draft copy>"
```

Unsupported claims, execution claims, compliance/security claims, named-customer claims, and quantified outcome claims should be fixed or backed with source evidence before use.

## Update

Rerun the installer:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

To check whether your local CLI/plugin version is current:

```bash
scripts/check-update.sh
```

## Long-Tail Skill Clients

For skill-aware agents that are not first-class Pluxx release targets, `skills.sh` can install the `SKILL.md` files only:

```bash
npx skills add orchidautomation/message-decision-packs --skill '*' -g -a <agent> -y
```

This does not install the `mdp` CLI. Use the MDP installer for the full CLI plus agent bundle setup.
