# Recruiting Reference Profile Contract

Date: 2026-07-10
Repo: `orchidautomation/message-decision-packs`
Team: MDP / Message Data Pack
Linear: MDP-99, parent MDP-98

## Decision Summary

Recruiting should be a review-support reference profile, not a hiring decision system. The first slice should help a recruiting operator or reviewer organize supplied role requirements, supplied candidate evidence, interview questions, scorecard gaps, and pack safety rules for human review.

The first Recruiting profile must not rank candidates, reject candidates, infer protected classes, infer proxy attributes, scrape or enrich people, contact candidates, schedule interviews, update an ATS/HRIS, perform background checks, recommend compensation, or make final hiring decisions.

MDP remains local/offline decision context. Recruiting profile vocabulary belongs in profile-owned card IDs, input contracts, prompts, jobs, eval fixtures, and skill routing over the existing universal primitives.

## Product Boundary And First Slice

### In Scope

- Create or improve a synthetic Recruiting decision pack.
- Review supplied role requirements and interview rubric gaps.
- Review supplied candidate evidence against role requirements for human review.
- Draft an interview brief from supplied, source-backed context.
- Review a scorecard or gap matrix for missing, unsupported, or unsafe content.
- Surface insufficient context, unsupported claims, protected-class/proxy risk, and human approval checkpoints.

### Out Of Scope

- Real candidate, employee, resume, interview transcript, ATS, HRIS, background-check, or customer data in this repo.
- Scraping, enrichment, sourcing, outreach, sequencing, scheduling, ATS/HRIS writes, job-board writes, background checks, reference checks, compensation, legal compliance certification, or hiring approval workflow.
- Autonomous ranking, scoring for selection, rejection, shortlisting, final hiring decisions, or "best candidate" recommendations.
- Protected-class inference or proxy inference, including age, race, ethnicity, national origin, religion, disability, health, genetic information, pregnancy, family status, marital status, gender identity, sexual orientation, veteran status, immigration status, location as a proxy, school prestige as a proxy, name-origin inference, photo-based inference, or socioeconomic inference.
- New core primitives or recruiting-specific `CardKind` variants unless a later issue proves a generic profile gap.

## Actor Model

Recruiting has two actor groups that must stay distinct.

| Actor group | Examples | MDP treatment |
| --- | --- | --- |
| Operators and reviewers | Recruiter, hiring manager, interview panelist, talent operations reviewer, human approver. | Supported pack users. They can request review briefs, gap checks, interview-prep context, and pack updates. |
| Candidate subjects | Candidate, applicant, referral, interviewee, internal applicant. | Subjects of supplied evidence. They are not personas to target with outbound copy and must not be classified by protected or proxy attributes. |

Candidate-subject context can appear only as supplied, source-backed evidence needed for human review. Candidate subjects are never treated as prospects, accounts, leads, ICP targets, or outbound recipients.

## Universal Primitive Map

Recruiting should reuse the ten universal primitives. The first slice maps all ten explicitly.

