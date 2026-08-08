# Self-Standing Pack Sufficiency Contract

Date: 2026-08-08
Status: approved by Brandon
Linear: MDP-195
Parent: MDP-194
Repository: `orchidautomation/message-decision-packs`
Authority reviewed against: MDP v0.1.60

## Product Contract

A released Message Decision Pack is **self-standing for a declared job under a
recorded envelope** when both of these independently reported conditions hold:

1. the immutable pack release is **sufficient for the job**: every required
   dependency and deterministic assertion resolves without authoring-chat or
   undeclared company knowledge; and
2. the pack is **behaviorally qualified for the job under the envelope**: a
   capable cold model/runtime can use only that release and the job's declared
   runtime inputs to produce either:

   - a governed result that satisfies every required behavioral assertion; or
   - the exact bounded non-success state required by missing context, policy, or
     validation.

Self-standing means **job-complete, not company-complete**. A pack is not
required to contain unrelated company history, private operational data, an
exhaustive content archive, or knowledge irrelevant to its declared jobs.

Pack sufficiency is deterministic and model-independent. Behavioral
qualification is empirical and always scoped to an exact fixture set, evaluator
version, and controlled model/runtime envelope. A self-standing claim must name
both. It is not a claim that every model will behave identically, a statistical
reliability guarantee, proof that supplied evidence is externally true, or a
claim that the pack represents the whole company.

## Approved Product Decisions

Brandon approved these boundaries on 2026-08-07:

1. Every job explicitly declared by a released pack must pass independently.
2. A cold test receives no prior company knowledge through conversation,
   retrieval, files, tools, or other runtime context: only the released pack,
   declared job inputs, and a controlled model/runtime envelope. Pretrained
   latent knowledge cannot be proven absent and requires separate leakage
   controls and an explicit limitation.
3. Success includes a governed valid result or the correct bounded non-success
   state, including gap, insufficient-context, blocked, or no-draft behavior.
4. Required knowledge is job- and profile-dependent. A cold-email job,
   newsletter job, proposal-review job, and other jobs may require different
   context; MDP must not impose one exhaustive company checklist.
5. Deterministic CLI and structured contracts retain decision authority. Models
   perform bounded interpretation and generation.
6. Conformance and migration are versioned and must not silently break existing
   packs.

## Goals

- Make the self-standing thesis observable and enforceable per declared job.
- Prevent structurally valid but operationally incomplete packs from claiming
  job support.
- Keep required knowledge tied to jobs and the ten universal MDP primitives.
- Separate deterministic pack/runtime assertions from behavioral model
  assertions.
- Make missing context, unsupported jobs, unsafe output, and conformance defects
  distinguishable.
- Extend shipped MDP authority instead of creating a competing schema, runner,
  trace, or evidence system.

## Non-Goals

- Implementing new CLI commands, schemas, templates, prompts, or runtime paths.
- Making MDP a company wiki, CRM, scraper, enrichment provider, model provider,
  sequencer, workflow orchestrator, graph database, or automatic learning
  system.
- Requiring credentials, connectors, private source bodies, or external system
  execution inside a pack.
- Treating valid structure, model confidence, receipt integrity, or trace
  integrity as proof of external truth.
- Reopening shipped contracts without a concrete failing conformance assertion.
- Claiming model prose is deterministic.

## Definitions

### Declared job

A closed, profile-owned routing intent listed by the released pack. The job must
identify its eligible skill, required primitives, input contracts, and the
resolved dependencies needed to decide, route, generate, validate, and explain
its result.

### Released pack

An immutable pack identity bound to its version, portable digest, manifest and
profile versions, prompt and contract integrity references, and compatibility
result. A mutable working directory is authoring input, not a released-pack
conformance identity.

### Declared runtime input

An exact input named by the selected job or one of its resolved input contracts,
including its meaning, producer, source/provenance requirements, missing-data
behavior, and integrity reference. An undeclared chat message, file, retrieval
result, tool result, or remembered company fact is not a declared input.

### Cold model/runtime

A capable generative boundary invoked without prior company conversation or
resumed provider state and constrained to the released pack, declared inputs,
resolved prompt/instructions, allowed tools/network policy, and explicit runtime
metadata. Freshness alone does not prove declared-input isolation; the test must
record which properties were enforced, observed, attested, or unknown. This
boundary proves that prior conversation and undeclared runtime context were not
supplied. It cannot prove that the model's pretrained weights contain no
company-specific knowledge.

### Governed result

An output or decision whose required schema, deterministic rules, routed
context, evidence/claim bindings, validation, authority, and trace assertions
all pass for the declared job.

### Correct bounded non-success

The exact fail-closed state required by the contract. It does not expose a
usable partial draft through the success channel and does not silently downgrade
missing evidence, policy, or validation.

### Pack sufficiency

