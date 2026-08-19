# Boundaries And Output Contracts

Read this when authoring claims, avoid rules, output rules, or proof requirements.

## Boundaries

Encode forbidden or unsupported claims, source-access limits, bad-fit/no-message conditions, refusal conditions, human approval requirements, channel/review limits, and exclusions from public artifacts.

Use literal `avoid` values for text the CLI should flag. Keep judgment-heavy boundaries in explicit entry bodies and eval fixtures.

## Output Contracts

Encode deterministic constraints where supported: word limits, subject limits, question/link/HTML/tracking restrictions, paragraph counts, required evidence bindings, and no-meta-commentary rules.

Use `mdp check-claims` for supplied text and `mdp verify-output` for proof-carrying output. Do not call a claim approved merely because it sounds consistent with the pack.

## Approved Boundaries Are Entries, Not Gaps

A boundary is authority, not a hole. Rules that scope or limit approved
authority belong in `avoid`/`output` entries and selected guardrail facets,
never in `gaps`:

- “Approved terminology is set; naming outside it requires review” is a
  terminology entry plus an avoid entry, not `terminology-missing`.
- “Case-led proof is allowed; portfolio-wide extrapolation is prohibited” is a
  `proof_boundaries` entry, not `proof-partial-missing`.
- “Case-specific outcomes are allowed; generic averages are prohibited” is an
  outcomes entry plus an avoid entry, not `outcomes-partial-missing`.

Author such a boundary as one entry with its exact `avoid`/`output` rules and
keep the facet free of `gaps`. Reserve a `gaps` ref for genuinely unresolved
job insufficiency: no approved source establishes the required authority at
all. The builder and reviewer own this classification; the Rust CLI never
infers gap meaning from prose and never auto-closes a gap from keywords such
as “approved” or “resolved.”

## Evidence Discipline

- Source IDs are not proof until resolved against the loaded pack.
- Every material claim needs an approved claim/proof binding or must remain a gap.
- Missing proof cannot be repaired with generic marketing language.
- Synthetic fixtures must be labeled synthetic and must not resemble real customer evidence.
- A selected required gap is a veto. Do not relabel missing authority as a
  boundary to clear it; do not relabel an approved boundary as a gap either.
