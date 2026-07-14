# Boundaries And Output Contracts

Read this when authoring claims, avoid rules, output rules, or proof requirements.

## Boundaries

Encode forbidden or unsupported claims, source-access limits, bad-fit/no-message conditions, refusal conditions, human approval requirements, channel/review limits, and exclusions from public artifacts.

Use literal `avoid` values for text the CLI should flag. Keep judgment-heavy boundaries in explicit entry bodies and eval fixtures.

## Output Contracts

Encode deterministic constraints where supported: word limits, subject limits, question/link/HTML/tracking restrictions, paragraph counts, required evidence bindings, and no-meta-commentary rules.

Use `mdp check-claims` for supplied text and `mdp verify-output` for proof-carrying output. Do not call a claim approved merely because it sounds consistent with the pack.

## Evidence Discipline

- Source IDs are not proof until resolved against the loaded pack.
- Every material claim needs an approved claim/proof binding or must remain a gap.
- Missing proof cannot be repaired with generic marketing language.
- Synthetic fixtures must be labeled synthetic and must not resemble real customer evidence.