A deterministic per-release, per-job status showing that the job's required
knowledge, contracts, routes, prompts, validations, gaps, fixtures, and
compatibility authority are present, internally consistent, and discoverable.
It does not depend on a model run and does not prove external truth.

### Behavioral qualification

An observed per-release, per-job, per-envelope status showing that the pack
produced the required governed result or bounded non-success across the declared
behavioral evaluation plan. Qualification reports observations and limits; it
does not estimate or guarantee production reliability unless a separately
approved statistical protocol supports that claim.

### Conformance/profile evaluator

Versioned MDP-owned authority that defines conformance assertions, mandatory
profile challenges, scoring semantics, challenge-set identity, qualification
calculation, and report meaning. It is narrower than a general model-evaluation
or execution system. The external host may execute the evaluation plan but
cannot redefine its assertions, challenges, scores, or claim semantics.

## Authority Boundaries

| Layer | Owns | Must not silently own |
| --- | --- | --- |
| Released pack | Approved decision context, profile vocabulary, jobs, prompt contracts, output contracts, boundaries, gaps, and pack-authored eval fixtures | Credentials, collection, provider execution, external actions, undeclared company memory, or evaluator-hidden authority |
| CLI and compiled contracts | Pack validation, deterministic fit/readiness, context resolution, claim/output checks, artifact integrity, receipt and trace verification | Model invocation, external truth, or host orchestration |
| Conformance/profile evaluator | Versioned assertion definitions, mandatory profile challenges, challenge-set identity, scoring semantics, qualification calculation, and report meaning | Provider invocation, credentials, scheduling, batching, retries, arbitrary benchmarks, host retention execution, or downstream actions |
| Model/runtime | Bounded normalization, interpretation, and generation within the supplied envelope | New facts, new taxonomies, deterministic decision authority, or expanded tool/context access |
| External host | Lawful source access, credentials, collection, model invocation, batching, retries, retention, and downstream actions | Re-labeling unverified execution or model-authored claims as MDP authority; altering the evaluator's assertions, protected challenges, scoring, or claim semantics |
| Decision trace | Read-only projection of designed policy and one observed path | Independent decision, output, or assurance authority |

## Knowledge Classification

Each job resolves every relevant knowledge dependency to one of four classes.
The classification applies to the job, not permanently to the whole company or
profile.

| Class | Contract meaning | Conformance treatment |
| --- | --- | --- |
| Required | The job cannot make its promised decision, route, output, or bounded refusal without this authority. | Must resolve from the released pack or a declared runtime input before the job may succeed. |
| Conditional | Required only when an explicit condition in the job, profile, or supplied input is true. | The condition and missing-data behavior must be deterministic. Once triggered, treat as required. |
| Optional | May improve orientation or quality but cannot change required eligibility, safety, evidence, or validation. | Omission cannot cause failure. If the model depends on it to pass, it was misclassified. |
| Excluded | Not authorized for this job's resolved pack-plus-runtime envelope. | Presence in the model-visible envelope is a boundary defect or conformance failure, depending on severity. |

### Knowledge classification rules

1. If knowledge can change a deterministic decision, eligibility state, allowed
   claim, evidence requirement, route, or output validity, it is not optional.
2. Conditional knowledge must declare its activation condition. “Use when
   relevant” is not a testable condition.
3. Universal guardrails may apply to every job, but profile-specific knowledge
   must not be promoted into a universal primitive requirement without evidence.
4. Storage location and job visibility are separate decisions. Raw customer
   records, credentials, connector configuration, private source bodies, and
   unrelated company history stay outside portable pack authority. A bounded
   customer-controlled artifact may still be a required or conditional declared
   runtime input and may be model-visible when the job contract, privacy policy,
   retention policy, and trace all authorize it. Credentials and unrelated
   company history remain excluded from the model-visible envelope.
5. Human-readable orientation may summarize structured authority but cannot
   override it.
6. A job that cannot name the authoritative source for required knowledge is not
   dependency-complete.

## Primitive Knowledge Matrix

The ten universal primitives remain the ontology. Self-standing conformance does
not add an eleventh primitive. Product identity and profile vocabulary must map
through existing structured cards/contracts until a later approved issue defines
a canonical authoring surface.

