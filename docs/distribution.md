# Distribution

MDP ships the Rust CLI and agent instructions together from one repository so command contracts, templates, docs, and skills stay version-aligned.

## Public Install

Install the CLI and all supported host bundles:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Install only the CLI:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

Install one host bundle plus the CLI when it is missing:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --codex -y
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --claude-code -y
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cursor -y
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --opencode -y
```

The vanity installer redirects to assets from the latest GitHub release. Rerun the same command to update. Set `MDP_VERSION` to pin a release.

## What Ships

Each release contains:

- platform-specific `mdp` CLI binaries;
- Pluxx-generated bundles and installers for Claude Code, Cursor, Codex, and OpenCode;
- `install.sh` and `install-cli.sh`;
- checksums and release metadata;
- released `llms.txt` and `llms-full.txt` files.

`plugin/` is the canonical plugin source. Pluxx packages that source for each supported host. Pluxx does not replace the Rust CLI, and the plugin does not hide a separate hosted runtime.

The tag, `cli/Cargo.toml`, `pluxx.config.ts`, and plugin manifests must use the same semantic version.

## Agent-Readable Context

Released context files are available at:

```text
https://mdp.orchidlabs.dev/llms.txt
https://mdp.orchidlabs.dev/llms-full.txt
```

The redirect source lives in `deploy/mdp-installer/`. Keep those routes pointed at released assets so install instructions and agent context describe the same version.

## Local Development

Install the source CLI locally:

```bash
make install-cli
mdp --json doctor --dir .
```

Validate the source plugin and release configuration:

```bash
make validate
```

`make validate` covers the Rust CLI, bundled templates, mirrored skill content, and the available plugin validators. Use `mdp --json capabilities` as the machine-readable command and side-effect contract.

## Updates And Drift

The plugin must not silently replace the CLI or itself during ordinary pack work. Use the explicit installer for updates and `scripts/check-update.sh` for a read-only drift check:

```bash
scripts/check-update.sh
```

The script compares the installed CLI and nearby plugin version to the latest release and prints the appropriate installer command when they differ.

## Release Closeout

Release-affecting changes are not shipped when a PR merely merges. From clean current `main`:

1. Run `make validate`.
2. Create the next approved release tag through the repository release process.
3. Confirm the expected GitHub release assets exist.
4. Run the documented installer.
5. Smoke-test the installed CLI and the behavior that changed.

Repository source, GitHub releases, and an installed artifact are three different states; closeout should report each explicitly.

## Boundaries

MDP is source-available under the [Elastic License 2.0](../LICENSE). Local/offline and internal use are allowed under its terms. Offering a hosted or managed service that exposes a substantial set of MDP functionality requires a separate commercial license; see [Commercial Use](../COMMERCIAL.md).

Distribution does not change the product boundary: MDP stores and validates decision context. It does not provide sending, CRM mutation, enrichment, scraping, sequencing, proposal submission, or a hosted MDP API.
