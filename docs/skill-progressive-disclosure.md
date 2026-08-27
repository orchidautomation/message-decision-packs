# Skill progressive-disclosure evidence

MDP-257 reduces activation-time context while preserving the Rust CLI as the
decision authority. Measurements use UTF-8 file bytes at baseline commit
`f1586d771d44b66e241d347ebd2b1128fe746762` and the refactored source tree.

| Entrypoint | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| `mdp` | 22,330 | 3,893 | 82.6% |
| `mdp-pack-builder` | 28,005 | 3,509 | 87.5% |
| `mdp-pack-review` | 18,513 | 3,743 | 79.8% |
| `mdp-gtm-brief` | 14,296 | 3,511 | 75.4% |
| `mdp-proposal-review` | 15,726 | 3,649 | 76.8% |
| **Total** | **98,870** | **18,305** | **81.5%** |

The validator caps each `SKILL.md` at 6,000 bytes and rejects a local Markdown
reference from any `references/*.md` file. That keeps every conditional load to
one direct hop from the entrypoint.

## Load map

- `mdp`: route first; load operator/runtime, managed-run, conformance, or
  mental-model detail only for that operator journey.
- `mdp-pack-builder`: select source-plan, source-extract, GTM authoring, or
  proposal authoring; load safe-authoring before mutation and only the selected
  content reference.
- `mdp-pack-review`: select structural, routing-eval, or installed-QA; load the
  matching review reference without taking edit ownership.
- `mdp-gtm-brief`: select exactly one GTM job; load that mode and governed
  execution only when normalized input, routed context, or receipts are needed.
- `mdp-proposal-review`: select exactly one proposal job; load its mode,
  evidence path when assurance is in question, and governed review only after
  selection.

## Versioned eval delta

`plugin/skill-evals/trigger-cases.json` and `coverage.json` carry revision
`mdp-257.v1`. Eight train/validation cases prove that explicit builder,
read-only pack-review, GTM-use, and proposal-review intent retains its
specialized owner even when the request names MDP. Each new case records an
explicit collision against the `mdp` coordinator. Existing authoritative job
mappings and output assertions are unchanged.
