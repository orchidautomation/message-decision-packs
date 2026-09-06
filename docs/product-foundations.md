# Product Foundations

A product foundation is a profile-owned index that tells an exact canonical
job which existing structured product authority it needs. It does not copy
product prose into the manifest and does not add an eleventh primitive, a new
`CardKind`, or a company wiki.

The referenced card entries and explicit gap entries remain authoritative.
The registry, job binding, CLI resolution, and `.mdp/README.md` are projections
or navigation over that authority.

## Manifest Contract

Declare reusable facets under `profile.product_foundation`:

```yaml
profile:
  id: gtm
  product_foundation:
    facets:
      - id: product-identity
        kind: product_identity
        entries:
          - card_id: positioning
            entry_id: target-identity
      - id: proof
        kind: claims
        entries:
          - card_id: claims
            entry_id: approved-proof
        gaps:
          - card_id: gaps
            entry_id: missing-customer-proof
        conflicts_with:
          - proof-restrictions
```

Facet IDs use lowercase kebab-case. Facet kinds use the closed product-foundation
vocabulary in the manifest schema. Every `entries` reference names an exact
existing card and entry. Every `gaps` reference names an exact entry in a
`gaps` card. `conflicts_with` records explicit structural incompatibility; the
CLI never compares prose for semantic conflict and never chooses a winner.

Bind facets to one exact canonical `jobs[].id`:

```yaml
jobs:
  - id: outbound-copy-brief
    skill_id: mdp-pack-apply
    product_foundation:
      required:
        - product-identity
      conditional:
        - facet_id: proof
          when:
            fact: job_id
            equals: outbound-copy-brief
```

Each facet may be classified once per job. Conditions are static equality
checks over `manifest_id`, `profile_id`, or `job_id`; there is no expression
language and no runtime-input or prose inference. Required facets and
conditionals whose equality check is true form the selected authority.
Optional facets remain inspectable but are not loaded into the minimal selected
set. Excluded and false conditional facets are not selected.

## Exact Job Resolution

Use exact canonical IDs from `jobs[].id`:

```bash
mdp --json skills --dir PACK_ROOT --job JOB_ID
mdp --json requirements --dir PACK_ROOT --job JOB_ID
mdp --json route --dir PACK_ROOT --persona PERSONA --job JOB_ID --entries
```

`skills --job` returns a compact `product_foundation` summary. `requirements`
returns the complete selected facets, exact entry/gap references, bounded entry
content, optional/excluded/untriggered IDs, and diagnostics. Route, context,
and brief outputs expose the exact selected reference load order. Do not
replace the canonical ID with a natural-language approximation: unknown or
free-text jobs are `unassessed` for product foundation and never gain authority
through keyword matching.

Only required and triggered conditional facets can enter selected product
context. Optional, excluded, unrelated-job, and untriggered facet content must
not leak into that context.

## Status And Blocking

The computed status is not stored in the manifest. The three computed states
are `unassessed`, `ready`, or `blocked`.

- `unassessed` — the job has no binding, or the supplied job is not an exact
  canonical manifest job. This preserves legacy pack validity without claiming
  product sufficiency.
- `ready` — every selected required or triggered facet resolves to existing
  authority, has no explicit gaps, and has no explicit conflict with another
  selected facet.
- `blocked` — selected authority is empty, dangling, explicitly gapped, or in
  an explicit selected-facet conflict.

Optional, excluded, and false conditional facets do not block the selected
job. A conflict matters only when both facets are selected.

Foundation readiness is a veto-only input to broader readiness. `blocked`
prevents job/profile activation and agent-visible `pack_ready`. `ready` never
promotes an otherwise unready pack and never establishes
`sufficient-for-job`, self-standing status, commercial readiness, factual
truth, human approval, or audit-grade execution. An explicit
`profile_eval.activation.status` of `needs-review` or `blocked` also prevents
activation even when the foundation resolves as ready.

## Targeted Packs And Gaps

New targeted GTM packs start with the target identity and safety boundaries
that initialization can support. Unsupported product facts, ICP detail,
claims, proof, certifications, integrations, and outcomes remain explicit gaps.
Those selected gaps correctly produce `blocked`, while the profile remains
`needs-review`. Replace a gap only with reviewed structured authority and its
source receipt; never fill it from a chat, README, target name, or plausible
industry knowledge.

Proposal packs follow the same resolver contract while preserving stricter
privacy boundaries. Keep raw or non-public RFPs, customer names, pricing,
regulated material, and private proof in customer-controlled paths. Never
invent requirements, certifications, compliance status, past performance, or
approval.

## README Orientation

`.mdp/README.md` is concise human orientation and secondary navigation. It may
summarize the thesis, actors, supported jobs, decision flow, boundaries,
sources, prompts, commands, and known gaps. Agents must inspect CLI-resolved
foundation output first. README prose cannot satisfy a facet, close a gap,
resolve a conflict, change activation, or override a card entry, source,
contract, or CLI diagnostic.

The README is still a regular file inside `.mdp/`, so it participates in the
portable pack snapshot and changing it changes the portable pack hash. This is
an identity/integrity effect, not a decision-authority effect: two packs with
different README bytes can have different portable hashes while resolving the
same product foundation and readiness.

## Compatibility And Review

Existing packs without product-foundation declarations remain valid and report
`unassessed`. Adoption is additive: author a profile registry, bind canonical
jobs, validate exact references, and review each job independently. Do not
rewrite existing cards into a universal product card or duplicate their prose
in the manifest or README.

Review every opted-in job with `validate`, `skills --job`, and `requirements
--job`. Confirm selected IDs and load order, verify optional/excluded content is
absent, and test empty, gap, dangling, conflict, and irrelevant-facet cases.

### Decision groups and temporal state

Decision groups are typed manifest membership and governance metadata. They
reference exact existing card entries and canonical jobs rather than copying
decision prose. Source observation/publication age, decision review cadence,
and lifecycle are separate states; unknown dates are not inferred from file
mtime, Git, or the current clock. A source digest proves byte identity only and
cannot approve, supersede, or rewrite a decision.
