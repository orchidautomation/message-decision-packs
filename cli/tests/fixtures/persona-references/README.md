# Persona reference fixtures

These synthetic cards replace the initialized GTM starter's `personas` card in
CLI parity tests. The test adds canonical `Buyer` to `manifest.personas`.

- `declared-card.yaml` proves case-insensitive selectors remain routable while
  preserving authored display values.
- `undeclared-card.yaml` proves an `Architect` `applies_to` selector remains a
  default-mode warning and blocks strict validation. Its prose-only role mention
  is intentionally unrestricted.
- `universal-gap-card.yaml` proves an empty card `personas` selector and an
  empty entry `applies_to` selector route a neutral gap for every persona;
  the non-empty comparison remains persona-scoped.

Empty and blank-only selectors are universal only for persona applicability.
They do not bypass job/channel policy, portfolio scope, guardrail, card-cap, or
context-budget gates.
