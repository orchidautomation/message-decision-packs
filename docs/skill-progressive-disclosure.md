# Skill progressive-disclosure evidence

MDP-257 reduces activation-time context while preserving the Rust CLI as the
decision authority. Measurements use UTF-8 file bytes at baseline commit
`f1586d771d44b66e241d347ebd2b1128fe746762` and the refactored source tree.

| Entrypoint | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| `mdp` | 22,330 | 5,681 | 74.6% |
| `mdp-pack-builder` | 28,005 | 4,534 | 83.8% |
| `mdp-pack-review` | 18,513 | 4,262 | 77.0% |
| `mdp-pack-apply` | 30,022 combined vertical baseline | 5,640 | 81.2% |
| **Total** | **98,870** | **20,117** | **79.7%** |

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
- `mdp-pack-apply`: resolve the exact profile/job through the CLI, load one
  direct job reference, and load the matching governed execution contract only
  when normalized input, routed context, or receipts are needed.

## Versioned eval delta

`plugin/skill-evals/trigger-cases.json` and `coverage.json` carry revision
`mdp-257.v1`. The current corpus retains distinct GTM and proposal modes but
expects the same neutral `mdp-pack-apply` owner. Explicit builder, read-only
pack-review, and apply intent remain distinct from the `mdp` coordinator.
