# Runtime and execution

Read this reference only when choosing CLI versus MCP, preparing a governed
model step, locating artifacts, or resuming and verifying a run.

## Authority and files

The local Rust CLI owns pack discovery, validation, profile/job resolution,
normalization, routing, prompt assembly, run directories, receipts, and
verification. The selected pack and supplied inputs are read-only during Apply.
Use only paths returned by CLI JSON; never invent an output path, write into
`.mdp/`, overwrite a source input, or select an ambient/latest run.

A clean-context evaluation receives only the exact prompt package, normalized
input, and bounded routed context prepared for the selected job. It must not
load the plugin skills, repository instructions, whole pack, unrelated profile
references, host memory, or ambient conversation as additional authority.

## CLI and MCP

Prefer the CLI when the host can execute local commands. MCP is only a thin
stdio transport for the same evaluation lifecycle and exposes exactly:

1. `mdp_run_tools`
2. `mdp_prepare_run`
3. `mdp_run`
4. `mdp_verify_run`

MCP does not discover or edit packs, manage providers, approve evidence, or
replace CLI verification. Its artifacts remain CLI-owned and must verify
against the same exact run directory.

## Native model calls and secrets

Dry-run and mock evaluation do not need a provider key. Native activation may
report only whether `OPENAI_API_KEY` is present; it must never print, copy,
cache, or persist the value. A real bundled OpenAI runner call requires both
`OPENAI_API_KEY` and explicit `MDP_ALLOW_NATIVE_MODEL_CALLS=1` in the server
start environment plus the existing one-shot consent boundary.

The portable Agent Plugins package has no native hooks or MCP declaration and
must use the CLI-only path. Never assume `PLUGIN_ROOT`, native helper scripts,
or MCP support in that package.

## Managed run

Prepare the exact job and input, inspect the returned action/tool inventory,
execute only allowed steps, and verify the output and receipt. On failure,
preserve the failed run for diagnosis. Resume only from an explicitly supplied
run directory after fresh verification; never infer a run from recency.
