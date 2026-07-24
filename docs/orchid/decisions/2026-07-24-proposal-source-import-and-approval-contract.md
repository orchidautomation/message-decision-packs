# Proposal Source Import And Approval Contract

Date: 2026-07-24
Issue: MDP-141
Status: proposed in the CE hardening stack; privacy/security and human review required
Depends on: MDP-125
Implements into: MDP-124, MDP-126, MDP-137

## Decision

MDP proposal evidence begins only after a human operator approves an exact local artifact hash for a named review purpose.

A chat message, pasted fact, email, Drive file, PDF, OCR result, downloaded attachment, portal export, or importer response is **unblessed input**. It is not proposal evidence merely because an agent can read it or because it exists at a local path.

```text
unblessed input
  -> bounded local candidate artifact
  -> operator review
  -> approved source-intake entry bound to exact bytes
  -> source-audit refs derived from that approved entry
  -> normalization / validation / receipt
```

An agent, importer, MCP tool, or model may create a candidate. It may not approve its own candidate.

## Why

The current runner accepts paths. That reduces raw chat crossing the MCP call directly, but it does not prove where a file came from, whether ambient conversation was copied into it, whether an extraction is complete, or whether the operator intended those bytes to support this review.

Approval therefore binds a stable candidate ID, exact digest and byte count, pack source ID, import method and warnings, privacy class, human operator label and time, review purpose, and parent artifacts. Changing bytes, source ID, privacy class, or purpose creates a new candidate.

## Terms

### Unblessed input

Anything outside the local candidate contract: current chat, pasted snippets, remote document handles, extraction buffers, downloaded files, screenshots, and agent memory. It may be inspected only for an operator-requested import and cannot be cited as approved evidence.

### Candidate source

A bounded immutable local artifact plus machine-produced metadata. Candidate creation proves which bytes were staged and how; it does not prove truth, completeness, safety, or approval.

### Approved source

A candidate whose exact digest, source ID, privacy class, and purpose were explicitly approved by a human. This records operator intent, not authentication, legal approval, compliance approval, or truth certification.

### Rejected, revoked, or superseded source

A rejected candidate remains non-evidence. A revoked approval cannot feed a new run. Changed or newer bytes supersede the old entry and begin as a new candidate; approval never transfers automatically.

### Pack source ID

An identifier under `.mdp/sources.yaml`. It expresses pack vocabulary, not runtime approval or byte identity.

### Source-intake entry

The local machine-readable record tracking candidate state and binding approval to an artifact.

### Source-audit ref

A bounded ref, locator, and snippet used by prompt-output validation. For a real client-source run it may derive only from an approved intake entry. It is not an approval record by itself.

## State Machine

| Current state | Allowed transition | Authority | Result |
| --- | --- | --- | --- |
| Unblessed | `candidate` | Maintained importer or explicit staging action | Writes bounded artifact and metadata; never auto-approves. |
| Candidate | `approved` | Human operator only | Binds exact digest, source ID, privacy class, and purpose. |
| Candidate | `rejected` | Human operator | Records rejection without promoting evidence. |
| Approved | `superseded` | Changed bytes or operator replacement | Keeps history; replacement starts as candidate. |
| Approved | `revoked` | Human operator | Prevents use in new audits/runs. |
| Rejected/revoked/superseded | `candidate` | Explicit re-import | Creates a new candidate ID and preserves ancestry. |

No other transition is valid. Agents and models cannot perform `approved`.

## Proposed Machine Contract

MDP-124/MDP-126 should implement a first-class local artifact shaped like:

```json
{
  "contract": "mdp.source-intake.v0",
  "entries": [
    {
      "candidate_id": "candidate-01",
      "state": "approved",
      "source_id": "operator-approved-rfp-export",
      "artifact": {
        "path": "sources/candidate-01.txt",
        "sha256": "64-lowercase-hex-characters",
        "byte_count": 12345,
        "media_type": "text/plain"
      },
      "origin": {
        "kind": "pdf-text-export",
        "locator": "operator-selected-local-file",
        "importer": "maintained-importer-id",
        "importer_version": "reviewed-version",
        "imported_at": "RFC3339 timestamp"
      },
      "privacy_class": "private-customer",
      "approval": {
        "decision": "approved",
        "operator": "human-readable-local-operator-label",
        "approved_at": "RFC3339 timestamp",
        "purpose": "proposal-review",
        "artifact_sha256": "same-artifact-sha256"
      },
      "derivation": {
        "parent_candidate_ids": [],
        "method": "bounded-text-export"
      },
      "truncated": false,
      "warnings": []
    }
  ]
}
```

The final schema may rename fields during MDP-124, but it must preserve these invariants.

