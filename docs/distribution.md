# Distribution Notes

The intended public shape is one repo containing both the CLI and the Codex plugin:

```text
message-decision-packs/
  cli/
  plugin/
  docs/
```

## Why One Repo

The CLI and plugin are tightly coupled:

- the CLI defines the pack schema, JSON contracts, validation, routing, entry routing, fit checks, claim checks, gaps, eval fixtures, and brief emission
- the plugin teaches agents how to author, inspect, and use those contracts
- examples, eval fixtures, and templates need to stay aligned with CLI behavior

Keeping them together avoids version drift while the standard is young.

## Local Use

Install the CLI:

```bash
make install-cli
```

Use the plugin source at `plugin/` when testing local Codex plugin installs.

## Future Distribution

Possible later channels:

- GitHub releases for CLI binaries
- Homebrew formula for `mdp`
- npm wrapper only if agent workflows need Node distribution
- Codex/agent plugin marketplace entry for `plugin/`
- hosted MDP API that can serve validated packs and briefs

Do not treat hosted serving as part of the local MVP. The current implementation is offline and auth-free.
