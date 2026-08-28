# MDP-278 — Unified data-first template registry and init pipeline

Status: `READY_TO_PIN`

## 1. Context and current behavior

The approved project architecture requires the two shipped templates to share
one private declarative registry and one transactional initialization control
flow. Repository inspection on cumulative head
`a16f7f6a110ef468ae9cffe7c49819a2dd17042c` confirms:

- `cli/src/commands/init.rs` owns separate GTM and proposal dispatch branches,
  builders, dry-run functions, payload functions, required-directory lists,
  and a manually maintained `PROPOSAL_TEMPLATE_FILES` `include_str!` table.
- GTM content is rendered from `cli/src/starter.rs` (or
  `cli/src/target_starter.rs` for target-aware packs); the existing golden test
  byte-compares canonical generated output with
  `plugin/assets/templates/basic`.
- Proposal content starts from checked-in assets, patches a custom manifest
  identity, regenerates the README, and is byte-compared with
  `plugin/assets/templates/proposal` for its canonical defaults.
- Both branches already converge on `commands::init_transaction` for
  preflight, staging, validation, publication, dry-run, collision handling,
  force replacement, symlink refusal, and rollback.
- Template IDs and defaults are repeated in `commands::init`, `app.rs`,
  `cli.rs`, and `commands::capabilities`; MDP-277 already declares the matching
  `gtm -> gtm` and `proposal -> proposal` associations in the private profile
  registry.
- The authored asset trees contain regular files plus required empty
  directories such as `.mdp/briefs`. The CLI has no build dependency capable
  of recursive embedding, but Cargo can run a standard-library-only build
  script and include generated Rust from `OUT_DIR`.

The existing CLI output envelopes, error text, JSON ordering, template bytes,
pack validation, release asset parity, and transaction semantics are
compatibility authority.

## 2. Objective, scope, and decisions

### Objective

Introduce one private `TemplateDescriptor` registry for `gtm` and `proposal`,
generate a deterministic recursive compile-time inventory from the canonical
authored template trees, and route lookup, option validation, inventory
construction, staging, validation, dry-run, publication, payload projection,
CLI help, and capabilities through the registry without changing canonical
output bytes or public contracts.

### In scope

- Descriptors for ID, default name, profile ID, authored asset root, supported
  init options, required directories, examples, and one closed postprocess
  enum.
- A standard-library-only Cargo build helper that recursively discovers and
  embeds regular template files and records directories in deterministic byte
  order.
- One request-to-inventory-to-transaction init flow for both templates.
- Bounded postprocessing for GTM target-aware/generated variants and proposal
  custom identity/README behavior.
- Registry-derived template lookup, unsupported-template diagnostics, default
  names, Clap help/accepted values, capabilities, and payload metadata.
- Unit and integration mutation coverage for inventory completeness and safe
  paths, plus exact canonical tree parity.

### Out of scope

- A public template/storage/API schema, runtime template discovery, dynamic
  libraries, executable callbacks, user-authored plugins, or third-profile
  activation.
- New template IDs, profile IDs, jobs, skills, CLI options, output fields,
  schema versions, or changed starter copy.
- Reworking the general validation engine or publication algorithm except for
  fail-closed artifact-relative-path validation needed by generated inventory.
- Merge, release, deployment, installation, or local host-bundle refresh.

### Decisions and assumptions

1. `TemplateDescriptor`, option metadata, examples, and `TemplatePostprocess`
   remain private immutable Rust data. The postprocess surface is a closed enum,
   not an executable plugin contract.
2. Add `cli/build.rs` and a dependency-free helper under
   `cli/build_support/`. The build scans every direct template asset root under
   `../plugin/assets/templates`, rejects symlinks, non-regular nodes, non-UTF-8
   or unsafe relative paths, sorts roots/directories/files, emits
   `cargo:rerun-if-changed`, and writes an `OUT_DIR` Rust inventory whose file
   entries use `include_bytes!`. No generated source is committed.