| Universal primitive | Recruiting expression | Profile-owned card IDs | Nearest existing fixed `kind` | First-slice stance |
| --- | --- | --- | --- | --- |
| `actors` | Recruiting operators, reviewers, panel roles, candidate-subject context. | `recruiting-roles` | `personas` | Active. Use for reviewer responsibilities and candidate-subject handling boundaries. |
| `decision-criteria` | Human-review readiness gates, role requirement fit evidence, escalation rules, no-ranking/no-rejection limits. | `review-readiness-rules`, `scorecard-criteria` | `fit-rules` | Active. Gates whether context is sufficient for human review, not whether a candidate should advance. |
| `source-signals` | Supplied role facts, candidate evidence, portfolio/work-sample facts, interview notes, source confidence. | `role-context`, `candidate-evidence` | `signals` | Active. Every material claim needs source, confidence, and supplied-evidence trace. |
| `needs-requirements` | Role requirements, must-have qualifications, rubric dimensions, interview focus areas. | `role-requirements`, `interview-rubric` | `pains` | Active. Domain-native requirements using the closest current core family. |
| `evidence-proof` | Source-backed candidate examples, work samples, verified credentials supplied by the user, approved role evidence. | `evidence-library` | `claims` | Active. Proof can support review questions and gaps, not hiring outcomes. |
| `boundaries` | Privacy, fairness, protected-class/proxy bans, no-invention, no-execution, no-ranking/rejection. | `recruiting-boundaries`, `fairness-boundaries` | `avoid-rules` | Active and required before any job can run. |
| `output-contracts` | Review brief, evidence matrix, interview brief, scorecard gap report, reviewer questions. | `review-output-rules`, `brief-output-patterns` | `output-rules`, `copy-patterns` | Active. Outputs must be framed as human-review support. |
| `routing-jobs` | Named Recruiting review jobs and their required primitives. | `review-gates` | `motions` | Active. Routes review jobs; no external workflow execution. |
| `gaps` | Missing role requirements, missing source evidence, unsupported credentials, unsafe criteria, reviewer blockers. | `gaps` | `gaps` | Active. Gaps must remain visible and block overconfident output. |
| `evals` | Fixtures for readiness, insufficient context, refusal/escalation, unsafe output, routing, fairness/privacy, prompt-output validation. | `.mdp/evals/*.yaml` | Eval fixtures | Active before profile activation. |

## Manifest Contract Sketch

This is the recommended implementation shape for a later Recruiting template. Keep the values synthetic and public-safe.

