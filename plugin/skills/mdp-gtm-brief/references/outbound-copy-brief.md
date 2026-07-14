# Outbound Copy Brief

Read this only for `outbound-copy-brief`.

## Workflow

1. Run `mdp fit` on the supplied prospect JSON. Stop on insufficient or disqualified.
2. Build bounded pre-draft context:

```bash
mdp --json --summary brief --context --dir PACK_ROOT --prospect PROSPECT_JSON --channel CHANNEL
```

3. Return a writing contract containing the audience/persona, fit rationale, safe personalization, approved claims/proof, message angles, CTA policy, avoid rules, output constraints, and known gaps.
4. If no prospect object is required for the pack-owned route, use `mdp emit-brief` with the exact persona, job, and required scope.

## Boundary

The output is a brief, not outbound copy. Do not write subject lines, opening lines, emails, DMs, sequences, or send instructions. A downstream writer must remain within the brief and run claim checks on any draft.