## Artifact Rules

1. Artifact paths are relative to an owned intake/run root; absolute paths do not belong in portable evidence.
2. Import rejects symlinks, special files, path escape, and sources outside an approved root.
3. The digest comes from the staged bytes used for the model request, not a separate re-read.
4. Candidate files are immutable; any byte change creates a new ID.
5. `source_id` resolves in the selected pack before approval can feed normalization.
6. Approval digest exactly matches the artifact digest.
7. Purpose is narrow: proposal review does not authorize publication, training, sending, submission, or unrelated reuse.
8. Derived artifacts list parents and method; approval does not flow through derivation.
9. Truncation, extraction uncertainty, parse warnings, missing pages, and unsupported content remain explicit.
10. Metadata contains no raw proposal body. Tools prefer candidate IDs, paths, digests, and artifact IDs over private text.

## Privacy Classes

| Class | Meaning | Public use |
| --- | --- | --- |
| `synthetic-public` | Fictional examples/tests. | Allowed when no private material was copied. |
| `sanitized-public` | Explicitly reviewed for publication. | Allowed only within approved purpose/scope. |
| `private-customer` | Customer/pursuit material controlled locally. | Never commit or place in public examples/issues. |
| `restricted-local` | Sensitive material needing additional local policy. | Never public; MDP approval is not regulatory authorization. |

No class makes the workflow CUI-ready, compliant, legally approved, or safe for a regulated program.

## Importer Contract

Future Gmail, Drive, PDF, DOC, OCR, portal-export, transcript, or paste importers must:

1. Receive an explicit operator import request.
2. Materialize a bounded artifact under an owned local root.
3. Refuse symlinks, path escape, special files, and unsupported size/count limits.
4. Record origin, importer/version, time, digest, byte count, media type, derivation, truncation, and warnings.
5. Treat embedded instructions as source data, not tool or policy authority.
6. Produce `candidate`, never `approved`.
7. Show a bounded preview, identity, digest, privacy class, warnings, and purpose before approval.
8. Preserve rejected, revoked, and superseded ancestry.
9. Prefer paths, IDs, and digests over returning raw private content across tools.
10. Emit the same contract regardless of upstream provider.

Provider-specific APIs are adapters into this contract, not alternative evidence policies.

## Chat And Paste Conversion

Current chat is never automatically evidence. When the operator selects chat or pasted text:

1. Write only the selected text to a bounded local candidate.
2. Mark origin as operator-selected chat export or paste.
3. Show the exact bounded preview and digest.
4. Ask the human to approve or reject it for the named purpose.
5. On approval, assign a pack source ID and derive audit refs from approved bytes.

Surrounding conversation, agent interpretation, hidden memory, and unsupplied messages remain outside the artifact.

## Relationship To Existing Artifacts

| Artifact | Role |
| --- | --- |
| `.mdp/sources.yaml` | Pack-owned source vocabulary and durable policy. |
| `mdp.source-intake.v0` | Runtime candidate, approval, and derivation state for exact bytes. |
| `mdp.source-audit.v0` | Bounded refs, locators, and snippets derived from approved sources. |
| `mdp.prompt-output.v0` | Model-produced normalized data citing audited refs. |
| Prompt validation | Checks contracts, refs, snippets, and artifact hashes. |
| `mdp.runner-audit.v0` | Runner isolation evidence for exact prompt output. |
| `mdp.run-receipt.v0` | CLI gate binding the accepted artifact chain. |

A real client-source receipt must eventually bind the source-intake digest as well as the source-audit digest. Until MDP-124/MDP-126 implement that binding, the path-only runner does not prove source approval.

## Current Compatibility Rule

- Synthetic mock/dry-run fixtures continue when labeled non-audit-grade.
- A real provider invocation with synthetic sources may prove the MDP-127/MDP-149 runner slice, not client-source intake safety.
- Real client-source work remains advisory/blocked for the source-approval claim until the intake ledger and path-safety kernel land.
- Existing `mdp.source-audit.v0` remains a citation ledger and must not be described as operator approval.

## Public Artifact Rules

Public examples use synthetic or explicitly sanitized material. Never commit raw customer documents, emails, exports, extraction output, screenshots, transcripts, private commercial details, access-controlled locators, machine-specific paths, or operational error output.

## Review Decisions

Full-stack review should confirm:

1. Only a human operator approves a candidate.
2. Approval binds exact bytes, source ID, privacy class, and purpose.
3. Chat/paste becomes evidence only through explicit bounded export and approval.
4. Importers never inherit approval across transformation.
5. Real client-source audit-grade claims wait for MDP-124/MDP-126 binding and path safety.