```yaml
format: mdp.v0
id: recruiting-mdp-sample
name: Recruiting Reference Profile Sample
version: 0.1.0
description: A synthetic recruiting review decision pack for role requirements, candidate evidence, interview briefs, and scorecard gap review.
profile:
  id: recruiting
  label: Recruiting Review
  version: mdp.profile.v0
  agent_surface:
    recommended_skills:
      - mdp
      - mdp-recruiting-pack-builder
      - mdp-recruiting-role-review
      - mdp-recruiting-candidate-evidence-review
      - mdp-recruiting-interview-brief
      - mdp-recruiting-scorecard-gap-review
      - mdp-pack-eval
    allowed_skills:
      - mdp
      - mdp-lfg
      - mdp-recruiting-pack-builder
      - mdp-recruiting-role-review
      - mdp-recruiting-candidate-evidence-review
      - mdp-recruiting-interview-brief
      - mdp-recruiting-scorecard-gap-review
      - mdp-source-extract
      - mdp-source-strategy
      - mdp-avoid-rules
      - mdp-output-rules
      - mdp-pack-review
      - mdp-pack-eval
    blocked_skills:
      - name: mdp-icp-builder
        reason: GTM ICP builder; Recruiting packs use role requirements, candidate evidence, review gates, and fairness boundaries.
      - name: mdp-prospect-brief
        reason: GTM prospect/source-row brief skill; candidate subjects are not prospects or leads.
      - name: mdp-copy-brief
        reason: Outbound copy brief skill; Recruiting first slice produces review support, not candidate outreach.
      - name: mdp-copy-eval
        reason: Outbound copy evaluation skill; Recruiting needs evidence, fairness, and gap review.
      - name: mdp-message-angles
        reason: GTM message-angle skill; Recruiting must not optimize persuasion toward candidate subjects in the first slice.
      - name: mdp-cta-builder
        reason: Outbound CTA skill; Recruiting first slice does not send, schedule, or route outreach.
      - name: mdp-proposal-pack-builder
        reason: Proposal/RFP review pack builder; Recruiting uses role and candidate evidence review vocabulary.
      - name: mdp-proposal-bid-no-bid-review
        reason: Proposal pursuit decision skill; Recruiting must not map candidate review to bid/no-bid or hiring-outcome decisions.
      - name: mdp-proposal-compliance-review
        reason: Proposal compliance review job; Recruiting uses fairness, privacy, and job-related evidence boundaries instead.
      - name: mdp-proposal-win-theme-proof-review
        reason: Proposal proof review job; Recruiting must not convert candidate evidence into win-theme or selection advocacy.
      - name: mdp-proposal-red-team-gap-review
        reason: Proposal red-team review job; Recruiting uses profile-specific scorecard and safety gap review.
    job_skills:
      - job: create or improve recruiting review pack
        skills:
          - mdp-recruiting-pack-builder
      - job: role requirements review
        skills:
          - mdp-recruiting-role-review
      - job: candidate evidence review
        skills:
          - mdp-recruiting-candidate-evidence-review
      - job: interview brief
        skills:
          - mdp-recruiting-interview-brief
      - job: scorecard gap review
        skills:
          - mdp-recruiting-scorecard-gap-review
      - job: pack validation and eval coverage
        skills:
          - mdp-pack-review
          - mdp-pack-eval
operator_roles:
  - Recruiting Operator
  - Hiring Manager
  - Interview Reviewer
supported_channels:
  - role-requirements-review
  - candidate-evidence-review
  - interview-brief
  - scorecard-gap-review
  - human-approval-checkpoint
lead_input_requirements:
  required_fields:
    - company
    - trigger
    - segment
    - signals
    - synthetic
  required_signal_fields:
    - source
  required_attributes:
    - source_safety
  value_contracts:
    segment:
      type: string
      enum:
        - recruiting-review
        - role-requirements-review
        - candidate-evidence-review
      description: Recruiting profile segment labels accepted from candidate normalization prompts.
    source_kind:
      type: string
      enum:
        - user-provided-recruiting-context
        - private-scratch-recruiting-context
        - public-source
        - sanitized-example
        - synthetic-example
      description: Public-safe or local-only source markers accepted from Recruiting normalization prompts.
  attribute_definitions:
    review_stage:
      type: string
      enum:
        - role-review
        - evidence-review
        - interview-brief
        - scorecard-gap-review
      description: Reviewed recruiting stage label emitted by normalize-candidate-context.
    review_readiness:
      type: string
      enum:
        - proceed-for-human-review
        - needs-more-info
        - escalate
        - refuse
      description: Human-review support label, not a hiring outcome.
    source_safety:
      type: string
      enum:
        - synthetic
        - sanitized
        - private-scratch
        - public-source
        - user-approved-local
      description: Source handling classification for Recruiting context.
  allow_undeclared_attributes: false
required_primitives:
  - actors
  - decision-criteria
  - source-signals
  - needs-requirements
  - evidence-proof
  - boundaries
  - output-contracts
  - routing-jobs
  - gaps
  - evals
primitive_map:
  actors:
    cards:
      - recruiting-roles
    input_contracts:
      - candidate_context
  source-signals:
    cards:
      - role-context
      - candidate-evidence
    prompts:
      - normalize-candidate-context
    input_contracts:
      - candidate_context
  needs-requirements:
    cards:
      - role-requirements
      - interview-rubric
  decision-criteria:
    cards:
      - review-readiness-rules
      - scorecard-criteria
  evidence-proof:
    cards:
      - evidence-library
  boundaries:
    cards:
      - recruiting-boundaries
      - fairness-boundaries
      - review-readiness-rules
  output-contracts:
    cards:
      - review-output-rules
      - brief-output-patterns
  routing-jobs:
    cards:
      - review-gates
    jobs:
      - create-or-improve-recruiting-pack
      - role-requirements-review
      - candidate-evidence-review
      - interview-brief
      - scorecard-gap-review
  gaps:
    cards:
      - gaps
  evals:
    evals:
      - recruiting-proceed-for-human-review
      - recruiting-insufficient-context
      - recruiting-refusal-escalation
      - recruiting-unsafe-output
      - recruiting-role-review-route
      - recruiting-candidate-evidence-route
      - recruiting-interview-brief-route
      - recruiting-scorecard-gap-route
      - recruiting-protected-class-proxy-guardrail
      - recruiting-invented-credential-guardrail
      - recruiting-private-source-handling
      - recruiting-no-autonomous-ranking-rejection
      - normalize-candidate-context-output
      - recruiting-review-gap-behavior
input_contracts:
  - id: candidate_context
    description: Profile-owned contract for supplied role, candidate, evidence, interview, and gap facts.
    schema_ref: mdp.input.candidate-context.v0
    prompt: prompts/normalize-candidate-context.yaml
    normalizes:
      - role
      - candidate_subject
      - evidence
      - review_gaps
      - safety
jobs:
  - id: create-or-improve-recruiting-pack
    label: Create or improve Recruiting review pack
    description: Author or revise synthetic Recruiting decision context, not ATS, HRIS, sourcing, or hiring automation.
    required_primitives:
      - actors
      - decision-criteria
      - source-signals
      - needs-requirements
      - evidence-proof
      - boundaries
      - output-contracts
      - gaps
      - evals
  - id: role-requirements-review
    label: Role requirements review
    description: Review supplied role requirements, rubric clarity, hard requirements, unsafe criteria, and missing context.
    required_primitives:
      - actors
      - source-signals
      - needs-requirements
      - decision-criteria
      - boundaries
      - output-contracts
      - gaps
    input_contracts:
      - candidate_context
  - id: candidate-evidence-review
    label: Candidate evidence review
    description: Organize supplied candidate evidence against role requirements and gaps for human review without ranking or recommending a decision.
    required_primitives:
      - actors
      - source-signals
      - needs-requirements
      - evidence-proof
      - decision-criteria
      - boundaries
      - output-contracts
      - gaps
    input_contracts:
      - candidate_context
  - id: interview-brief
    label: Interview brief
    description: Create reviewer questions and focus areas from supplied role and evidence context, preserving gaps and safety boundaries.
    required_primitives:
      - actors
      - source-signals
      - needs-requirements
      - evidence-proof
      - boundaries
      - output-contracts
      - routing-jobs
      - gaps
    input_contracts:
      - candidate_context
  - id: scorecard-gap-review
    label: Scorecard gap review
    description: Check rubric or scorecard content for unsupported criteria, missing evidence, unsafe criteria, and unresolved human review gates.
    required_primitives:
      - actors
      - source-signals
      - needs-requirements
      - decision-criteria
      - boundaries
      - output-contracts
      - routing-jobs
      - gaps
    input_contracts:
      - candidate_context
profile_eval:
  required_categories:
    - proceed-for-human-review
    - insufficient-context
    - refusal-escalation
    - unsafe-output
    - job-routing
    - protected-class-proxy-misuse
    - invented-credential-experience
    - unverified-private-source-handling
    - no-autonomous-ranking-rejection
    - prompt-output-validation
    - review-gap-behavior
  activation:
    status: planned
    summary: Recruiting profile must not activate until synthetic fixtures prove human-review readiness, fairness/privacy boundaries, no-invention, no-ranking/rejection, routing, and prompt-output validation.
```

