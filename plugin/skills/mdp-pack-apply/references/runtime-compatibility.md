# Runtime Compatibility

MDP skills are portable instructions; deterministic decisions still require the
installed `mdp` CLI. Before doing CLI-owned work, run:

```bash
command -v mdp
mdp --version
mdp --json capabilities
```

If `mdp` is missing, stop and report that the MDP CLI prerequisite is missing.
Point the operator to the release installer at
`https://github.com/orchidautomation/message-decision-packs/releases/latest`;
do not imitate validation, routing, authoring, or receipt decisions in prose.

A portable Agent Skills installation contains this skill directory and its
references. It does **not** imply a plugin root, bundled JavaScript helpers,
hooks, or MCP registration. Use the canonical CLI commands directly in that
layout. If the requested host cannot execute the CLI, report the host/runtime
limitation and stop at the last evidenced decision.

A native MDP plugin bundle may additionally expose helper scripts under
`${PLUGIN_ROOT}/scripts`. Before invoking one, require Node.js 18 or newer and
verify the exact script exists:

```bash
node --version
test -f "${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs"
```

Use `${PLUGIN_ROOT}` only when the host actually supplies it. Never infer it
from the current directory, this skill's directory, or a repository checkout.
If Node.js, the plugin root, or the helper is missing, name that prerequisite
and use the direct CLI path when it provides the same operation; otherwise
stop without claiming the helper ran.

MCP is optional transport, not authority. Use it only when the host can launch
and register the plugin's local stdio server. A portable skill-only install or
a host without local MCP support must use direct CLI `prepare-run`, `run`, and
`verify-run`; it must not fabricate MCP tool results or assurance.

No API key is needed for deterministic CLI work or key-free mock/dry-run
fixtures. A real bundled OpenAI model call additionally requires the
operator-supplied `OPENAI_API_KEY` and `MDP_ALLOW_NATIVE_MODEL_CALLS=1` in the
helper's startup environment. Never request, print, persist, or pass the key as
a tool argument.