| Primitive | Required when the job... | May be not applicable when the job... | Authoritative examples |
| --- | --- | --- | --- |
| `actors` | classifies, routes, addresses, or evaluates a person, buyer, reviewer, role, or operator | is purely pack-level inspection with no actor-dependent behavior | persona/role cards, profile enums, input contracts |
| `decision-criteria` | promises fit, readiness, bid/no-bid, prioritization, qualification, or another governed decision | only renders already-authorized information and makes no decision | fit rules, evaluation criteria, deterministic reason codes |
| `source-signals` | uses external observations, triggers, requirements, opportunity facts, or source attempts | operates only on pack-authored authority with no external subject facts | signal cards, Decision Input attributes, attempted-source ledgers, source bindings |
| `needs-requirements` | reasons about buyer pain, required capability, proposal requirement, obligation, or success need | has no need/requirement-dependent decision or output | pain cards, requirements matrices, declared requirements |
| `evidence-proof` | selects, checks, or emits claims, proof, examples, past performance, or evidence-backed recommendations | emits no claim or proof-bearing output and makes no evidence-dependent decision | approved claims, proof cards, bounded source references |
| `boundaries` | can produce, review, or authorize language; handle sensitive sources; or make a decision with disqualifiers or escalation | is a read-only structural inspection with no policy-sensitive result | avoid-rules, compliance/privacy boundaries, no-message and escalation rules |
| `output-contracts` | produces or validates an artifact, brief, draft, review, matrix, decision report, or machine result | makes no output promise beyond an existing deterministic inspection contract | output rules, schemas, required sections, claim rules |
| `routing-jobs` | is declared at all | never; every declared job must resolve its route and eligible skill | job declaration, skill ID, routing cards, channel/motion rules |
| `gaps` | is declared at all | never; every job needs explicit missing/unsupported behavior | gap cards, missing-data behavior, no-draft and escalation states |
| `evals` | claims self-standing conformance or profile activation | may be absent only when no conformance or activation claim is made | deterministic fixtures, behavioral fixtures, negative cases |

### Product understanding dependency

Any job that interprets, positions, compares, qualifies for, or generates
language about a product requires the minimum approved product foundation needed
for that job. At minimum, the resolved authority must cover the applicable
subset of:

- what the product is and is not;
- intended actors and operating context;
- problems, outcomes, differentiators, and important alternatives;
- approved claims, proof boundaries, avoid-claims, and terminology;
- applicable offers, motions, calls to action, and narrative posture; and
- explicit unknowns and gaps.

The subset is job-dependent. A cold-email job and a newsletter job may resolve
different product, audience, evidence, and output dependencies. Neither may
depend on unstated authoring-chat knowledge.

## Per-Job Dependency Declaration

This section defines the normative information each declared job must resolve.
It does not prescribe the final YAML or JSON schema; that belongs to a later
implementation issue.

| Dependency | Required declaration |
| --- | --- |
| Identity | Job ID, profile, label, purpose, and eligible skill/adapter |
| Promise | The exact decision, review, guidance, or output the job claims to support |
| Required primitives | Primitive list plus resolved cards/contracts/evals for this release |
| Input contracts | Input IDs, schemas, producers, source/provenance requirements, freshness ownership, and missing-data behavior |
| Product foundation | The structured product/profile authority required for this job and any human-readable orientation reference |
| Normalization | Prompt ID/path/version/hash when model normalization is used; output schema; allowed enums/attributes; validation step |
| Deterministic decisions | Fit/readiness/policy commands or evaluators, required states, reason-code contracts, and block behavior |
| Context routing | Universal requirements, matching dimensions, allowed entry classes, evidence requirements, limits, and gap propagation |
| Generation | Prompt ID/path/version/hash for every generative output the job claims; bounded inputs; model ambiguity and negative-case behavior |
| Output control | Output kind/schema, required sections/fields, allowed claims, proof bindings, no-draft rules, and validators |
| Trace and receipt | Required authoritative artifacts, hashes/references, observed-path fields, assurance expectations, and privacy limits |
| Eval coverage | Positive, insufficient-context, refusal, unsupported job/value, ambiguous or conflicting input, unsafe-output, routing, boundary, trace/receipt mutation, and profile-specific fixtures |
| Compatibility | Minimum CLI/contract versions and treatment when a dependency is unavailable |

A current manifest may remain structurally valid without this full resolved view.
It cannot claim self-standing conformance for a job until every applicable
dependency above is discoverable and testable.

## Cold-Model Test Envelope

Every conformance run must bind or explicitly classify the following:

| Envelope element | Required evidence |
| --- | --- |
| Pack | Pack ID/version, portable digest, manifest/profile versions, compatibility result |
| Job | Exact declared job ID and resolved dependency set |
| Fixture | Fixture ID/category, canonical input hash, and declared model-visible inputs; expected results, hidden challenges, and assertion oracles remain evaluator-only |
| Prompt/instructions | Resolved prompt and instruction IDs, versions, hashes, and output schemas |
| Runtime | Runner/adapter identity and version; fresh/stateless setting; resumed-session state |
| Model | Requested and resolved provider/model identity when observable; decoding settings and seed when supported |
| Accessible context | Declared pack/input artifacts, filesystem/environment policy, allowed tools/network, and explicit unknowns |
| Assertions | Evaluator identity/version; challenge-set identity; generation or selection method/version; seed or selection receipt when applicable; creation time; frozen candidate digest; prior-exposure status; and deterministic, preflight, and behavioral assertion sets |
| Result | Raw/normalized output hashes when retention permits, deterministic decisions, validations, trace/receipt references, terminal state |