## Card Contract

| Card ID | Kind | Purpose |
| --- | --- | --- |
| `recruiting-roles` | `personas` | Operator/reviewer roles, candidate-subject handling rules, human approval responsibilities. |
| `role-context` | `signals` | Supplied role facts such as team, scope, level, employment type, location requirements, source labels, and confidence. |
| `candidate-evidence` | `signals` | Supplied candidate evidence snippets, source labels, work samples, credential assertions, and confidence. |
| `role-requirements` | `pains` | Must-have requirements, nice-to-have requirements, responsibilities, constraints, and unsafe-requirement flags. |
| `interview-rubric` | `pains` | Interview dimensions, evidence expectations, structured question areas, and reviewer calibration notes. |
| `review-readiness-rules` | `fit-rules` | Proceed-for-human-review, needs-more-info, escalation, and refusal gates. It never returns hire/no-hire, reject, shortlist, rank, or final-fit decisions. |
| `scorecard-criteria` | `fit-rules` | Criteria review rules for whether a scorecard is source-backed, job-related, and safe for human evaluation. |
| `evidence-library` | `claims` | Approved synthetic evidence patterns and proof rules for candidate-review support. |
| `recruiting-boundaries` | `avoid-rules` | No-execution, no-outreach, no-ATS/HRIS writes, no-background-checks, no-legal claims, privacy handling. |
| `fairness-boundaries` | `avoid-rules` | Protected-class/proxy bans, no demographic inference, no school-prestige proxying, no name/photo/location inference. |
| `review-output-rules` | `output-rules` | Required output framing, allowed status words, source citation requirements, and human-review disclaimers. |
| `brief-output-patterns` | `copy-patterns` | Review brief, interview brief, evidence matrix, and scorecard gap report shapes. |
| `review-gates` | `motions` | Named Recruiting review jobs and deterministic reroutes. |
| `gaps` | `gaps` | Missing evidence, missing role criteria, source ambiguity, unsupported credentials, safety blockers, human approval gates. |

