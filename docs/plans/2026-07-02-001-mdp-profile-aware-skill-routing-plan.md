# MDP-52 Profile-Aware Skill Routing Plan

## Objective

Add a deterministic, pack-owned way for agents to decide which MDP skills are relevant to a template/profile before editing or running domain-specific workflows.

## Scope

- Add optional `.mdp/manifest.yaml` `profile.agent_surface` metadata.
- Add `mdp --json agent-surface --dir <pack>` as a read-only orchestration command.
- Keep legacy packs valid. Missing profile metadata falls back to generic MDP skills.
- Teach GTM and proposal templates which skills are recommended, allowed, and blocked.
- Update representative skills to self-gate against the active pack surface.

## Non-Scope

- Do not add new core card kinds for proposal or GTM variants.
- Do not enforce `primitive_map` yet.
- Do not add a proposal opportunity normalizer in this slice.
- Do not turn MDP into an execution layer, sender, scraper, CRM, or proposal management system.

## Contract

The manifest may declare:

```yaml
profile:
  id: gtm
  label: GTM Messaging
  version: mdp.profile.v0
  agent_surface:
    recommended_skills:
      - mdp
      - mdp-icp-builder
    allowed_skills:
      - mdp
      - mdp-icp-builder
    blocked_skills:
      - name: mdp-proposal-pack-builder
        reason: Use only with profile.id: proposal.
    job_skills:
      - job: create or improve GTM messaging pack
        skills:
          - mdp-icp-builder
```

Agents should call `mdp --json agent-surface --dir <pack>` before choosing domain-specific MDP skills. `blocked_skills` is deterministic reroute guidance; prose prompts should not override it.

## Future Work

- Add first-class `primitive_map` validation after profile metadata has usage evidence.
- Add proposal-owned opportunity normalization only when there is a stable input contract.
- Consider host/plugin runtime filtering after Pluxx or first-party plugin surfaces expose deterministic per-template skill loading.
