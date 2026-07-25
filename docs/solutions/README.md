# MDP Solution Notes

Solution notes capture a verified, reusable engineering lesson after a problem
is solved. They are not a second roadmap, issue tracker, or source of product
contracts.

## Capture Loop

1. **Trigger:** capture only after a fix or decision has validation evidence.
2. **Sanitize:** remove customer data, secrets, private URLs, raw transcripts,
   local-only paths, and access-controlled source material.
3. **Write:** add `docs/solutions/YYYY-MM-DD-short-slug.md` using
   [`TEMPLATE.md`](TEMPLATE.md).
4. **Connect:** link the owning public issue ID, changed contract/docs, and
   reproducible validation command.
5. **Promote:** if the lesson changes vocabulary, update
   [`CONCEPTS.md`](../../CONCEPTS.md); if it changes behavior, update CLI,
   template, docs, and canonical `plugin/skills/` guidance together.
6. **Recheck:** run the narrow tests and `make validate` before publishing.

Do not capture speculative advice, private incident details, or fixture-only
evidence as production proof.
