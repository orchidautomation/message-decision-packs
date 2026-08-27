# Compound Learning

After a validated MDP fix produces a reusable lesson, use the current
repository's own solution index and template when it provides them. Their
presence is optional and is not an installed-skill prerequisite. Otherwise
return the sanitized lesson in the handoff without inventing a repository
path.

Capture only verified facts and reproducible validation. Never include private
customer material, credentials, raw transcripts, private endpoints, or
local-only paths. Link a public issue identifier, not private Linear
discussion.

Update `CONCEPTS.md` only when a reusable term or state boundary changes.
Behavior changes must still update CLI contracts, templates, docs, and the
canonical authored skill under `plugin/skills/`; a solution note cannot replace
those sources of truth.
