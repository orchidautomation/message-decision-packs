# MDP-306: Host-aware, idempotent installer

## Goal

Make the MDP release installer a thin product orchestrator over the released
Pluxx `0.1.43` install-results contract. MDP continues to own its standalone
CLI and portable-package policy while Pluxx owns core-four host detection,
native installation, and per-host terminal states.

## Implementation

1. Preserve the Pluxx-generated aggregate installer as the checksummed
   `install-agents.sh` release asset before replacing the public front door
   with MDP's `install.sh`.
2. Have `scripts/install.sh` invoke that asset with `--json --quiet`, validate
   `pluxx.install-results.v1`, and render one branded summary. Explicit native
   flags remain strict; `--agents` uses Pluxx aggregate detection.
3. Resolve the selected MDP release version once and skip the CLI download when
   the installed CLI already reports that exact version. Keep
   `MDP_FORCE_CLI_UPDATE=1` as the explicit repair route.
4. Keep portable destination, overlap, ownership, archive, and checksum gates
   local to MDP. Mention portable routing only when explicitly requested or
   configured, and report portable unchanged when its verified tree matches.
5. Pin CI/release generation to exact Pluxx `0.1.43`, then cover clean,
   repeat, mixed-detection, explicit-missing, malformed-portable, and partial
   failure fixtures plus the existing four-host release smoke.

## Validation

- `bash -n scripts/install.sh scripts/bootstrap-runtime.sh`
- `bash scripts/test-install.sh`
- `bash scripts/test-release-install-smoke.sh`
- `make validate-installers`
- `bash scripts/validate-version-sync.sh`
- `bash scripts/test-version-sync.sh`
- affected workflow/Pluxx pin tests

No release, merge, active-home reinstall, or external mutation is part of this
change.