3. Descriptors associate the generated asset-root keys (`basic`, `proposal`)
   with the public template IDs (`gtm`, `proposal`). Registry validation rejects
   duplicate IDs/roots, missing or unreferenced roots, missing required files,
   invalid profile associations, unsorted/duplicate inventory entries, and
   absent declared examples/directories.
4. Inventory construction always begins from the descriptor's embedded files
   and directories. A bounded hook may replace or patch known entries: GTM may
   reuse the existing generated/target-aware functions for custom identity,
   inline schemas, legacy internal mode, or explicit target identity; proposal
   may patch only manifest identity and regenerate README. Hook output is
   revalidated as a complete safe inventory before transaction preflight.
5. Canonical no-option defaults use embedded bytes wherever doing so is byte
   identical. Existing generators stay only where options require derived
   content; this reduces duplicated starter construction without deleting the
   target-aware behavior needed by current CLI contracts.
6. Clap's init command is constructed with registry-backed possible values/help
   rather than another literal list. If derive attributes cannot consume the
   registry directly, add one small command-construction function and parse via
   `CommandFactory`/`FromArgMatches`; do not keep a second authoritative ID
   array.
7. The profile registry remains authoritative for profile ownership. Template
   registry validation checks each descriptor against
   `skill_catalog::profile_descriptor`; it does not duplicate job routing.

## 3. Acceptance mapping

| Acceptance criterion | Implementation | Proof |
| --- | --- | --- |
| Both templates execute the same init control flow | Resolve one descriptor/request, build one inventory, then call one dry-run or publish path and one payload projector. | Control-flow unit tests and black-box transaction tests run for both IDs. |
| Canonical defaults are byte-identical tracked trees | Seed inventories from recursively embedded authored assets; retain only bounded derived transformations. | Golden tests compare the full generated file set and every byte for both roots. |
| Dry-run, collision, force, rollback, README, and retarget refusal remain intact | Preserve `init_transaction`; invoke validation and payload projection from the unified flow; keep target-destination checks in the GTM hook. | Existing `cli/tests/init_transactional.rs` matrix plus focused proposal/GTM cases. |
| Future template work is assets plus one descriptor | Build-time discovery embeds asset roots recursively; registry metadata supplies options, directories, examples, payload behavior, and profile association. | Synthetic descriptor/inventory tests and a no-manual-file-enum source assertion. |
| No storage/API/executable plugin model | Keep all types private and all hooks as closed enum variants. | Source/review audit and registry rejection tests. |
| Help and capabilities come from registry | Project values/order from descriptor iteration. | Clap help and `mdp capabilities` parity tests assert the same ordered IDs. |
| Missing/extra/renamed/symlinked/unsafe assets fail closed | Validate build-time tree collection and runtime descriptor/inventory completeness. | Mutation fixtures exercise each case without altering canonical assets. |

## 4. Affected files and ownership

### New build-time inventory surfaces

- `cli/build.rs`: locate canonical authored roots, call the shared collector,
  emit rerun directives, and generate the compile-time inventory module.
- `cli/build_support/template_inventory.rs`: standard-library-only path and
  filesystem collector used by the build script and mutation integration test.
- `cli/tests/template_inventory_generator.rs`: temporary-tree mutation tests
  for missing, extra, renamed, symlinked, non-regular, duplicate, and unsafe
  inventory entries.

### New runtime registry

- `cli/src/template_registry.rs`: private descriptor/option/example/hook types,
  generated inventory inclusion, registry lookup/validation, ordered template
  IDs/help values, artifact kind derivation, and descriptor-to-profile parity.
- `cli/src/main.rs`: register the module and, only if needed, use the
  registry-aware Clap construction path.

### Existing consumers

- `cli/src/commands/init.rs`: delete `AVAILABLE_TEMPLATES` and
  `PROPOSAL_TEMPLATE_FILES`; replace per-template publish/dry-run dispatch with
  the unified request/inventory/transaction pipeline; preserve bounded GTM and
  proposal transformation helpers and exact error/payload behavior.
