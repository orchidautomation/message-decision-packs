# Persona reference fixtures

These synthetic cards replace the initialized GTM starter's `personas` card in
CLI parity tests. The test adds canonical `Buyer` to `manifest.personas`.

- `declared-card.yaml` proves case-insensitive selectors remain routable while
  preserving authored display values.
- `undeclared-card.yaml` proves an `Architect` `applies_to` selector remains a
  default-mode warning and blocks strict validation. Its prose-only role mention
  is intentionally unrestricted.
