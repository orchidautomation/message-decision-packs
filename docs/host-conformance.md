# MDP v1 Host Conformance

Cold-model qualification composes this host-assurance vocabulary with
deterministic pack/job sufficiency and separately recorded behavioral trials.
MDP conformance validates supplied behavioral-trial evidence; it does not
perform that trial call or promote a caller-selected assurance label. A
separate `mdp run` may execute one declared model step through the optional
native driver, but that receipt is not behavioral qualification. Hard cold-context dimensions
must be `enforced` or `verified`; self-attestation alone cannot qualify them.
See [Cold-model Conformance](cold-model-conformance.md).

This guide defines how an external host can invoke the MDP clean-run boundary without inheriting an authoring conversation. It applies to ephemeral coding agents, Clay-style table jobs, and customer-controlled/BYOK workers.

MDP remains a local/offline decision-context standard and deterministic authority. It does not become a scheduler, credential vault, model gateway, enrichment service, sequencer, or generalized production API. MDP Cloud's current gateway is bounded and synthetic; it is not the endpoint described by this guide.

Job-owned `mdp.prompt.v1` contracts make the boundary explicit. MDP compiles
the exact prompt, declared input producers, selected product authority, output
schema, version, and hash. The customer host still owns workflow sequencing,
credentials, provider approval, and external actions. It may execute the step
itself or explicitly select MDP's local BYOK native driver.

## Normative Authority

The installed CLI is the authority for public contract shape, hashes, terminal-state validation, assurance derivation, and receipt verification. Hosts must consume the schemas instead of copying them into a second implementation:

```bash
mdp --json schema run-request-v1
mdp --json schema run-bundle-v1
mdp --json schema driver-request-v1
mdp --json schema driver-result-v1
mdp --json schema driver-request-v2
mdp --json schema driver-result-v2
mdp --json schema runner-audit-v1
mdp --json schema run-receipt-v1
mdp --json schema run-verification-v1
```

All contracts are closed JSON Schemas. Reject unknown fields, duplicate JSON members, floats, negative zero, integers outside the JavaScript-safe range, oversized payloads, and contract-version mismatches before execution. Do not accept an extension field as an unaudited way to pass extra context.

The [synthetic conformance envelopes](../examples/run-conformance/) demonstrate the wire contracts. Fixture data is not evidence of a provider call or a maintained integration.

## Boundary and Driver Protocol

The host supplies `mdp.run-request.v1` to the local CLI. MDP resolves and freezes the pack release and declared artifacts, then creates `mdp.run-bundle.v1`. For the bundled native path, the operation selects one resolver-emitted model-step ID and the CLI creates a closed `mdp.driver-request.v2` containing the exact model-visible bytes. External v1 drivers continue to receive content-addressed staged authority through `mdp.driver-request.v1`.

The run output root is also a filesystem boundary: place it in a new external
customer-controlled scratch/work directory, never at the active pack root or
under it. The Rust CLI and stdio MCP adapter compare canonical path components
and refuse unsafe roots before creating output parents, claims, or
transactions. If a pack already contains generated run evidence, validation
returns one remediation diagnostic for the generated root and does not delete
the evidence.

An external v1 driver follows one bounded protocol:

1. Receive exactly one UTF-8 `mdp.driver-request.v1` JSON object on stdin.
2. Resolve each logical artifact only inside the read-only staged root provided by the runtime. Reject absolute paths, `..`, symlinks, hard-link substitutions, sockets, devices, and files absent from the bundle.
3. Construct one fresh provider request or one fresh headless task. Do not resume, continue, attach, or import conversation history.
4. Keep stdout reserved for exactly one UTF-8 `mdp.driver-result.v1` JSON object. Send bounded, redacted diagnostics to stderr.
5. Return either `success` with one output authority or a `no-draft:*` state with `output: null`. Always return an audit authority for a staged `mdp.runner-audit.v1` artifact.
6. Exit. Do not publish, send, sync to CRM, or trigger another row from inside the driver.

The bundled native driver is narrower. Rust constructs the v2 envelope, starts
`scripts/mdp-native-model-openai.mjs` with a cleared allowlisted environment,
and permits only the official OpenAI Responses endpoint. Real calls are
default-deny: both `MDP_ALLOW_NATIVE_MODEL_CALLS=1` and `OPENAI_API_KEY` must be
present before the CLI or MCP server starts. Neither value may be supplied by
the run request or an MCP tool argument. Dry-run and mock fixtures require no
key and do not prove a provider call.