- `cli/src/app.rs`: derive the default init name from descriptor lookup and
  avoid a second template switch.
- `cli/src/cli.rs`: remove the literal available-template list and project
  accepted/help values from the registry.
- `cli/src/commands/capabilities.rs`: derive `defaults.init_templates` from
  descriptor order.
- `cli/src/commands/init_transaction.rs`: add or reuse a single safe-relative
  artifact validator before any join/stage/preflight operation; preserve
  publication modes and envelopes.
- `cli/tests/init_transactional.rs`: parameterize material transaction
  guarantees across both shipped templates where applicable.

`cli/src/starter.rs`, `cli/src/target_starter.rs`, and
`cli/src/skill_catalog.rs` may receive only narrow visibility/helper changes or
parity tests. Template asset bytes, public schemas, plugin skills, workflows,
and packaging layout are forbidden unless exact validation proves a genuinely
required compatibility fix.

## 5. Ordered implementation sequence

### Step A — Generate and validate recursive embedded assets

Implement the build-support collector with lexical component validation:
relative paths must be non-empty UTF-8, use `/`, contain only normal path
components (allowing dot-prefixed names such as `.mdp`), remain beneath the
declared root, and be unique. Reject symlinks before following metadata and
reject sockets/devices/FIFOs. Record all directories, including empty ones.

Have `build.rs` scan direct child asset roots deterministically and emit static
root/directory/file records. Generated file records embed exact bytes with
`include_bytes!`; runtime never reads repository paths. Add rerun directives
for the template parent and each discovered node.

Test the collector against synthetic trees. A missing/renamed required
manifest is rejected by descriptor completeness validation; an extra asset
root is rejected as unregistered; symlink and non-regular nodes are rejected
during collection; injected `..`, absolute, separator-confused, duplicate, and
unsorted entries are rejected by pure validators.

### Step B — Define the closed template registry

Create exactly two ordered descriptors:

- `gtm`: default `Example Message Pack`, profile `gtm`, asset root `basic`,
  custom name/target identity/inline-output-schema options, canonical required
  directories and examples, `TemplatePostprocess::Gtm`.
- `proposal`: default `Proposal Reference Profile Sample`, profile `proposal`,
  asset root `proposal`, custom name only, proposal proof-output example
  directories, `TemplatePostprocess::Proposal`.

Expose private lookup and ordered projection helpers. Validate the canonical
registry once on access and provide injectable validation helpers for mutation
tests. Validate exact profile/template association against MDP-277 without
moving template metadata into public profile output.

### Step C — Build one descriptor-driven inventory

Introduce an internal init request carrying name, target arguments, force,
inline-schema selection, dry-run, and the existing governed/legacy distinction.
Resolve the descriptor before option validation. Copy its embedded files and
directories into `GeneratedArtifact` values, apply the selected closed hook,
then validate uniqueness, containment, required files/directories, and examples.

For GTM, preserve target lexicon and retarget refusal exactly. Use embedded
canonical bytes for the no-option case and existing typed generators only for
the files/variants that require custom name, target identity, inline schemas,
or legacy internal behavior. Regenerate README after any authority mutation.

For proposal, patch only the manifest `id`/`name` for a non-default name and
regenerate README from the resulting inventory. README parsing must iterate the
inventory rather than a manual file constant.

### Step D — Unify transaction, dry-run, and payload projection

Replace `init_gtm_pack*`/`init_proposal_pack*` dispatch with one function that:

1. resolves and validates the request/descriptor;
2. builds and validates the complete generated inventory;
3. calls `tx_dry_run` or `run_publish` once;
4. runs existing strict staged pack validation before publication;
5. creates required empty directories through inventory entries, not an
   after-publication side effect; and
6. projects the current public payload and publication envelope from descriptor
   metadata plus the bounded hook-specific example/next-command values.

Preserve exact unsupported-template and target-option errors, JSON fields,
default slugs, next commands, dry-run write-plan semantics, staging cleanup,
and rollback behavior.