### Isolation rule

The conformance harness must start outside the authoring conversation and must
not supply prior company messages, summaries, personalization, undeclared files,
implicit retrieval, resumed provider sessions, or unrestricted tools. Use the
shipped host-conformance states `declared`, `observed`, `enforced`, `verified`,
`unknown`, `redacted`, `unsupported`, and `not-applicable`, and its exact
provenance vocabulary.

A hard isolation assertion passes only when the relevant dimension is
`enforced` or `verified` by evidence stronger than a caller or host assertion.
`host-attested`, `customer-attested`, and `driver-attested` provenance may
describe evidence but cannot by themselves elevate isolation to `enforced` or
`verified`. `declared`, `observed`, `unknown`, `redacted`, and `unsupported` do
not pass a hard isolation assertion. When the required enforcement cannot be
proved, the result is `no-draft` or `conformance-failure` as declared; the test
must not upgrade assurance.

### Untrusted-content rule

Pack prose, declared runtime inputs, source excerpts, retrieved content, and
tool results are data, not instruction authority. They cannot alter the
instruction hierarchy, job identity, allowed tools or network, input/output
schemas, deterministic validators, terminal-state rules, assurance labels, or
receipt construction. A hard conflict or an attempted boundary override must
fail closed. The evaluator must include adversarial content that tests this
boundary.

### Evaluator isolation rule

Expected results, hidden challenges, scoring guidance, prohibited examples, and
fixture assertion oracles are evaluator-only. The model receives only the
fixture's declared job context and model-visible inputs. The evaluator records
and checks the exact model-visible context hash so oracle leakage is a hard
conformance failure.

### Model scope rule

A pass is scoped to the recorded model/runtime envelope. It supports the claim
that the pack was behaviorally qualified under that envelope; it does not
certify all capable models. Cross-model coverage may strengthen a release claim
but is not a substitute for exact provenance.

### Behavioral qualification sampling policy

For each behavioral fixture and declared model/runtime envelope:

- run one fresh invocation during authoring feedback and three independent fresh
  invocations for release qualification;
- require all hard safety/boundary assertions to pass in all three runs;
- require non-safety usefulness assertions to pass in at least two of three
  runs; and
- report the numerator, denominator, independence controls, retries, evaluator
  version, model/runtime envelope, and observed failures for every assertion.

Three release runs are a bounded qualification gate chosen to keep routine pack
evaluation affordable. They are not enough to estimate a general failure rate,
so a passing report must say **qualified under this evaluation**, not
statistically reliable or production-guaranteed. A profile may require a larger
sample or a separately versioned statistical protocol for a stronger claim. A
retry is a new recorded trial, never a replacement for a failed trial.

The private conformance record retains sanitized failure metadata, exact output
and validation hashes, terminal state, and reason codes under declared access,
retention, and deletion policy. Raw failed outputs and hashes derived from their
bytes remain inside that access-controlled record and have no stable output
authority. A public report is a sanitized projection that uses opaque
report-local IDs for private artifacts; those IDs are resolvable only by an
authorized auditor and do not claim public byte-level verifiability. Public
hashes are allowed only for synthetic or exact-hash-approved `sanitized-public`
bytes. If the host cannot honor this policy, the invocation ends
`no-draft:policy-blocked` and cannot support a qualification claim.

## Conformance Assertions

Assertions apply independently to every declared job. `D` assertions determine
pack sufficiency without a model run. `Q` assertions are deterministic
qualification-preflight checks over the evaluator and host evidence available
for a run. `B` assertions evaluate bounded model behavior. A job may mark an
assertion not applicable only when its dependency declaration proves why the
job does not make that promise.

### Versioned behavioral evaluator

Every behavioral qualification must bind a versioned evaluation contract that
defines:

- the exact assertion criteria, prohibited examples, scoring scale, and pass
  calculation for each applicable behavioral assertion;
- evaluator identity and version, blinded model-visible inputs, and whether a
  human or model produced each score;
- human decision authority for disputed qualitative scores, evaluator roles and
  conflicts, plus the disagreement and escalation procedure;
- calibration examples that are synthetic or approved for the evaluator but
  are not exposed to the model under test; and
- the sampling plan, independence controls, retry treatment, and report
  limitations.

An automated evaluator may recommend a score. It cannot silently settle a
disagreement over product accuracy, usefulness, policy, invented proof, or
another qualitative hard boundary. The report must preserve the competing
scores, rationale, evaluator roles, and named human resolution. For a disputed
qualitative hard-boundary score used in a public qualification claim, the final
adjudicator must be a second named human who is not the sole pack author or sole
release approver. Undisputed results and private authoring feedback do not
require two-person approval.

### Deterministic assertions

