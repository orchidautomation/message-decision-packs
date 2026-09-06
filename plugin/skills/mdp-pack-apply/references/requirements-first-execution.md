# Requirements-first execution

Use this reference for every Apply job before collecting evidence, preparing a
model call, or calling an artifact usable. The same loop applies across GTM,
support, recruiting, proposal, and future profiles; domain nouns never replace
the exact pack and job contract.

## Establish requirements authority

Run a fresh projection for the exact pack and canonical job:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

A presented requirements artifact may be reused only when all of these match
the fresh CLI projection:

- contract is exactly `mdp.requirements.v2` and the installed CLI advertises it;
- `pack.id`, `pack.version`, and `pack.sha256` match the selected pack;
- `job.id` is the exact selected canonical job;
- `requirements_sha256` matches the canonical CLI result; and
- the presented artifact or receipt is intact and belongs to this invocation or
  an explicitly supplied, freshly verified lineage.

Do not accept filenames, timestamps, prose summaries, or matching domain names
as proof. If every binding verifies, report `requirements_state:
verified-and-reused`. If the artifact is absent, incomplete, stale, tampered,
unverified, for another job, or on another contract version, discard it as
authority, compile now, and report `requirements_state: compiled-now` with the
categorical reason. Never silently call an unverified artifact reused.

## Collect only what is missing

Read `job_prerequisites`, decision-input contracts, model-step inputs, routed
context requirements, source bindings, and host-owned inputs from the compiled
projection. Build the smallest collection plan for unresolved blocking items;
do not recollect satisfied inputs or broaden into general enrichment.

Preserve every attempt as one of:

```text
found | not-found | stale | conflicting | rejected | failed
```

Keep source identity, time, and receipt/hash evidence when the contract
requires them. A failed or conflicting attempt is evidence about the attempt,
not evidence satisfying the requirement. Never erase an earlier attempt to
make the newest result appear clean. Recompile and reevaluate after accepted
evidence changes; do not patch a prior readiness result in place.

## Classify the constraint owner

Classify each applicable requirement before generation:

| Owner | Examples | Required proof |
| --- | --- | --- |
| Schema/provider | closed references, types, required fields, enum or pattern constraints | projected provider schema and local schema validation agree |
| CLI deterministic | forbidden literals, declared text surfaces, mechanical counts, routed claim/reference closure | the installed CLI exposes and executes the rule |
| Versioned model review | clarity, relevance, coherence, persuasive or substantive quality | exact review prompt/contract and its validated result |
| Human review | legal, compliance, commercial approval, judgment, submission authority | named responsible reviewer or explicit unresolved state |

Prompts may repeat deterministic requirements to improve the first pass, but
prompt text is orientation, not enforcement proof. If a mechanical rule is
declared but the installed CLI cannot expose and enforce it, report
`declared-but-unenforced`, keep the exact rule and owner as a gap, and block
final readiness. Do not replace semantic judgment with brittle deterministic
heuristics; route semantic quality to the versioned model review and human
owner declared by the job.

## Final usability gate

Generation, integrity verification, deterministic policy approval, model
review, and human review are separate facts. A convenience workflow may
compose them, but must not collapse them into one inferred success.

Call a generated artifact usable only when:

1. the exact run directory, bundle, receipt, and artifact root pass
   `mdp --json verify-run`;
2. the verified run bundle's ordered `post_generation_validator_ids` exactly
   match the receipt's ordered validator evidence;
3. `final_validation` verifies and `artifact_state` is `valid` when validators
   are declared; and
4. every required model or human review is separately satisfied.

Missing, duplicated, reordered, failed, or hash-mismatched validator evidence
is not usable. Historical receipts without final-validator declarations remain
historical authority only; never upgrade them by inference.

## Bounded handoff

Return categorical evidence, not private bodies:

```text
requirements_state: verified-and-reused | compiled-now
requirements_contract: mdp.requirements.v2
pack: <id>@<version> <sha256>
job: <exact job id>
requirements_sha256: <sha256>
missing_prerequisites: <stable ids or none>
attempt_states: <stable prerequisite/source id + categorical state>
enforcement: <rule id + owner + enforced | declared-but-unenforced>
run_verification: verified | failed | not-run
final_validators: <ordered ids + passed/failed/missing>
artifact_usability: usable | blocked | unassessed
human_review: satisfied | required | not-applicable
next_action: <one smallest permitted action or stop>
```

Do not include source bodies, generated prose, provider responses, secrets, or
private scratch paths in this handoff.