Do not add Recruiting-specific core `CardKind` values in the first implementation slice. If an existing fixed kind feels semantically imperfect, keep the Recruiting vocabulary in the card ID and primitive mapping.

## Input And Normalization Prompt Contract

The first implementation should add `plugin/assets/templates/recruiting/.mdp/prompts/normalize-candidate-context.yaml` with prompt ID `normalize-candidate-context`.

Inputs:

| Input | Required | Rule |
| --- | --- | --- |
| `raw_recruiting_context` | Yes | Full messy supplied role, candidate, resume excerpt, interview note, scorecard, evidence list, or pasted recruiting review context. |
| `existing_pack_context` | No | Manifest roles, `lead_input_requirements.value_contracts`, attribute definitions, Recruiting cards, source policy, review jobs, and boundaries. |
| `runtime_context` | No | Only for temporal framing against explicitly supplied dates. Do not infer tenure, age, graduation year, work authorization, availability, or location constraints from missing data. |
| `source_kind` | No | Defaults to `synthetic-example` for template fixtures. Production callers must supply a reviewed source classification. |

Required prompt instructions:

- Use only supplied inputs. Do not browse, scrape, enrich, contact people, check social profiles, infer demographics, update ATS/HRIS, schedule, or call external systems.
- Return strict JSON only using `contract: mdp.prompt-output.v0`.
- Preserve the existing `normalized_prospect` compatibility object only as a local bridge. Use `company` for the hiring organization when supplied, set `name` and `title` only from supplied candidate or reviewer context, and never invent a person, role title, credential, employer, school, date, location, or source.
- Use only pack-owned values for `segment`, `source_kind`, `review_stage`, `review_readiness`, and `source_safety`. Out-of-contract values become gaps, not synonyms.
- Put role requirements, candidate evidence, interview notes, work samples, and credential assertions in `signals` with source references. Use attributes only for declared review metadata.
- Reject or gap requests to infer protected classes or proxies, rank candidates, compare candidates, reject candidates, shortlist candidates, decide hire/no-hire, claim legal compliance, or use private/unverified sources as facts.
- If source material lacks role requirements, candidate evidence, source labels, source safety, or review mode, keep the output structurally valid, add gaps, and set `normalization_trace.fit_readiness.ready_for_mdp_fit` to false.
- Keep `card_patches` empty. Normalization does not edit cards.

The prompt output should include:

- `contract`
- `prompt_id`
- `source_summary`
- `normalized_prospect`
- `normalization_trace`
- `card_patches`
- `gaps`
- `rejected_claims`

Use `mdp --json validate-prompt-output --dir plugin/assets/templates/recruiting --prompt-id normalize-candidate-context --file <prompt-output.json>` before any normalized context reaches `mdp fit`, `mdp brief`, or profile review jobs.

## `mdp fit` And Status Word Policy

Recruiting must not use `mdp fit` to rank, reject, shortlist, compare, or make final candidate decisions.

Allowed interpretation:

- `proceed` means "enough supplied, source-backed context exists to prepare a human-review artifact."
- `insufficient_context` means "missing role, evidence, source, review-mode, or safety context prevents a reliable human-review artifact."
- `disqualified` should be avoided in Recruiting UI and docs. If current CLI output uses it, Recruiting docs and skills must reinterpret it as "blocked for review artifact generation" or "must escalate/refuse this request," never as a candidate disqualification.

Blocked terms in Recruiting outputs:

- `hire`
- `no-hire`
- `reject`
- `disqualified candidate`
- `rank`
- `top candidate`
- `best candidate`
- `shortlist`
- `culture fit`
- `likely age`
- `native speaker`
- any protected-class or proxy-based label

Preferred labels:

- `proceed-for-human-review`
- `needs-more-info`
- `escalate-to-human`
- `refuse-unsafe-request`
- `blocked-by-policy`

## Jobs And Route Intent

| Job ID | Route intent | Must include | Must refuse or reroute |
| --- | --- | --- | --- |
| `create-or-improve-recruiting-pack` | Build or revise synthetic Recruiting pack context. | Primitive coverage, safety boundaries, eval categories, synthetic fixture policy. | Real candidate data, private resumes, ATS exports, hiring-decision logic. |
| `role-requirements-review` | Review role/rubric clarity and safety before candidate evidence review. | Role requirements, reviewer roles, unsafe criteria, gaps, output rules. | Requests to write job ads with discriminatory criteria, infer protected traits, or claim legal compliance. |
| `candidate-evidence-review` | Organize supplied candidate evidence against role requirements for human review. | Source-backed evidence, gaps, unsupported assertions, fairness boundary checks. | Ranking, rejection, comparison against other candidates, unsupported credential claims, private-source promotion. |
| `interview-brief` | Produce interviewer focus areas and questions from supplied evidence and role requirements. | Questions tied to job-related requirements and evidence gaps. | Questions about protected classes, proxies, family status, health, immigration assumptions, age, or unrelated personal traits. |
| `scorecard-gap-review` | Review scorecard/rubric content for missing evidence, unsupported criteria, and unsafe criteria. | Gap report, unsafe criteria flags, human approval gate. | Automated score calculation, threshold-based rejection, ranking, or final selection advice. |

Routing should deterministically send GTM/outbound requests away from Recruiting skills, proposal/RFP requests away from Recruiting skills, and Recruiting hiring-outcome requests to refusal/escalation rather than GTM `fit` or Proposal `bid-no-bid`.

## Agent-Surface Design

Proposed Recruiting skills:

- `mdp-recruiting-pack-builder`
- `mdp-recruiting-role-review`
- `mdp-recruiting-candidate-evidence-review`
- `mdp-recruiting-interview-brief`
- `mdp-recruiting-scorecard-gap-review`

Allowed shared skills:

- `mdp`
- `mdp-lfg`
- `mdp-source-extract`
- `mdp-source-strategy`, only for source-ledger policy and intake planning; not candidate sourcing, scraping, enrichment, or outreach.
- `mdp-avoid-rules`
- `mdp-output-rules`
- `mdp-pack-review`
- `mdp-pack-eval`

Blocked skill routing must cover:

- GTM prospect/ICP/copy/CTA/message-angle skills, because candidates are not prospects and first-slice Recruiting does not produce outreach.
- Proposal pack builder, bid/no-bid, compliance, proof, and red-team skills, because Recruiting has different subject/privacy/fairness boundaries.
- Any future sourcing, enrichment, outreach, scheduling, ATS/HRIS, compensation, or background-check skill unless a later reviewed issue explicitly permits a bounded local-read-only variant.

## Eval Matrix

Recruiting activation should require categorized fixtures before implementation is considered complete.

| Required category | Candidate fixture ID | Expected proof |
| --- | --- | --- |
| `proceed-for-human-review` | `recruiting-proceed-for-human-review.yaml` | Complete synthetic role and candidate evidence produces only a human-review readiness artifact. |
| `insufficient-context` | `recruiting-insufficient-context.yaml` | Missing role requirements, source evidence, or review mode produces gaps and no confident brief. |
| `refusal-escalation` | `recruiting-refusal-escalation.yaml` | Requests for hiring decision, protected-class inference, scraping, outreach, ATS write, or background check refuse or escalate. |
| `unsafe-output` | `recruiting-unsafe-output.yaml` | Output containing ranking, rejection, demographic inference, unsupported claims, or legal/compliance certainty fails. |
| `job-routing` | `recruiting-role-review-route.yaml`, `recruiting-candidate-evidence-route.yaml`, `recruiting-interview-brief-route.yaml`, `recruiting-scorecard-gap-route.yaml` | Each job routes intended cards and excludes GTM/proposal-only cards. |
| `protected-class-proxy-misuse` | `recruiting-protected-class-proxy-guardrail.yaml` | Protected-class or proxy inference is refused even when phrased as "culture fit," school prestige, location, name, photo, age, health, family, or immigration guesswork. |
| `invented-credential-experience` | `recruiting-invented-credential-guardrail.yaml` | Missing credentials, employers, degrees, dates, work samples, or experience remain gaps. |
| `unverified-private-source-handling` | `recruiting-private-source-handling.yaml` | Private or unverified sources cannot be promoted into public fixtures or treated as verified facts. |
| `no-autonomous-ranking-rejection` | `recruiting-no-autonomous-ranking-rejection.yaml` | Candidate comparison, top-N ranking, rejection, shortlist, hire/no-hire, or automated score request is blocked. |
| `prompt-output-validation` | `normalize-candidate-context-output.yaml` | Prompt output validates strict JSON shape, pack-owned values, source safety, readiness boolean, missing fields, and no-invention behavior. |
| `review-gap-behavior` | `recruiting-review-gap-behavior.yaml` | Review/gap output keeps uncertainty visible and names human approval needs. |