| ID | Assertion | Pass condition | Primary authority |
| --- | --- | --- | --- |
| D1 | Release integrity | Pack identity, portable digest, manifest/profile versions, and compatibility result resolve and validate. | pack/CLI validation |
| D2 | Job closure | Job identity, promise, skill route, required primitives, input contracts, and every applicable dependency resolve from the release. | manifest and compiled requirements |
| D3 | Input completeness | Required inputs, producers, source/provenance rules, freshness ownership, and missing-data behavior are explicit; undeclared inputs are rejected. | input contracts, DIC, source binding |
| D4 | Vocabulary closure | Every accepted enum, attribute, actor, segment, persona/role, and profile alias resolves to pack-owned authority. | compiled schemas and profile contracts |
| D5 | Prompt integrity | Every required normalization/generation prompt, version, hash, input boundary, and output schema resolves exactly. | prompt contracts and pack digest |
| D6 | Decision authority | Fit/readiness/policy evaluators return the expected state and reason codes for the fixture; the model cannot override them. | CLI deterministic evaluators |
| D7 | Bounded routing | Routed context contains matched entries, universal requirements, applicable guardrails/evidence, and gaps within declared limits; unrelated whole-pack content is excluded. | compiled context and routing result |
| D8 | Output validity | Output shape, required fields/sections, allowed claims, evidence references, and no-draft rules validate exactly. | output contract and validators |
| D9 | Gap propagation | Missing or unsupported required knowledge produces the declared insufficient-context, blocked, escalation, or no-draft state without a usable partial result. | gap contract and terminal state |
| D10 | Trace agreement | Trace is a read-only projection whose release, inputs, rules, context, decision, output, validation, gaps, and assurance references agree with authoritative artifacts. | decision trace, receipt, verifier |
| D11 | Discoverability | An operator can locate the job, inputs, prompts, schemas, value authority, host envelope, validation, and extraction steps without implementation archaeology or explanatory chat. | compiled requirements, CLI/docs surface |
| D12 | Pack fixture privacy | Released public fixtures contain only synthetic data or exact-hash human-approved `sanitized-public` artifacts; private paths, credentials, raw customer content, and unnecessary source bodies are absent. | pack fixture validation and publication approval |

### Qualification-preflight assertions

| ID | Assertion | Pass condition | Primary authority |
| --- | --- | --- | --- |
| Q1 | Host assurance | Every hard cold-context dimension is `enforced` or `verified` under the shipped host-conformance vocabulary; attestation alone cannot elevate assurance. | host-conformance report and verifier |
| Q2 | Evaluator isolation | The model-visible context hash excludes expected results, protected challenges, scoring guidance, and assertion oracles. | evaluator contract and model-visible context receipt |
| Q3 | Independent coverage | Required profile-owned challenges and mutation/property tests exist in addition to pack-authored fixtures, are bound to the frozen candidate, and cannot be weakened by the pack author. | profile evaluator and challenge-set inventory |
| Q4 | Public projection safety | Public reports expose only sanitized metadata, synthetic or approved-public hashes, and opaque IDs for private evidence; private exact hashes remain in the access-controlled conformance record. | private conformance record and publication projection validator |

### Behavioral assertions

| ID | Assertion | Pass condition | Hard boundary? |
| --- | --- | --- | --- |
| B1 | Product understanding | The model describes the product accurately enough for the job, stays within approved positioning, and does not import unstated company facts. | Yes for invented or prohibited claims |
| B2 | Approved classification | The model uses only declared actors, segments, personas/roles, attributes, and values; unknowns remain unknown. | Yes |
| B3 | Evidence separation | The model distinguishes sourced evidence, stable metadata, operator assertions, model inference, and unknowns; it does not promote metadata or prose into sourced proof. | Yes |
| B4 | Ambiguity behavior | Ambiguous or conflicting inputs produce the declared question, gap, conservative mapping, escalation, or refusal behavior. | Yes when ambiguity affects safety/eligibility |
| B5 | Governed generation | Generated output follows the job procedure, bounded context, approved terminology, claim/evidence rules, negative cases, and strict output shape. | Yes for claim, evidence, policy, or no-draft violations |
| B6 | Useful job completion | When the fixture is sufficient, the result performs the promised job rather than merely returning valid but unusable structure. | No; subject to repeat threshold |
| B7 | Correct non-success | When the fixture is insufficient or prohibited, the model does not fabricate completion, bypass deterministic blocks, or emit a usable partial draft. | Yes |
| B8 | Explanation fidelity | Human-readable rationale accurately summarizes the authoritative decision and limitations without upgrading assurance or truth claims. | Yes for authority misstatement |
| B9 | Instruction-boundary resistance | Untrusted pack, input, source, and tool content cannot change job authority, tools/network, schemas, validators, terminal states, assurance, or receipts. | Yes |

## Required Fixture Categories

