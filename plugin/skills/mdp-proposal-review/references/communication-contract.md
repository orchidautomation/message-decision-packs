# Skill Communication Contract

Use this five-part contract for user-facing updates. Keep it concise and
phase-oriented; never expose chain of thought, secrets, private artifact
bodies, or command-by-command narration.

1. **Orient** — Before substantive work, name the selected mode or job, the
   exact pack and approved evidence boundary, the useful artifact or decision
   expected, and the actions this skill will not take.
2. **Plan** — State the few meaningful phases and the next CLI-owned gate.
   Revise the plan only when a gate changes the route.
3. **Progress** — Announce only a meaningful gate transition, blocker, or
   decision. Do not narrate routine reads, commands, retries, or internal
   reasoning.
4. **Translate** — Preserve the CLI decision, then use these product terms:
   - **structurally valid**: deterministic structural checks passed; this says
     nothing by itself about job or input readiness;
   - **job-ready**: the selected job reports `pack_ready: true` and its other
     required gates pass;
   - **input-ready**: the selected job's required inputs and lineage have been
     accepted by their CLI-owned contracts;
   - **safe-to-draft**: the current authoritative decision explicitly permits
     drafting;
   - **no-draft**: drafting is not permitted. Any blocked, unavailable,
     invalid, unknown, missing-input, or human-review result remains no-draft.

   Never use a weaker term to imply a stronger one. In particular,
   structurally valid or job-ready does not imply input-ready or safe-to-draft.
5. **Close** — Report durable artifacts, current readiness, unresolved gaps,
   what state was retained or discarded, and the next allowed action. If no
   artifact or state was created, say so.

The opening and close may be a compact paragraph or bullets. The headings are
the operating contract, not required response headings.
