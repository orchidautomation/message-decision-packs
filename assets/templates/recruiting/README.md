# Recruiting Reference Profile Sample

This fictional, public-safe pack demonstrates a Recruiting profile over the ten universal MDP primitives. It prepares local role-requirements, candidate-evidence, interview-question, and scorecard-gap artifacts for human review.

MDP is not an ATS, job board, sourcing or enrichment provider, scraper, background-check service, employee database, scheduler, ranker, rejection engine, hiring decision maker, legal reviewer, or compliance certifier.

Candidate A, Example Hiring Team, the role, and every source are synthetic. Do not replace them in this public template with resumes, applications, interview notes, identifying data, access-controlled material, or other real candidate records.

For a practical walkthrough of when a recruiter would use this MVP, how the review moves from supplied evidence to a human-owned handoff, and what the recruiter gains, see [Recruiting Reference Profile: Recruiter User Story](https://github.com/orchidautomation/message-decision-packs/blob/main/docs/recruiting-user-story.md).

The profile keeps the candidate as an evidence subject and uses `Recruiter`, `Hiring Manager`, and `Interviewer` as operator personas. The normalization prompt emits profile-neutral `normalized_context`; `human-review-ready` and `ready_for_review` mean only that enough permitted context exists to prepare the requested review artifact. They never mean candidate fit, advancement, rejection, or a hiring recommendation.

Real local context defaults to an opaque subject ID with no display name. Prompt output also reports expected sources as present, empty, or missing and carries a human-review handoff with the accountable owner, source snapshot, unresolved gaps, and safe next action.

Run:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json validate --strict --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json agent-surface --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json route --entries --dir plugin/assets/templates/recruiting --persona "Recruiter" --job "candidate evidence review"
cargo run --manifest-path cli/Cargo.toml -- --json gaps --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json check-claims --dir plugin/assets/templates/recruiting --persona "Recruiter" --job "candidate evidence review" --text "Rank candidates and reject the bottom candidate."
cargo run --manifest-path cli/Cargo.toml -- --json validate-prompt-output --dir plugin/assets/templates/recruiting --prompt-id normalize-recruiting-context --file <prompt-output.json>
cargo run --manifest-path cli/Cargo.toml -- --json verify-output --dir plugin/assets/templates/recruiting --file plugin/assets/templates/recruiting/examples/proof-output/valid-binding.json
```

The 27-fixture eval corpus covers every route plus insufficient context, protected/proxy misuse, autonomous outcomes, invented credentials, unverified or restricted sources, prompt-output validation, opaque-identity privacy, expected-source coverage, reviewer handoff, gaps, and proof bindings. Proof verification applies to evidence-carrying review text; a written source or card ID is not proof until `verify-output` accepts it.

Real employment decisions require accountable human review outside MDP.