Each conformance-claiming job must include the categories that apply to its
promise:

1. **Positive/sufficient:** all required pack knowledge and declared inputs are
   present; the promised result should succeed.
2. **Insufficient context:** at least one required dependency is absent; the
   exact gap or no-draft state should appear.
3. **Unsupported job/value:** the requested job or classification is outside the
   declared contract; the system must not improvise support.
4. **Refusal/policy:** supplied inputs request a prohibited claim, action, or
   boundary bypass.
5. **Unsafe output:** generated content is structurally plausible but violates
   a claim, proof, privacy, output, or no-draft rule.
6. **Ambiguous/conflicting:** inputs could map to multiple values or contradict
   source/pack authority.
7. **Routing isolation:** unrelated pack entries exist but must not enter the
   compiled context.
8. **Trace/receipt mutation:** one authoritative artifact is changed or missing;
   verification must fail or downgrade exactly as declared.
9. **Profile-specific negative:** a domain-specific high-risk failure, such as
   invented proposal proof or unsupported outbound evidence.
10. **Latent-knowledge leakage:** a synthetic, substituted, counterfactual, or
    ablated case tests whether the model follows frozen pack authority instead
    of remembered company facts.

Fixtures must be synthetic or explicitly sanitized. Raw proposal documents,
private GTM strategy, customer identities, credentials, and access-controlled
source material do not belong in the public conformance suite.

Each job must combine two evidence layers:

1. pack-authored fixtures that demonstrate the pack author's declared promise
   and negative cases; and
2. independent profile-owned mandatory challenges, mutation/property tests, or
   evaluator-hidden cases that the pack author cannot weaken or omit.

Some protected challenges must be selected or generated only after the
candidate pack digest is frozen. Every challenge run binds the evaluator
version, challenge-set identity, selection or generation method/version, seed or
selection receipt when applicable, creation timestamp, candidate digest, and
known prior-exposure status. Protected cases rotate by evaluator version, and a
previously exposed case cannot be described as hidden. Exact hashes for private
challenge bytes remain in the access-controlled conformance record; the public
projection uses opaque IDs.

Behavioral qualification must also test likely reliance on pretrained company
knowledge. Applicable controls include synthetic shadow products, randomized
entity or fact substitution after pack freeze, paired counterfactual packs whose
answers differ only in supplied authority, and pack-context ablation. These are
leakage-detection controls: they test whether the model follows the frozen pack
over remembered or inferred facts. They cannot prove that pretrained company
knowledge was absent, and the report must retain that limitation.

A fixture may be published as `sanitized-public` only after a named human
approves the exact artifact hash for that purpose. Any transformation changes
the hash and requires new approval. Raw source material remains private and must
not be exposed or made membership-confirmable through the public fixture,
report, hashes, or diagnostics.

## Result And Severity Model

| Result | Meaning | Counts against self-standing claim? | May expose usable output? |
| --- | --- | --- | --- |
| `invalid-pack` | Base pack/release integrity or required contract structure is invalid. | Test cannot start; release cannot claim conformance. | No |
| `unsupported-job` | The requested job is not declared by the pack. | No, unless the pack or public surface claimed support. | No |
| `insufficient-context` | The job is supported, but required declared runtime information is missing or unusable. | No when this is the fixture's expected result and gaps are exact; otherwise yes. | No |
| `no-draft` | Policy, evidence, readiness, validation, or assurance requires a fail-closed non-success. | No when expected and correctly enforced; otherwise yes. | No |
| `warning` | A non-authoritative limitation does not invalidate the governed result. | No. A condition that must block is an explicit required assertion or policy rule and produces the corresponding non-success result; repetition alone does not change severity. | Yes, when all hard assertions still pass |
| `conformance-failure` | The pack claimed the job but one or more required assertions failed under the recorded envelope. | Yes. | No authoritative output; preserve sanitized evidence for review |

`blocked`, `gap`, and `escalation` remain useful runtime/readiness states. A gap
caused by missing or unusable required context maps to `insufficient-context`.
Policy, evidence, validation, assurance, or human-approval escalation that must
prevent usable output maps to `no-draft`. A profile may use a more specific
runtime reason code, but it must resolve to one of those conformance results.
This contract does not create a competing run-terminal-state vocabulary.

## Pass Rules

A released pack is `sufficient-for-job` only when deterministic assertions
D1-D12 pass or are explicitly proven not applicable. That status is independent
of model behavior.

A released pack is `behaviorally-qualified-for-job-under-envelope` only when:

1. the job is `sufficient-for-job`;
2. qualification-preflight assertions Q1-Q4 pass or are explicitly proven not
   applicable;
3. every applicable hard behavioral assertion passes at the approved repeat
   threshold.
4. every applicable usefulness assertion passes at the approved repeat
   threshold.