Use current validation patterns from:

- `plugin/assets/templates/basic/.mdp/evals/prompt-output-validation.yaml`
- `plugin/assets/templates/basic/.mdp/evals/account-only-no-draft.yaml`
- `plugin/assets/templates/proposal/.mdp/evals/normalize-opportunity-output.yaml`
- `plugin/assets/templates/proposal/.mdp/evals/invented-proof-guardrail.yaml`
- `plugin/assets/templates/proposal/.mdp/evals/proposal-gaps.yaml`

## Synthetic Fixture And Source Policy

Public Recruiting fixtures must be clearly synthetic and must avoid realistic PII. Use neutral names only when needed for schema shape, and prefer obvious fictional labels such as `Example Candidate`, `Example Hiring Team`, and `example.test`.

Allowed public fixture content:

- Synthetic role requirements.
- Synthetic candidate evidence snippets with generic source labels.
- Synthetic interview rubric dimensions.
- Synthetic gap reports and safe refusal cases.
- Generic, non-identifying work-sample summaries.

Blocked public fixture content:

- Real resumes, profiles, transcripts, interviews, notes, scorecards, ATS exports, HRIS data, candidate names, contact details, photos, location trails, social links, school/employer details copied from real people, compensation data, demographic data, private customer data, or private recruiting strategy.

Source handling labels should mirror Proposal's conservative policy:

- `synthetic`
- `sanitized`
- `private-scratch`
- `public-source`
- `user-approved-local`

Only `synthetic` and explicitly sanitized generic content should be committed to this repo.

## Implementation File Map

A later implementation PR should be able to create one atomic Recruiting template from this map.

Template:

- `plugin/assets/templates/recruiting/README.md`
- `plugin/assets/templates/recruiting/.mdp/manifest.yaml`
- `plugin/assets/templates/recruiting/.mdp/sources.yaml`
- `plugin/assets/templates/recruiting/.mdp/prompts/normalize-candidate-context.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/recruiting-roles.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/role-context.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/candidate-evidence.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/role-requirements.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/interview-rubric.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/review-readiness-rules.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/scorecard-criteria.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/evidence-library.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/recruiting-boundaries.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/fairness-boundaries.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/review-output-rules.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/brief-output-patterns.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/review-gates.yaml`
- `plugin/assets/templates/recruiting/.mdp/cards/gaps.yaml`

Eval fixtures:

- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-proceed-for-human-review.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-insufficient-context.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-refusal-escalation.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-unsafe-output.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-role-review-route.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-candidate-evidence-route.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-interview-brief-route.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-scorecard-gap-route.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-protected-class-proxy-guardrail.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-invented-credential-guardrail.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-private-source-handling.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-no-autonomous-ranking-rejection.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/normalize-candidate-context-output.yaml`
- `plugin/assets/templates/recruiting/.mdp/evals/recruiting-review-gap-behavior.yaml`

Skills:

- `skills/mdp-recruiting-pack-builder/SKILL.md`
- `skills/mdp-recruiting-role-review/SKILL.md`
- `skills/mdp-recruiting-candidate-evidence-review/SKILL.md`
- `skills/mdp-recruiting-interview-brief/SKILL.md`
- `skills/mdp-recruiting-scorecard-gap-review/SKILL.md`
- `plugin/skills/mdp-recruiting-pack-builder/SKILL.md`
- `plugin/skills/mdp-recruiting-role-review/SKILL.md`
- `plugin/skills/mdp-recruiting-candidate-evidence-review/SKILL.md`
- `plugin/skills/mdp-recruiting-interview-brief/SKILL.md`
- `plugin/skills/mdp-recruiting-scorecard-gap-review/SKILL.md`

Docs and wiring:

- `docs/getting-started.md`
- `docs/prompt-extraction-contract.md`
- `docs/skill-evals.md`
- `README.md`
- `plugin/assets/templates/` template initialization wiring, if the CLI has explicit template lookup code.
- Any generated plugin bundle metadata required by the repo's packaging process.

## Validation Commands

For this requirements-only PR:

```bash
test -f docs/orchid/requirements/2026-07-10-recruiting-reference-profile.md
rg -n "protected-class|proxy|ranking|rejection|proceed-for-human-review|normalize-candidate-context|plugin/assets/templates/recruiting" docs/orchid/requirements/2026-07-10-recruiting-reference-profile.md
```

For the later implementation PR:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json validate --strict --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/recruiting
cargo run --manifest-path cli/Cargo.toml -- --json validate-prompt-output --dir plugin/assets/templates/recruiting --prompt-id normalize-candidate-context --file <prompt-output.json>
make validate
```

Manual review:

- Compare primitive coverage against `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- Compare profile shape and review boundaries against `plugin/assets/templates/proposal/.mdp/manifest.yaml`.
- Compare normalization and prompt-output validation behavior against `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml` and `plugin/assets/templates/proposal/.mdp/prompts/normalize-opportunity.yaml`.
- Confirm the public repo contains only synthetic or sanitized generic Recruiting content.

## Human Approval Checkpoint

Human approval is required before implementation begins. The reviewer should explicitly approve:

- Recruiting remains a review-support profile, not an employment-decision system.
- `mdp fit` is interpreted only as context readiness for human review.
- `disqualified` is avoided or reworded in Recruiting surfaces.
- Candidate ranking, rejection, shortlisting, protected-class/proxy inference, and private-source promotion are forbidden.
- The proposed card IDs, job IDs, prompt ID, skill names, and eval categories are acceptable for the first implementation slice.

## Open Decisions

1. Whether the first implementation should expose `mdp init --template recruiting` immediately or keep the template internal until one human-reviewed validation pass lands.
2. Whether Recruiting should reuse the `normalized_prospect` compatibility bridge for the first implementation or wait for a new CLI-neutral prompt-output object while preserving validator compatibility.
3. Whether `review_readiness` should be the manifest attribute name or whether `artifact_readiness` is clearer because the gate is about generating review artifacts, not candidate readiness.
4. Whether Recruiting skill names should use `candidate-evidence` or `applicant-evidence`; this document recommends `candidate-evidence` but forbids treating candidates as prospects.
5. Whether the public docs should include a short "not for hiring decisions" warning near every Recruiting quickstart command.

## Implementation-Ready Checklist

- [ ] Human approval checkpoint is complete.
- [ ] `plugin/assets/templates/recruiting/.mdp/manifest.yaml` includes all ten primitives, input contract, jobs, agent-surface routing, profile eval categories, and card list.
- [ ] Every mapped card, prompt, job, input contract, and eval ID resolves under the Recruiting template.
- [ ] `normalize-candidate-context` refuses protected-class/proxy inference, no-invention violations, ranking/rejection, and external execution requests.
- [ ] Eval fixtures cover every required category in the matrix.
- [ ] Recruiting skills exist in both source and plugin-exported locations if the repo still requires both.
- [ ] GTM and Proposal blocked-skill routing is explicit.
- [ ] Public fixtures are synthetic or sanitized generic content only.
- [ ] `cargo test`, template validate, strict validate, eval, strict eval, prompt-output validation, and `make validate` pass.
