# Route-budget preflight examples

Synthetic example packs for the `mdp route-budget` generation-time preflight.
These packs are copied from the starter GTM template and patched to
demonstrate the 99-persona-match plus guardrail failure shape that MDP-65
requires generation handoff to catch before declaring a pack finished.

Both packs declare the same `outbound-copy-brief` context budget
(`max_entries: 64`, `max_bytes: 65536`) and the same `Buyer` persona. They
differ only in how broadly a synthetic `buyer-case-studies` claims card stamps
`applies_to: Buyer`:

- `overflow/` stamps all 99 entries with `applies_to: Buyer`. Every entry
  matches the Buyer persona, so the routed context overflows the declared
  budget. `mdp route-budget` fails, `validate --strict` fails, and
  `route --entries --persona Buyer --job outbound-copy-brief` blocks without
  exposing model-visible context.
- `ready/` narrows the same card to 5 Buyer-scoped entries. The routed
  context fits the 64/65536 budget through structured applicability, not
  larger limits, and `brief --context --dry-run` produces a ready governed
  context for a supported persona.

The fixtures are intentionally synthetic and public-safe. They assert no real
customer outcome, certification, compliance status, or past performance, and
they do not touch the audited Sanity.io pack. Regenerate them with:

```bash
node scripts/build-route-budget-fixtures.mjs
```

Run the focused validation:

```bash
make validate-route-budget
```
