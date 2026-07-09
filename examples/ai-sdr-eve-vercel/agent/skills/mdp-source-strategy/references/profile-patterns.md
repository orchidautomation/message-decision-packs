# MDP Source Strategy Profile Patterns

Use these patterns to keep strategy artifacts profile-aware without turning MDP into a sourcing, scraping, enrichment, outreach, or proposal-execution system.

## Universal Primitive Lens

Every source strategy should explain what evidence is needed for:

- `actors`: people, organizations, accounts, customers, agencies, evaluators, reviewers, owners.
- `decision-criteria`: proceed, pause, refuse, escalate, bid/no-bid, qualify/disqualify, ask-for-more.
- `source-signals`: observed facts, provenance, freshness, confidence, snippets.
- `needs-requirements`: pains, requirements, evaluation factors, obligations, acceptance criteria.
- `evidence-proof`: approved claims, proof points, certifications, references, examples.
- `boundaries`: no-invention, privacy, access, unsupported claim, no-execution, channel limits.
- `output-contracts`: allowed review shape, brief shape, matrix shape, scout result shape.
- `routing-jobs`: source extraction, ICP review, proposal review, compliance review, red-team review.
- `gaps`: missing, weak, stale, blocked, or unapproved inputs.
- `evals`: proceed, insufficient-context, refusal, unsafe-output, and job-routing cases.

## GTM / BDR Scout Pattern

Use when ICP/account/persona context needs to become source targets and scout instructions before MDP extraction or outbound brief generation.

### Inputs To Normalize

- ICP or target segment.
- Account identity: name, domain, product line, geography, company size, industry, existing source notes.
- Persona or buying committee context: role/title family, job-to-be-done, likely pains, objections.
- Trigger window: recent hiring, launch, funding, migration, compliance event, leadership change, expansion, vendor switch.
- Negative fit: excluded industries, existing customers, competitors, geographies, tiny accounts, students, agencies, unsupported personas.
- Evidence threshold: public primary source, date requirement, confidence, citation shape.

### Source Targets

Prefer public primary sources and approved user data:

- account website, about, product, pricing, customer, case-study, docs, trust/security pages;
- press, newsroom, blog, release notes, changelog, public webinars/events;
- careers/jobs pages and public role descriptions;
- public funding, filing, procurement, app marketplace, review, or technology pages when relevant;
- user-approved CSV rows, notes, CRM exports, or research summaries treated as supplied source material.

Exclude personal contact enrichment, login-required social/profile scraping, email discovery, private communities, customer records without approval, and CRM/sequencer writes.

### Query / Prompt Building

Combine:

1. Account identifiers: legal name, common name, domain, product names.
2. ICP terms: segment, industry, size, use case, alternative.
3. Trigger terms: hiring, launch, expansion, migration, funding, compliance, incident, integration, partnership.
4. Persona terms: title family, department, budget owner, user role.
5. Negative filters: excluded geos/segments, jobs spam, unrelated subsidiaries, contact databases, login-required pages.

For Exa/Firecrawl/Apify-style scouts, write bounded jobs:

```yaml
scout_family: firecrawl
target_ids: [account-site, account-blog]
max_scope: "public pages on {{domain}}, depth<=2, max 25 pages"
query_or_prompt: "Collect source-backed evidence of current expansion, migration, hiring, compliance, or product-launch triggers relevant to {{persona}}. Return URL, snippet, observed date, trigger type, confidence, and why it matters. Do not collect personal emails or gated pages."
```

### Route Hints And Eval Checks

- Route reviewed trigger evidence to `$mdp-source-extract`.
- Route weak ICP/account/persona boundaries to `$mdp-icp-builder`.
- If only account context exists and no person/persona readiness exists, mark a no-draft or gap route instead of inventing a contact.
- Eval checks should fail when scout output has no receipt, uses contact databases, ignores negative fit, routes account-only context to draft-ready copy, or treats stale/uncited claims as proof.

## Proposal Source-Intake Pattern

Use when an opportunity, RFP, capture summary, or approved corpus needs a source-intake plan before proposal pack building or review jobs.

### Inputs To Normalize

- Opportunity/RFP name, customer/agency, buyer/evaluator, due date, vehicle, owner, review mode.
- Approved source corpus: RFP, amendments, Q&A, SOW/PWS, instructions, evaluation criteria, compliance clauses, capture notes, proof library, past performance, policies.
- Approval status: user-provided-approved, approved-corpus, sanitized-example, synthetic-example, needs-approval, excluded.
- Required review jobs: bid/no-bid, compliance, win-theme proof, red-team gap review, executive brief.
- Confidentiality, regulated data, customer-identifying, pricing, security, and public-repo boundaries.

### Source Targets

Default to local/approved sources. Do not imply autonomous web scraping is default for proposal work.

Use:

- supplied RFP/opportunity files and approved exports;
- amendments, Q&A, attachments, submission instructions, evaluation criteria, contract clauses;
- approved proof library, approved past-performance summaries, certifications with source evidence, policy excerpts;
- sanitized or synthetic examples for public fixtures.

Exclude:

- procurement portal scraping unless the user supplies an approved export;
- private customer files not approved for this pack;
- invented certifications, compliance status, references, pricing, deadlines, or past performance;
- raw confidential material in committed public paths.

### Query / Prompt Building

For local/approved corpus review, use prompts such as:

- "List every `shall`, `must`, submission instruction, and evaluation factor with source id, section, snippet, and page/line when available."
- "Map each mandatory requirement to available proof or mark it as a gap; do not infer compliance."
- "Extract due dates, submission formats, portal instructions, Q&A amendments, and reviewer owners; mark missing or conflicting facts."
- "Find proof claims that require human approval before use."

### Route Hints And Eval Checks

- Route accepted source inventory to `$mdp-proposal-pack-builder`.
- Route compliance-only review against existing supplied materials to `$mdp-proposal-compliance-review`.
- Route proof/theme checks to `$mdp-proposal-win-theme-proof-review`.
- Route risk/gap review to `$mdp-proposal-red-team-gap-review`.
- Eval checks should fail when the strategy recommends scraping gated portals by default, accepts unsupported proof, hides missing RFP sections, skips source approval status, or routes confidential raw content into public artifacts.

## Other Domains

For hiring, support, legal, customer success, partnerships, or other future profiles, keep the same shape:

- describe domain actors and decision criteria in domain language;
- identify source targets and source classes;
- require citations or approved corpus references;
- map evidence to universal primitives;
- keep unsafe/private/execution asks in exclusions;
- route reviewed evidence to extraction/review jobs, not side effects.
