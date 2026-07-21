---
name: mdp-pack-review
description: "Use when auditing, validating, hardening, testing, or diagnosing an existing Message Decision Pack itself: structure, evidence, jobs, routes, prompts, gaps, rules, or evals. Do not use to review a prospect, copy draft, proposal, or RFP."
---

# MDP Pack Review

Review an existing pack and produce evidence-backed findings. Do not silently repair it unless the user also asks for changes.

## Gate

Identify the exact pack root and inspect its policy state:

```bash
mdp --json skills --dir PACK_ROOT
mdp --json doctor --dir PACK_ROOT
```

This is a shared skill and remains eligible for invalid packs so it can diagnose them. Host discovery remains unobserved and host-managed.

## Deterministic Review

Run the narrow checks first, then strict gates:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
mdp --json eval --dir PACK_ROOT
mdp --json validate --strict --dir PACK_ROOT
mdp --json eval --strict --dir PACK_ROOT
```

Preview a portable compilation when needed:

```bash
mdp --json pack --dir PACK_ROOT --out PACK_JSON --dry-run
```

Read [references/structural-audit.md](references/structural-audit.md) for manifest, primitive, evidence, and content review. Read [references/routing-evals.md](references/routing-evals.md) for job binding, route, prompt, and eval review. Read [references/installed-template-qa.md](references/installed-template-qa.md) only when testing a released install or freshly initialized templates.

## Review Rules

- Treat CLI errors as findings, not prose to reinterpret away.
- Verify every agent-routable `jobs[]` entry has one canonical `skill_id` and a supported closed pair.
- Check source receipts, freshness, confidence, approved claims/proof, avoid rules, output rules, and gaps for internal consistency.
- When `manifest.target` exists, verify target kind/name, source IDs, aliases, supported external terms, exclusions, and internal vocabulary boundaries. Treat target contamination as a high-severity wrong-product risk.
- Distinguish structural validity from commercial readiness or human approval.
- Sample representative routes and deterministic claim/output gates when the pack changed those decisions.
- Exercise generated surfaces such as sample leads, prompt output, JSON/readable briefs, run receipts, and eval payloads; required contracts and CLI receipts are implementation metadata, while their prospect-facing content must remain target-aware or neutral.
- For proposal/document normalization QA, require `mdp run-receipt` before calling the flow audit-grade; paid-pilot QA should include `--runner-audit ... --require-runner-audit`. Ambient, unknown, missing runner-audit, or invalid runner-audit context isolation is an advisory/blocking finding even when prompt-output validation passes.
- Keep evaluation output and temporary packs outside committed source paths.

## Findings

Report findings first, ordered by severity:

- High: invalid pack, unsafe boundary, unsupported proof, broken job binding, or route/eval behavior that can produce a wrong decision.
- Medium: ambiguous decision context, weak evidence, missing high-value fixture, or output rule that is not enforceable as written.
- Low: clarity, duplication, naming, or maintainability issue with limited behavior risk.

For each finding include the file or CLI path, evidence, impact, and smallest durable fix. If there are no findings, say so and list the commands and coverage limits.

## Boundaries

Pack review does not enrich prospects, review supplied copy/proposals as business artifacts, certify compliance, submit work, or mutate downstream systems. Route those user jobs to the appropriate specialized skill.
