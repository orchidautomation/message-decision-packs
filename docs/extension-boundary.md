# Extension boundary

This is the maintainer checklist for extending the shipped MDP registry. It
describes the current closed system: the only registered runtime profiles and
templates are `gtm` and `proposal`. Ordinary pack authoring uses those
registrations; it does not create new ones.

## Terms and ownership

| Layer | Meaning | Repository authority |
| --- | --- | --- |
| Primitive | One of ten fixed, domain-agnostic decision families: actors, decision criteria, source signals, needs/requirements, evidence/proof, boundaries, output contracts, routing/jobs, gaps, and evals. | `cli/src/primitives.rs`; public vocabulary in `CONCEPTS.md` |
| Profile | A reviewed domain mapping: vocabulary, primitive map, input contracts/adapter, jobs, eval categories, and exactly one template association. | `cli/src/skill_catalog.rs` and pack profile validation |
| Template | An authored starter tree for one already registered profile, with metadata, asset root, required directories, examples, and bounded post-processing. It is not a profile. | `cli/src/template_registry.rs`, build-time inventory, and `plugin/assets/templates/` |
| Pack | A local `.mdp/` directory containing decision context and routing contracts. Editing one uses or declares an existing profile; it does not register a profile, template, or skill. | Pack files and the Rust CLI validator |
| Skill | Authored agent instructions for a supported operator or profile job. A job route must point to a packaged skill. | `plugin/skills/` (authored source), `cli/src/skill_catalog.rs`, and Pluxx packaging |
| Host | The customer-controlled environment that owns connectors, credentials, model/provider execution, sequencing, and external side effects. It does not extend MDP vocabulary or promote compatibility evidence. | Host boundary; MDP remains the deterministic CLI authority |

The registries fail closed: profile and template IDs, jobs, adapters, skill
routes, asset roots, required directories, examples, and associations must be
unique and complete. Capabilities and help are derived from those authorities,
not from free text or dynamically discovered plugins.

## Maintaining a template for an existing profile

The shipped registry is one-to-one: each profile has exactly one associated
template, and each template belongs to exactly one profile. A maintainer may
deliberately maintain or replace that single template/descriptor, but adding a
second template to an existing profile is unsupported without a separately
reviewed architecture and public-contract change. Only do this as reviewed
repository source work:

1. Update or replace the template descriptor and its unique profile association
   in the template registry; preserve the one-profile/one-template invariant.
2. Add the authored asset root and build-time inventory for every file and
   required directory, including the manifest and declared examples.
3. Use only bounded post-processing already supported by the CLI. Do not add a
   provider call, orchestration, or runtime API.
4. Check the profile descriptor, jobs, packaged skill routes, adapter,
   primitive map, input contracts, eval categories, and template ID remain
   consistent. Capabilities and `init --template` help must share the registry.
5. Keep `plugin/skills/` as authored source; generated host bundles remain
   Pluxx's packaging responsibility.
6. Run profile conformance, template/asset/version, public artifact,
   plugin/skill, packaging, and exact-head CI checks before shipping.

## Adding a new reviewed profile

A new profile is a reviewed extension, not a pack edit. It must arrive with
exactly one associated template and add a unique profile descriptor with unique
jobs, one packaged skill route per job, one input adapter, explicit primitive
mappings and input contracts, required eval categories, authored template
assets, and any needed skill guidance. Validate capabilities/help parity, asset
inventory, common
conformance, Pluxx packaging, and the full exact-head CI matrix. Keep the
registry fail-closed: never activate a profile by filename, prose, provider
response, or host configuration.

There is no third profile implied or supported by this guide. A neutral test
fixture, if present in implementation or conformance evidence, is test-only
and is not a profile, template, skill, or product capability.

## Compatibility table

| Existing name/path | Status |
| --- | --- |
| `normalized_prospect` | Retained compatibility field for proposal integrations. `normalized_opportunity`, where present, is its exact alias; neither is a new primitive. |
| Route-budget v0 `job` | Deprecated alias retained equal to canonical `job_id`. |
| Proposal v0 runner and MCP | Compatibility-only paths; they cannot upgrade v1 authority, isolation, or assurance. |
| GTM-shaped file and example names | Existing compatibility surfaces remain readable and tested; they do not define core ontology or authorize a new profile. |

## Safety boundary

Do not add primitives, dynamic runtime plugins, provider calls, orchestration,
credentials, sending, CRM mutation, scraping, enrichment, sequencing, or
external side-effect authority as an extension. Hosts own those integrations;
the CLI validates and projects reviewed pack authority. Keep compatibility
names labeled, preserve existing commands and contracts, and stop rather than
change a forbidden runtime surface when the documentation cannot be truthful.
