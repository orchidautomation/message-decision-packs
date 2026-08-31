# MDP-300 — Fast PR mutation gate with complete-suite assurance

Status: `READY_TO_PIN`

## Context and decision

The `Authority mutations` workflow currently runs the same four-shard,
24-candidate campaign for every pull request that touches any CLI source file.
That preserves authority assurance, but it makes unrelated CLI changes wait on
the longest repository check. MDP-300 will split policy from execution:

- pull requests are classified from their changed paths;
- authority-sensitive pull requests run one deterministic smoke candidate set;
- unrelated pull requests run no mutation workers but still finish one stable
  `authority-mutations` aggregate check successfully;
- pushes to `main`, scheduled runs, manual dispatches, and release tags run the
  unchanged complete four-shard campaign.

The complete selector, 24-candidate ceiling, pinned cargo-mutants version,
timeouts, cache isolation, and surviving-mutant failure behavior remain intact.

## Scope and ownership

Implementation owns only:

- `.github/workflows/authority-mutations.yml`
- `.github/workflows/ci.yml` when required to keep the contract test routed
- `scripts/test-authority-mutations.sh`
- `scripts/test-authority-mutations-contract.mjs`
- `docs/authority-conformance.md` for the operator-facing policy rationale

Forbidden without Sol arbitration: `cli/src/**`, release publishing logic,
runner-vendor changes, generated plugin bundles, assets, schemas, version files,
or any reduction of the complete candidate set.

## Policy contract

### Trigger classes

The workflow will expose one classifier job with reviewable path groups:

1. `full`: any non-PR event supported by this workflow—`push` to `main`, a
   `schedule`, `workflow_dispatch`, or a release tag push.
2. `smoke`: a pull request touching authority implementation, authority corpus,
   the mutation runner/contract, or the mutation workflow itself.
3. `skip`: every other pull request admitted by the workflow trigger, including
   unrelated CLI source changes.

The pull-request trigger remains broad enough that the stable aggregate check
is created for both smoke and skip classes. Classification must fail closed:
missing or contradictory classifier outputs cause the aggregate to fail rather
than silently skip assurance.

### Candidate policy

The complete campaign remains the current selector
`(from_run|permits_projection)` against `src/authority/mod.rs`, capped at 24
candidates and split over `0/4` through `3/4` exactly as today.

The smoke campaign is a declared, ordered list of a small fixed number of
candidate selectors representing both authority reconstruction and projection
permission behavior. `scripts/test-authority-mutations.sh` will expose a
dedicated `--smoke` mode and deterministic `--list --smoke` output. Smoke mode
must reject sharding, produce at least one candidate for every declared smoke
selector, reject duplicates, enforce its own small cap, and invoke
cargo-mutants with the exact selected mutants. A selection error, build/test
error, timeout, surviving mutant, or runner failure remains non-zero.

### Stable aggregate

Only one job named `authority-mutations` is intended for branch protection. It
runs with `if: always()` and depends on classification, the contract check, the
optional smoke job, and the optional complete matrix. It accepts exactly:

- `smoke` with successful classifier/contract/smoke and skipped full suite;
- `skip` with successful classifier/contract and both mutation suites skipped;
- `full` with successful classifier/contract/complete matrix and skipped smoke.

All other combinations fail. Optional worker job names are never the required
branch-protection surface.

## Ordered implementation

1. Refactor `.github/workflows/authority-mutations.yml` to add schedule,
   manual, and release-tag triggers, a deterministic changed-path classifier,
   mutually exclusive smoke/full execution, and the stable aggregate truth
   table. Preserve the complete four-shard job body and cache boundaries.
2. Extend `scripts/test-authority-mutations.sh` with explicit complete and smoke
   selection modes. Preserve default complete behavior for existing callers.
   Make smoke list generation deterministic and fail closed on missing,
   duplicate, over-cap, or out-of-complete-set candidates.
3. Extend `scripts/test-authority-mutations-contract.mjs` to assert trigger and
   path classification, the exact smoke policy, complete coverage/topology,
   aggregate failure semantics, and the unrelated-PR skip result. Contract
   tests should exercise classifier/aggregate truth tables through pure helper
   logic or fixtures rather than relying only on comments or regex presence.
4. Update `.github/workflows/ci.yml` only if required to ensure changes to the
   classifier/helper contract run the existing validation step. Document the
   tiered policy and required-check name close to the existing authority docs.
5. Run focused local checks, then bind verification and review to the exact
   final commit. Push one cumulative branch and open one validated PR. Stop at
   Ready for Human; Brandon alone merges.

## Acceptance mapping and validation

| Acceptance | Proof |
|---|---|
| Unrelated CLI PRs avoid the 24-mutant campaign | Contract fixture/classifier result is `skip`; both worker suites are skipped; aggregate succeeds |
| Authority-sensitive PRs run bounded smoke | Contract fixture/classifier result is `smoke`; deterministic smoke list is non-empty, within cap, and a subset of complete candidates |
| Errors and surviving mutants fail PR gate | Script stays `set -euo pipefail`; smoke/full job failures and invalid aggregate combinations are asserted to fail |
| Main/schedule/manual/release retain complete assurance | Trigger contract maps every non-PR assurance event to `full`; matrix remains exactly `0/4..3/4` |
| All 24 candidates remain reachable | Existing complete selector/cap/topology stay unchanged; list union contract remains deterministic and disjoint when cargo-mutants is installed |
| Branch protection sees one stable check | `authority-mutations` always runs and explicitly accepts only the three valid job-result combinations |

Focused checks:

```bash
git diff --check
node scripts/test-authority-mutations-contract.mjs
bash scripts/test-authority-mutations.sh --help
```

If pinned `cargo-mutants` 27.1.0 is locally available, also prove the complete
unsharded list, every complete shard, and the smoke subset through list-only
mode. Do not run the full mutation execution locally solely to duplicate GitHub
Actions; the PR workflow supplies mutation runtime proof.

## Risks and rollback

- A too-narrow path classifier could miss an authority change. Keep workflow,
  runner, corpus, and authority module paths in the smoke class, and fail
  classifier ambiguity closed.
- GitHub skipped-job semantics can make required checks brittle. Require only
  the aggregate and test every accepted/rejected result tuple explicitly.
- Exact cargo-mutants display strings can drift. The pinned version and
  deterministic contract make drift a selection failure, never a silent skip.
- Release assurance must not depend on publishing mutation results after a tag
  is already released. The tag-triggered complete campaign is additive proof;
  existing release publication behavior remains unchanged.

Rollback is one workflow/script revert to the prior always-full PR topology.
No data migration, external configuration mutation, deployment, or runner
vendor change is involved.