One native run executes one declared normalization, generation, or review
step. The customer host must make deterministic fit/routing and later model
steps separate calls with separately declared inputs and receipts.

The driver request contains artifact authority, not raw workspace context. An implementation-specific launcher may communicate the staged root through an inherited file descriptor or one allowlisted environment variable; that launcher detail is included in the driver configuration hash and audit limitations. No other ambient environment variable is authorized.

MDP hashes the exact provider-request bytes only when the MDP-owned native transport constructed those bytes. A customer or headless driver may attest to its request hash, but that evidence remains `driver-attested`, `host-attested`, or `customer-attested`. It cannot be relabeled `mdp-observed`, and none of these hashes proves what hidden provider instructions or transformations existed.

### Terminal behavior

| State | Required host behavior |
| --- | --- |
| `success` | The runtime completed and sealed a decision. Inspect that decision: GTM may still decide `no-draft`. Never treat terminal success alone as drafting authorization. |
| `no-draft:preflight-refused` | Do not launch the driver. Return no draft when inputs, compatibility, staging, or boundary checks fail. |
| `no-draft:runner-failed` | Kill the process tree on timeout/cancellation, quarantine partial bytes, and return no output authority. |
| `no-draft:output-invalid` | Quarantine malformed or oversized driver output; never surface it as a usable draft. |
| `no-draft:decision-invalid` | Preserve diagnostics and audit metadata, but publish no draft authority. |
| `no-draft:audit-incomplete` | Treat missing, contradictory, or unbindable audit evidence as no-draft. |
| `no-draft:policy-blocked` | Refuse before disclosure when credential, privacy, network, tool, or retention policy cannot be honored. |

A host UI may explain a failure, but commentary is not decision authority. Only the CLI-rendered receipt and verification block may authorize downstream use.

## Host Patterns

### Ephemeral coding agent

Use a brand-new non-resumed task or process with a sterile working directory containing only the staged root and runner executable. Disable repository discovery, global/user instruction discovery, plugins, MCP servers, tools, shell access, network access beyond an explicitly authorized provider endpoint, persistent memory, session caches, and output-history reuse.

A new Codex or Claude task is evidence of a fresh invocation only if those surrounding channels are controlled. Merely opening a new task does not prove declared-input isolation. If the host cannot observe or enforce a channel, record it as `unknown` or `unsupported`; do not infer enforcement from the prompt.

The original authoring conversation may launch the clean run and later display the verified receipt. It must not revise the clean-run decision, add undeclared facts, or treat its commentary as part of the published authority.

### Clay or table job

Map one row to one stable `job_id` and one idempotency identity. Freeze the row fields selected by the declared input manifest before launch; do not give the worker table-wide access. Batch orchestration may run many isolated row jobs, but a batch ID is not a substitute for row identity.

On retry, consume the verified receipt in the host's durable transaction:

- the exact same job ID, idempotency key, receipt hash, and original prior version may be classified as `permitted-exact-replay` when policy allows it;
- the same job with changed row bytes, key, or receipt is `duplicate` and must not overwrite the first authority;
- reuse of a key or receipt across another row/job is `cross-job` and must fail closed;
- an unexpected ledger version is `prior-version-mismatch` and must be reconciled before any downstream action.

Do not retry a generative call blindly after an ambiguous timeout. First determine whether a receipt was durably consumed. If that cannot be determined, stop for reconciliation.

### Customer-controlled or BYOK worker

Run the CLI and driver in the customer's trust domain. Inject provider credentials only into the transport process that needs them, never the model-visible prompt, pack, bundle, stdout result, receipt, or ordinary logs. Pin the driver artifact/configuration and provider endpoint. Clear proxy variables unless a registered enforcing proxy is part of the declared boundary.

BYOK changes who owns credentials and transport; it does not automatically improve assurance. Customer observations remain customer-attested unless an enforcing control or independent verifier produced stronger evidence.

### Platform responsibility matrix