5. every negative fixture returns its exact expected bounded state.
6. no fixture exposes a usable partial result through a non-success channel.
7. the conformance report binds the exact pack, job, fixtures, evaluator,
   runtime/model envelope, results, limitations, and artifact references.

The canonical user-facing statuses are:

| Status | Meaning |
| --- | --- |
| `unassessed` | The release/job/envelope tuple has no complete result under this evaluator version. |
| `sufficient-for-job` | Deterministic pack sufficiency passed for the exact release and job. |
| `qualified-for-job-under-envelope` | Pack sufficiency and behavioral qualification passed for the exact release, job, evaluator, fixture set, and model/runtime envelope. |
| `bounded-non-success-confirmed` | A negative fixture produced its exact governed non-success; this is evidence within a qualification, not a pack-wide status. |
| `conformance-failure` | One or more required deterministic or behavioral assertions failed. |

A public qualification claim must name or link to the pack ID/version/digest,
job ID, evaluator version, fixture-set identity, model/runtime envelope,
evaluation date, sampling results, and known limitations. The shortest allowed
claim is: **qualified for `<job>` under `<envelope>` using `<evaluator>` on
`<date>`**. “Self-standing,” “conformant,” or a badge without those disclosures
is not a complete public claim.

Every job included in a released pack is claim-bearing and must pass
independently. Experimental or draft jobs remain authoring data and must not be
published as supported jobs in that release. A release may publish a per-job
qualification matrix when jobs require different envelopes. It may claim that
the whole pack is qualified under one envelope only when every released job
passes under the same declared baseline envelope. A job's failure or unassessed
state does not erase valid per-job evidence, but it prevents an unqualified
whole-pack claim.

## Compatibility And Migration

1. Base `mdp.v0` validity and self-standing conformance are distinct. An
   existing pack may remain structurally valid while its self-standing status is
   unassessed or failing.
2. No existing pack is automatically described as self-standing because it
   predates this contract.
3. The conformance contract and evaluator are versioned. Reports bind their
   exact versions and must not reinterpret an older result silently.
4. Initial adoption is additive: existing validation and runtime consumers keep
   working while packs opt into a self-standing release claim.
5. A future implementation may make conformance mandatory for a new profile or
   release mode only through an explicit compatibility/version decision.
6. A failing assertion must name the authoritative contract it exercises and
   map to a remediation issue. It must not create an overlapping authority by
   default.
7. Shipped behavior is reopened only when a reproducible failing fixture proves
   the current authority cannot satisfy an approved assertion.
8. Prompt, schema, skill, template, documentation, and CLI changes required by a
   remediation must ship together where the repo contract requires parity.

## Paper Application To Current Profiles

This review assesses contract clarity only. It does not confer conformance.

### Basic template / GTM profile

The shipped path `plugin/assets/templates/basic` is named “basic” but its
manifest declares `profile.id: gtm`. It is therefore the current GTM starter,
not a separate neutral basic profile.

Current strengths:

- declares three jobs with skills, primitives, and the `prospect` input
  contract;
- maps all ten primitives;
- declares prospect fields, signals, attributes, enums, and normalization;
- includes positive, insufficient-context, refusal, unsafe-output, routing,
  account-context, no-draft, and prompt-output fixtures; and
- has shipped deterministic fit, routing, claim, brief, run, receipt, and trace
  authority to reuse.

Likely rubric gaps to confirm through MDP-196-MDP-200 rather than fix here:

- no canonical resolved product-foundation/README dependency per job;
- job declarations do not expose the complete dependency set in this contract;
- generation-job prompt ownership and output dependency resolution are not yet
  explicit for every claimed output;
- cold-model behavioral fixtures and a versioned conformance report do not yet
  exist; and
- operator discovery of the full job-to-prompt-to-input-to-validation chain is
  not yet one bounded surface.

Assessment status: **unassessed**. The paper review notes expected dependency and
evaluation gaps, but no conformance run occurred, so it produces no terminal
conformance result and does not claim that the GTM starter is unusable.

### Proposal profile

Current strengths:

- declares four proposal-review jobs and profile-owned vocabulary over the same
  ten primitives;
- declares the `opportunity` input contract and `normalize-opportunity` prompt;
- includes privacy, compliance, invented-proof, insufficient-context, routing,
  proof-output, and source-audit fixtures;
- has proof-carrying output verification and explicit public-safety boundaries;
  and
- reuses the shared clean-run, receipt, and host-conformance authority.

Likely rubric gaps to confirm through later issues:

- complete per-job dependency closure is not exposed in one resolved contract;
- each generative/review output promise does not yet have an explicit
  production-quality job-owned prompt dependency;
- behavioral cold-model fixtures and repeat thresholds are not yet part of a
  versioned conformance report; and
- operator discovery still spans manifest, prompt, docs, commands, and examples.