### Step E — Derive CLI and capabilities projections

Make `app.rs` obtain default names from the descriptor. Make Clap show and
enforce the registry's ordered possible values without a literal `gtm,
proposal` string. Build `capabilities.defaults.init_templates` from the same
ordered registry iterator. Add a parity test that help, parse acceptance,
unsupported-template diagnostics, capabilities, descriptors, and profile
associations expose exactly the same two IDs in the same order.

### Step F — Compatibility and evidence

Run focused generator, registry, init, transaction, help, and capabilities
tests during implementation. Run full Rust and repository checks only after the
tree stabilizes. Create a public-safe exact-head QA receipt under
`docs/orchid/qa/` after implementation/review only; do not include private
Linear prose, `/tmp` paths, tokens, or host state.

## 6. Compatibility invariants

- Public template IDs and order remain `gtm`, then `proposal`.
- Defaults remain `Example Message Pack` and
  `Proposal Reference Profile Sample`.
- Canonical tracked GTM and proposal trees contain the exact same paths and
  bytes, including `.mdp/README.md` and empty required directories.
- Custom proposal identity serialization, GTM custom/target-aware generation,
  inline output schemas, target-crossing refusal, and all existing diagnostics
  remain unchanged.
- `mdp.capabilities.v1`, CLI JSON envelopes, dry-run write plans, publication
  envelopes, next commands, and exit behavior do not gain or lose fields.
- Artifact relative paths cannot be absolute, parent-relative, duplicated,
  separator-confused, or symlink-mediated.
- Build output embeds bytes only; no runtime repository path, filesystem scan,
  host state, or network dependency is introduced.
- Exactly two descriptors and two existing profiles are active. No runtime
  extension point or third template is shipped.

## 7. Validation commands

Run from repository root:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml template_inventory_generator
cargo test --manifest-path cli/Cargo.toml template_registry
cargo test --manifest-path cli/Cargo.toml commands::init
cargo test --manifest-path cli/Cargo.toml init_transactional
cargo test --manifest-path cli/Cargo.toml commands::capabilities
cargo test --manifest-path cli/Cargo.toml cli
cargo test --manifest-path cli/Cargo.toml
make validate-template validate-asset-sync validate-public-artifacts
node scripts/test-release-workflow.mjs
git diff --check
```

Create fresh default GTM and proposal packs with the exact built binary,
byte-compare each with its authored asset root, then require strict `validate`
and `eval` success for both. Exercise
custom names, GTM target-aware init, inline schemas, dry-run, collision without
force, force replacement, symlink refusal, late publication failure, rollback,
and cleanup.

## 8. Risks, rollout, and rollback

- **Build-time path escape or symlink traversal:** fail before code generation,
  validate again at runtime before joining paths, and cover adversarial fixtures.
- **Generator/asset drift:** recursive build discovery plus descriptor
  completeness and full-tree golden tests make additions/removals explicit.
- **Serialization drift:** prefer embedded canonical bytes; constrain hooks to
  existing typed serializers and compare every output byte.
- **Transaction regression:** do not rewrite publication mechanics; parameterize
  its black-box matrix across descriptors and bind proof to the exact head.
- **Help/error drift:** derive values from registry while preserving current
  wording and ordering with snapshots/assertions.

Rollout is the existing cumulative PR only. Rollback is commit-level reversal
of all MDP-278 changes together; do not leave generated inventory, registry
consumers, or init dispatch on different authorities. No data migration or
runtime compatibility shim is required.

## 9. Blockers and readiness verdict

MDP-278 has no remaining Linear blocker. Repository ownership, cumulative PR,
profile associations, asset roots, transaction kernel, test seams, and delivery
boundary are known. The implementation may proceed on
`codex/mdp-278-template-registry`, then integrate its exact verified head into
`codex/mdp-273-primitive-contracts` and update cumulative PR #236. Do not create
a second PR, add optional `@codex review`, merge, release, deploy, or install.

Readiness: `READY_TO_PIN`.