| Surface | Ephemeral coding agent | Clay/table job | Customer/BYOK worker |
| --- | --- | --- | --- |
| Invocation identity | Host creates a non-resumed task/process ID | Host binds one stable row/job ID and idempotency key | Customer scheduler binds job and worker identity |
| Declared data | Sterile staged directory only | Frozen declared columns for one row only | Content-addressed staged bundle only |
| Hidden-context risk | User/global instructions, plugins, MCP, repository and session discovery | Neighboring rows, formulas, enrichment columns, prior cell results | Mounted customer files, inherited environment, sidecars and caches |
| Credential owner | Host; credentials must bypass model-visible context | Table platform/customer; never place secrets in columns passed to MDP | Customer secret manager and transport process |
| Retry owner | Host task orchestrator | Row/batch orchestrator with durable row transaction | Customer queue/workflow engine |
| Retention owner | Host session, trace, and artifact policy | Table history, job logs, and column-retention policy | Customer storage, provider policy, backups, and legal holds |
| Strongest ordinary provenance | `host-attested`, unless independently enforced/observed | `host-attested`; table visibility gaps often remain unknown | `customer-attested`, plus verifier-recomputed artifact evidence |
| Downstream action | Separate user/host approval | Separate column update, sequence, or webhook authorization | Separate customer policy and transaction |

These are reference mappings, not verified integration listings. A named product becomes supported only after its maintained adapter passes the end-to-end suite and that evidence is recorded separately.

## Assurance Mapping

Hosts report observations; the CLI derives and verifies assurance. A host must never accept a caller-selected label or implement an alternative “audit-grade” boolean.

For the bundled native MDP route, configuration and model-parameter hashes are
also runtime-bound identities. The request fields are declarations. Rust owns
the closed `mdp.driver-configuration.v1` and `mdp.model-parameters.v1`
projections, compares their hashes with those declarations before publication,
and records the observed values in the bundle and runner audit. The provider
request-body SHA is separate full-body evidence: it includes model parameters
and model-visible input, but it does not prove the launcher configuration or
replace the model-parameter projection. Hosts must keep keys, environment
values, raw payloads, and private paths out of identity material and ordinary
diagnostics.

| Dimension | Strongest evidence a typical host can supply | Mandatory downgrade examples |
| --- | --- | --- |
| `fresh-invocation` | Observed new process/request with resume and session attachment disabled | New task asserted only; session/cache behavior hidden |
| `declared-input-isolation` | Enforced staged filesystem, environment allowlist, tool denial, and bounded network | Workspace mounted, home/config discovery possible, undeclared row/table fields available |
| `stateless-request-construction` | Exact one-request construction observed, with no prior messages/session ID | Headless tool or provider may silently attach history |
| `filesystem-enforcement` | Sandbox/container policy observed and bound to the run | Read-only flag asserted but host home or repository remains visible |
| `tool-enforcement` | No tools registered plus process/tool events independently observed | Prompt says “do not use tools” |
| `network-enforcement` | Egress denied or restricted to validated HTTPS endpoint tuples | DNS/proxy/redirect path uncontrolled |
| `artifact-integrity` | MDP/verifier recomputed hashes from published bytes | Host merely reports a digest; artifact root unavailable |
| `validation` | MDP validation artifact recomputed and bound to output | Host-side schema success without exact output binding |
| `replay-protection` | Host-owned atomic consumption with durable monotonic state | Local JSONL ledger, signature alone, cache with rollback/cloning risk |
| `authoring-provenance` | Source/pack provenance separately preserved | Clean execution is incorrectly said to cleanse polluted source selection |

Use only the schema states `declared`, `observed`, `enforced`, `verified`, `unknown`, `redacted`, `unsupported`, and `not-applicable`, with the exact provenance vocabulary `mdp-observed`, `provider-returned`, `customer-attested`, `host-attested`, `driver-attested`, `verifier-recomputed`, or `unknown`.

Driver-attested evidence cannot elevate a dimension to `enforced` or `verified`. A signature authenticates bytes or an issuer; it does not establish freshness, containment, or truthful source content.

## Replay Contract

`mdp consume-run` is a conformance and local-pilot reference. It uses an exclusive lock and append-only hash chain, and it fails closed on corruption or interrupted append. It cannot detect filesystem rollback, snapshot restore, or cloned ledgers.

Production hosts own a single atomic operation that:

1. verifies the expected job and idempotency identities;
2. verifies the receipt hash and terminal state;
3. compares the expected prior monotonic version;
4. classifies first acceptance, permitted exact replay, duplicate, cross-job substitution, or version mismatch;
5. records the receipt consumption and any downstream authorization in the same durable transaction.

Do not “repair” a corrupt ledger automatically, reuse a receipt across profiles, or let a retry create a new idempotency identity. Backup restore and regional failover need an external monotonic trust anchor or an explicit reconciliation stop.

## Host-Owned Operations

MDP does not own these production concerns:

- batching, queues, schedules, row fan-out, rate limits, backoff, retry budgets, and poison-job handling;
- provider and customer credentials, secret rotation, endpoint authorization, tenancy, quotas, and billing;
- lawful source access, source truth, consent, privacy classification, and data residency;
- durable idempotency, replay state, regional consistency, backup/restore, and disaster recovery;
- raw provider-response retention, artifact retention/deletion, audit-log access, and legal holds;
- monitoring, incident response, abuse prevention, support escalation, and model-provider outages;
- CRM writes, email drafting/sending, Clay updates, campaign enrollment, proposal submission, and every other downstream action.

Downstream actions require a separately authorized host policy after successful receipt verification and durable consumption. A `success` receipt does not send, publish, or approve anything by itself.

## Certification Checklist

A host is conformant only when it can demonstrate all applicable items with synthetic fixtures:

- It validates the CLI-exported schemas and rejects unknown fields and malformed authority JSON.
- It provides the driver exactly the staged declared artifacts and proves what filesystem, environment, tool, and network surfaces remained available.
- It never resumes an authoring conversation and never calls a new task “isolated” without surrounding-control evidence.
- It binds driver identity, configuration, model/provider request identity, pack release, prompt, input manifest, and execution policy.
- It returns no output authority for every `no-draft:*` result and quarantines partial output.
- It lets MDP derive assurance and preserves all limitations and provenance without elevation.
- It independently verifies published artifact hashes before durable receipt consumption.
- It distinguishes exact row retry, duplicate, cross-job substitution, and stale prior version atomically.
- It keeps secrets and raw private payloads out of receipts, fixtures, stdout, ordinary logs, and public issue reports.
- It requires separate authorization for every downstream side effect.

Passing these fixtures establishes conformance to this local contract, not certification by Orchid Labs, provider behavior, regulatory compliance, or production readiness. A maintained integration support claim requires separately recorded end-to-end evidence.

### Offline executable suite

Run conformance against the CLI build or installed release being evaluated:

```bash
cargo build --manifest-path cli/Cargo.toml
node scripts/test-run-conformance.mjs
```

For an installed artifact, set `MDP_BIN` to its absolute path. The command must
report every case passed. Use `--keep` only for local diagnosis; generated
scratch may contain copied declared inputs and is never release evidence.

The black-box suite covers declared-input closure, unknown fields, symlink and
Unix hard-link refusal, logical path escape, malformed/duplicate/oversized
JSON, output-directory reuse, no-draft authority absence, receipt and artifact
tampering, assurance non-elevation, and local replay classifications and
corruption handling. The Rust runtime suite supplies the deterministic
source-mutation race hook; release validation must run both suites. Neither
suite can prove external host isolation, provider statelessness, or production
replay durability.

## Adversarial Cases Every Host Must Test

- Hidden home-directory instructions, repository files, global plugins, MCP tools, shell aliases, environment variables, proxies, caches, or resumed sessions alter the result.
- A staged file is replaced between hashing and invocation, or a symlink/hard link escapes the staged root.
- The driver reads a neighboring row, a previous batch output, stderr history, or an undeclared network source.
- Provider redirects, DNS rebinding, proxy inheritance, or userinfo routes credentials/request bytes to an unauthorized endpoint.
- A driver asserts `verified`, forges an assurance array, returns success without complete artifacts, or returns a draft with `no-draft:*`.
- A valid receipt is copied to another job, row, profile, pack release, or prior ledger state.
- A timeout leaves a descendant process running or publishes a partial response.
- Logs, traces, crash dumps, or retained raw responses expose secrets or private source material.
- Backup restore or ledger cloning makes an already consumed receipt appear fresh.
- The authoring agent adds ambient facts after the clean run and presents the combined narrative as the MDP decision.

Any unmitigated case must produce an explicit downgrade or no-draft result. Silence is not evidence.