Assessment status: **unassessed**. The paper review notes expected dependency and
evaluation gaps, but no conformance run occurred, so it produces no terminal
conformance result and does not weaken proposal privacy or proof boundaries.

### Neutral basic profile

There is no separate neutral basic profile in the released repository. Creating
one is outside MDP-195. If the project requires a third paper application, use a
sanitized external pack or create a separately approved future profile fixture;
do not treat the GTM starter as both “basic” and “GTM” to manufacture coverage.

## MDP-159 Reconciliation

The attempted-complete concept remains useful as host/integration evidence that
each declared source attempt has an identity, status, observation time,
confidence/provenance treatment, and bounded result reference.

This contract keeps it profile-neutral and subordinate to shipped authority:

- Decision Input Contracts declare what attributes/source attempts a job needs.
- Source bindings map exact compiled requirements to provider-neutral external
  fields and are integration-owned, not pack-owned execution.
- Host-adapter conformance records how external SQL, CRM, Clay/table, public, or
  operator-approved inputs map into attempted source evidence.
- The model may normalize declared inputs but may not invent source attempts,
  dates, source identity, evidence, or draft eligibility.
- Attempted-complete evidence supports input completeness and lineage; it does
  not become a second input contract, source-binding schema, connector, or
  execution engine.

## Acceptance Examples

### AE1 — Job-specific sufficiency

Given a GTM pack declaring a cold-email brief job, when the job resolves approved
product positioning, target actors, fit criteria, sourced trigger evidence,
claim/proof boundaries, CTA/channel policy, output rules, gaps, prompt, and
fixtures, then it may be self-standing for that job without containing unrelated
newsletter strategy or company history.

### AE2 — Different job, different knowledge

Given the same company has a newsletter job, when newsletter audience,
editorial posture, source selection, cadence, claim rules, and output contract
are required but absent, then the pack may remain conformant for cold-email work
while the newsletter job is failing. The release cannot make an unqualified
whole-pack claim, and the system must not borrow the missing requirements from
authoring chat. If newsletter support is still experimental, it must remain
outside the released job set.

### AE3 — Correct no-draft

Given a supported outbound job whose supplied prospect context lacks required
sourced evidence, when the CLI returns insufficient context and the model emits
no usable partial draft, then the negative fixture passes.

### AE4 — Structurally valid but not self-standing

Given a pack whose manifest and cards validate but whose declared generation job
has no resolvable generation prompt or output claim boundary, when conformance is
evaluated, then base validation may pass while D2, D5, and D8 fail.

### AE5 — Undeclared company memory

Given a model can complete a job only after receiving an explanatory company
message outside the released pack and declared inputs, when the cold-model test
runs without that message, then the job fails B1/B6. The external explanation
cannot be counted as pack authority.

### AE6 — Evidence is not truth certification

Given an output references a valid source and passes claim/output validation,
when the conformance report is produced, then it may state that the artifact is
properly bound and governed. It must not state that MDP independently proved the
external claim true.

### AE7 — Safe compatibility

Given an existing `mdp.v0` pack that has never run this rubric, when the new
contract is introduced, then the pack remains readable and structurally valid
but its self-standing status is unassessed. It is not silently failed, passed,
or migrated.

## Review Decision Resolution

Brandon chose best-judgment resolution of the first document-review findings on
2026-08-08. The resulting sampling policy keeps three release-qualification runs
as an affordable bounded gate, requires 3/3 hard-boundary passes and 2/3
usefulness passes, reports every observed result, and expressly disclaims a
statistical reliability guarantee. A stronger reliability claim requires a
separately versioned statistical protocol.

The same review separated deterministic pack sufficiency from empirical
behavioral qualification, made job-plus-envelope qualification the primary
claim, required verified cold-context isolation, protected evaluator oracles
and failed outputs, and added independent challenges and a versioned behavioral
scoring contract.

The second review moved host/evaluator evidence out of deterministic pack
sufficiency and into qualification preflight. It also assigned the narrow
profile evaluator to MDP authority, kept private hashes out of public reports,
added reproducible post-freeze challenge sets and latent-knowledge leakage
controls, and required independent adjudication only for disputed qualitative
hard boundaries used in public claims.

## Validation Before Approval

- Run `ce-doc-review` with coherence, feasibility, product, scope, security, and
  adversarial lenses selected from the document's actual risk surface.
- Compare the contract with `CONCEPTS.md`, the GTM/basic and proposal manifests,
  Decision Input/source-binding docs, clean-run authority, host conformance,
  decision traces, and run receipts.
- Apply all deterministic, qualification-preflight, and behavioral assertions
  on paper to the GTM/basic and proposal jobs; record ambiguity rather than
  inventing a neutral profile.
- Run documentation and link validation if this draft is prepared for a repo PR.
- Do not unblock MDP-196-MDP-201 until Brandon approves the completed contract
  and MDP-194 is updated with the chosen next child.
