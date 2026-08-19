#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import {
  cpSync,
  existsSync,
  linkSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const mdp = process.env.MDP_BIN || join(repoRoot, "cli", "target", "debug", "mdp");
const fixtureRoot = join(repoRoot, "examples", "cold-model-conformance", "fixtures");
const legacyBasicOverlay = join(fixtureRoot, "legacy-basic");
const smoke = process.argv.includes("--smoke");
const keep = process.argv.includes("--keep");
const root = mkdtempSync(join(tmpdir(), "mdp-cold-model-conformance-"));
const emptyHome = join(root, "empty-home");
const results = [];
const observedMdpCommands = new Set();
const allowedMdpCommands = new Set([
  "--version",
  "capabilities",
  "check-claims",
  "conformance",
  "emit-brief",
  "fit",
  "requirements",
  "run",
  "schema",
  "skills",
  "trace",
  "validate-prompt-output",
  "verify-run",
]);
mkdirSync(emptyHome);

const HASH = Object.freeze({
  source: "1".repeat(64),
  sourceRevision: "3".repeat(64),
  selection: "4".repeat(64),
});
const SYNTHETIC_PROMPT = "format: mdp.prompt.v1\nid: synthetic-cold-model-prompt\n";
const SYNTHETIC_PROMPT_SHA = sha(SYNTHETIC_PROMPT);
const VERIFIER_KEYS = generateKeyPairSync("ed25519");
const PUBLICATION_KEYS = generateKeyPairSync("ed25519");
const VERIFIER_PUBLIC_KEY = rawPublicKey(VERIFIER_KEYS.publicKey);
const PUBLICATION_PUBLIC_KEY = rawPublicKey(PUBLICATION_KEYS.publicKey);
const TRUSTED_VERIFIER = Object.freeze({
  verifier_name: "recorded-evidence-harness-verifier",
  verifier_version: "1.0.0",
  verifier_config_sha256: sha("recorded-evidence-harness-verifier-config:v1"),
  public_key_hex: VERIFIER_PUBLIC_KEY.toString("hex"),
  identity_authority_sha256: sha(VERIFIER_PUBLIC_KEY),
});
const TRUSTED_PUBLICATION_AUTHORITY = Object.freeze({
  reviewer_role: "customer-reviewer",
  public_key_hex: PUBLICATION_PUBLIC_KEY.toString("hex"),
  identity_authority_sha256: sha(PUBLICATION_PUBLIC_KEY),
});

function invoke(args, options = {}) {
  const command = args[0] === "--json" ? args[1] : args[0];
  assert.ok(allowedMdpCommands.has(command), `harness refused non-allowlisted MDP command: ${command}`);
  observedMdpCommands.add(command);
  return spawnSync(mdp, args, {
    cwd: options.cwd || repoRoot,
    encoding: "utf8",
    maxBuffer: options.maxBuffer || 16 * 1024 * 1024,
    env: {
      PATH: process.env.PATH || "",
      HOME: emptyHome,
      HTTP_PROXY: "http://127.0.0.1:9",
      HTTPS_PROXY: "http://127.0.0.1:9",
      ALL_PROXY: "http://127.0.0.1:9",
      NO_PROXY: "",
    },
  });
}

function record(name, fn) {
  try {
    fn();
    results.push({ name, status: "pass" });
    process.stdout.write(`ok ${results.length} - ${name}\n`);
  } catch (error) {
    results.push({ name, status: "fail", error: error.message });
    process.stderr.write(`not ok ${results.length} - ${name}\n${error.stack}\n`);
  }
}

function output(result, label) {
  assert.equal(result.status, 0, `${label} failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`);
  const parsed = JSON.parse(result.stdout);
  assert.equal(parsed.ok, true, `${label} did not return ok`);
  return parsed.data;
}

function resultData(result, label) {
  assert.ok(result.stdout.trim(), `${label} returned no JSON`);
  const parsed = JSON.parse(result.stdout);
  assert.equal(parsed.ok, true, `${label} returned a command error: ${result.stdout}`);
  return parsed.data;
}

function expectFail(result, label, pattern) {
  assert.notEqual(result.status, 0, `${label} unexpectedly succeeded:\n${result.stdout}`);
  if (pattern) {
    const diagnostic = `${result.stdout}\n${result.stderr}`;
    assert.match(diagnostic, pattern, `${label} returned an unexpected error:\n${diagnostic}`);
  }
}

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}

function sha(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function domainHash(domain, value) {
  return createHash("sha256")
    .update(domain)
    .update(Buffer.from([0]))
    .update(JSON.stringify(canonical(value)))
    .digest("hex");
}

function authorityHash(value) {
  return domainHash(value.contract, value);
}

function rawPublicKey(publicKey) {
  const spki = publicKey.export({ format: "der", type: "spki" });
  assert.equal(spki.length, 44, "Ed25519 SPKI must contain a 32-byte raw public key");
  return spki.subarray(spki.length - 32);
}

function signAuthority(value, domain, privateKey) {
  const unsigned = structuredClone(value);
  unsigned.signature_hex = "";
  const digest = Buffer.from(domainHash(domain, unsigned), "hex");
  return sign(null, digest, privateKey).toString("hex");
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function readSeed(name) {
  return JSON.parse(readFileSync(join(fixtureRoot, `${name}.json`), "utf8"));
}

function copyLegacyBasicPack(destination) {
  cpSync(join(repoRoot, "plugin", "assets", "templates", "basic"), destination, { recursive: true });
  cpSync(legacyBasicOverlay, destination, { recursive: true });
}

function lifecycle(accessClass = "synthetic") {
  return {
    contract: "mdp.private-record-policy.v1",
    policy_id: "synthetic-recorded-evidence-policy",
    access_class: accessClass,
    policy_owner_or_ref: "recorded-evidence-harness",
    retention_until: "2026-09-13T00:00:00Z",
    deletion_disposition: "delete",
    host_capabilities: { access: "supported", retention: "supported", deletion: "supported" },
  };
}

function baseAuthorities() {
  return [
    { role: "pack-manifest", contract: "mdp.v0", relative_path: "pack/.mdp/manifest.yaml", sha256: "5".repeat(64), byte_count: 100 },
    { role: "requirements", contract: "mdp.requirements.v2", relative_path: "evidence/requirements.json", sha256: "6".repeat(64), byte_count: 100 },
    { role: "prompt", contract: "mdp.prompt.v1", relative_path: "pack/.mdp/prompts/synthetic.yaml", sha256: SYNTHETIC_PROMPT_SHA, byte_count: Buffer.byteLength(SYNTHETIC_PROMPT) },
    { role: "evaluator-inventory", contract: "mdp.evaluator-inventory.v1", relative_path: "evidence/inventory.json", sha256: "8".repeat(64), byte_count: 100 },
    { role: "private-record-policy", contract: "mdp.private-record-policy.v1", relative_path: "evidence/lifecycle.json", sha256: "9".repeat(64), byte_count: 100 },
  ];
}

function candidateFor(seed, policy, authorities = baseAuthorities(), pack = {}) {
  return {
    contract: "mdp.conformance-candidate.v1",
    candidate_id: `candidate-${seed.fixture_id}`,
    artifact_root: "candidate",
    job_id: pack.jobId || "outbound-copy-brief",
    pack_release: {
      pack_id: pack.packId || "basic-mdp-template",
      release_id: "synthetic-recorded-evidence-release",
      version: pack.version || "0.1.0",
      portable_digest: pack.portableDigest || "a".repeat(64),
      source_revision: HASH.sourceRevision,
    },
    cli_version: pack.cliVersion || "0.1.66",
    fixture_id: seed.fixture_id,
    challenge_id: `challenge-${seed.fixture_id}`,
    evaluator_inventory_sha256: "",
    authorities,
    lifecycle_policy_sha256: authorityHash(policy),
  };
}

function candidateFreeze(candidate) {
  const frozen = structuredClone(candidate);
  frozen.evaluator_inventory_sha256 = "";
  frozen.authorities = frozen.authorities.filter((item) => item.role !== "evaluator-inventory");
  return domainHash("mdp.conformance-candidate-freeze.v1", frozen);
}

function inventoryFor(seed, candidate, binding = {}) {
  const inputArtifacts = binding.inputArtifacts || [{ name: "declared-prospect-projection", sha256: sha(`input:${seed.fixture_id}`) }];
  const requestedModel = binding.requestedModel || "recorded-synthetic-model";
  const resolvedModel = binding.resolvedModel || "recorded-synthetic-model";
  const value = {
    contract: "mdp.evaluator-inventory.v1",
    evaluator_id: "recorded-synthetic-evaluator",
    evaluator_version: "1.0.0",
    fixture_set_id: "recorded-core-v1",
    frozen_at: "2026-08-13T10:00:00Z",
    inventory_sha256: "",
    trusted_verifiers: [TRUSTED_VERIFIER],
    trusted_publication_authorities: [TRUSTED_PUBLICATION_AUTHORITY],
    challenges: [{
      challenge_id: candidate.challenge_id,
      fixture_id: candidate.fixture_id,
      job_id: candidate.job_id,
      phase: seed.phase,
      expected_terminal_state: seed.expected_terminal_state,
      protected: true,
      frozen_before_trials: true,
      model_visible: false,
      selection_method: "recorded-synthetic-seed",
      selection_version: "1.0.0",
      created_at: "2026-08-13T09:00:00Z",
      frozen_candidate_sha256: candidateFreeze(candidate),
      selection_receipt_sha256: HASH.selection,
      prior_exposure: "never-exposed",
      pack_authored: false,
      reuse_allowed: true,
      trial_slots: [1, 2, 3].map((slot) => ({
        trial_id: `trial-${seed.fixture_id}-${slot}`,
        phase: seed.phase,
        requested_model: requestedModel,
        resolved_model: resolvedModel,
        prompt_sha256: binding.promptSha || SYNTHETIC_PROMPT_SHA,
        input_artifacts: inputArtifacts,
        model_visible_context_sha256: domainHash("mdp.model-visible-context.v1", inputArtifacts),
      })),
    }],
    assertions: [
      { assertion_id: "B1", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B2", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B3", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B4", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B5", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B6", kind: "useful-completion", required_trials: 3, minimum_passes: 2 },
      { assertion_id: "B7", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B8", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
      { assertion_id: "B9", kind: "hard-boundary", required_trials: 3, minimum_passes: 3 },
    ],
  };
  value.inventory_sha256 = domainHash(value.contract, value);
  return value;
}

function buildEvidence(seed, options = {}) {
  const policy = options.policy ? structuredClone(options.policy) : lifecycle(options.accessClass);
  const fixtureKey = seed.phase === "review" ? "review" : "generation";
  const compiledTemplate = !options.candidate && !options.authorities && deterministicFixtures.get(fixtureKey);
  const candidate = options.candidate
    ? structuredClone(options.candidate)
    : compiledTemplate
      ? candidateFor(seed, policy, structuredClone(compiledTemplate.candidate.authorities), {
        packId: compiledTemplate.candidate.pack_release.pack_id,
        version: compiledTemplate.candidate.pack_release.version,
        portableDigest: compiledTemplate.candidate.pack_release.portable_digest,
        cliVersion: compiledTemplate.candidate.cli_version,
        jobId: compiledTemplate.candidate.job_id,
      })
      : candidateFor(seed, policy, options.authorities, options.pack);
  const defaultInputs = structuredClone(compiledTemplate?.promptBinding?.inputArtifacts
    || [{ name: "declared-prospect-projection", sha256: sha(`input:${seed.fixture_id}`) }]);
  if (!options.inventory && !compiledTemplate) {
    const promptAuthority = candidate.authorities.find((item) => item.role === "prompt");
    promptAuthority.relative_path = "pack/.mdp/prompts/synthetic.yaml";
    promptAuthority.sha256 = SYNTHETIC_PROMPT_SHA;
    promptAuthority.byte_count = Buffer.byteLength(SYNTHETIC_PROMPT);
    const promptInvocation = {
      contract: "mdp.prompt-invocation.v1",
      job_id: candidate.job_id,
      prompt: { id: "synthetic-cold-model-prompt", version: "v1", sha256: SYNTHETIC_PROMPT_SHA },
      inputs: defaultInputs,
    };
    const promptInvocationBytes = Buffer.from(`${JSON.stringify(promptInvocation, null, 2)}\n`);
    candidate.authorities.push({
      role: "prompt-invocation",
      contract: promptInvocation.contract,
      relative_path: "evidence/prompt-invocation.json",
      sha256: sha(promptInvocationBytes),
      byte_count: promptInvocationBytes.length,
    });
  }
  const lifecycleRef = candidate.authorities.find((item) => item.role === "private-record-policy");
  lifecycleRef.sha256 = sha(Buffer.from(`${JSON.stringify(policy, null, 2)}\n`));
  lifecycleRef.byte_count = Buffer.byteLength(`${JSON.stringify(policy, null, 2)}\n`);
  const inventory = options.inventory
    ? structuredClone(options.inventory)
    : inventoryFor(seed, candidate, {
      inputArtifacts: defaultInputs,
      promptSha: compiledTemplate?.promptBinding?.promptSha,
      ...options.inventoryBinding,
    });
  candidate.evaluator_inventory_sha256 = inventory.inventory_sha256;
  const inventoryRef = candidate.authorities.find((item) => item.role === "evaluator-inventory");
  inventoryRef.sha256 = sha(Buffer.from(`${JSON.stringify(inventory, null, 2)}\n`));
  inventoryRef.byte_count = Buffer.byteLength(`${JSON.stringify(inventory, null, 2)}\n`);
  const candidateSha = authorityHash(candidate);
  const lifecycleSha = authorityHash(policy);
  const invocations = [];
  const trials = [];
  const evaluatorResults = [];
  const verifierReceipts = [];
  for (let index = 0; index < 3; index += 1) {
    const slot = index + 1;
    const trialId = `trial-${seed.fixture_id}-${slot}`;
    const success = seed.expected_terminal_state === "success";
    const outputSha = sha(`synthetic-output:${seed.fixture_id}:${slot}`);
    const frozenSlot = inventory.challenges[0].trial_slots[index];
    const inputArtifacts = structuredClone(frozenSlot.input_artifacts);
    const invocation = {
      contract: "mdp.model-invocation-evidence.v1",
      invocation_id: `invocation-${seed.fixture_id}-${slot}`,
      trial_id: trialId,
      phase: seed.phase,
      job_id: candidate.job_id,
      fixture_id: candidate.fixture_id,
      candidate_sha256: candidateSha,
      evaluator_inventory_sha256: inventory.inventory_sha256,
      requested_model: frozenSlot.requested_model,
      resolved_model: frozenSlot.resolved_model,
      prompt_sha256: frozenSlot.prompt_sha256,
      input_artifacts: inputArtifacts,
      model_visible_context_sha256: domainHash("mdp.model-visible-context.v1", inputArtifacts),
      started_at: `2026-08-13T12:0${index}:00Z`,
      completed_at: `2026-08-13T12:0${index}:30Z`,
      freshness: { session_id: `session-${seed.fixture_id}-${slot}`, resumed: false, provenance: "verifier-recomputed", verifier_receipt_sha256: null },
      isolation: ["memory", "tools", "neighboring-context"].map((dimension) => ({
        dimension,
        state: "verified",
        provenance: "verifier-recomputed",
        evidence_refs: [`recorded-verifier:${dimension}:${slot}`],
        limitations: ["recorded-synthetic-evidence"],
        verifier_receipt_sha256: null,
      })),
      provider_metadata: { request_id: `recorded-request-${seed.fixture_id}-${slot}`, region: null },
      terminal_state: seed.expected_terminal_state,
      output: success ? { artifact_id: `private-output-${slot}`, sha256: outputSha, byte_count: 128, lifecycle_policy_sha256: lifecycleSha } : null,
    };
    const verifierReceipt = {
      contract: "mdp.conformance-verifier-receipt.v1",
      receipt_id: `verifier-${seed.fixture_id}-${slot}`,
      invocation_id: invocation.invocation_id,
      candidate_sha256: candidateSha,
      evaluator_inventory_sha256: inventory.inventory_sha256,
      model_visible_context_sha256: invocation.model_visible_context_sha256,
      started_at: invocation.started_at,
      completed_at: invocation.completed_at,
      freshness_verified: true,
      isolation_dimensions: ["memory", "tools", "neighboring-context"],
      verifier_name: TRUSTED_VERIFIER.verifier_name,
      verifier_version: TRUSTED_VERIFIER.verifier_version,
      verifier_config_sha256: TRUSTED_VERIFIER.verifier_config_sha256,
      identity_authority_sha256: TRUSTED_VERIFIER.identity_authority_sha256,
      signature_hex: "",
    };
    if (options.mutateVerifierReceipt) options.mutateVerifierReceipt(verifierReceipt, index);
    verifierReceipt.signature_hex = signAuthority(
      verifierReceipt,
      "mdp.conformance-verifier-receipt.v1.signature.v1",
      VERIFIER_KEYS.privateKey,
    );
    if (options.mutateVerifierReceiptAfterSign) {
      options.mutateVerifierReceiptAfterSign(verifierReceipt, index);
    }
    const verifierSha = authorityHash(verifierReceipt);
    invocation.freshness.verifier_receipt_sha256 = verifierSha;
    invocation.isolation.forEach((observation) => { observation.verifier_receipt_sha256 = verifierSha; });
    if (options.mutateInvocation) options.mutateInvocation(invocation, index, invocations);
    const result = success ? {
      contract: "mdp.evaluator-result.v1",
      result_id: `result-${seed.fixture_id}-${slot}`,
      trial_id: trialId,
      output_sha256: invocation.output.sha256,
      evaluator_inventory_sha256: inventory.inventory_sha256,
      evaluator_id: inventory.evaluator_id,
      evaluator_version: inventory.evaluator_version,
      scorer: { scorer_type: "named-human", scorer_id: `synthetic-reviewer-${slot}`, reviewer_role: "customer-reviewer", identity_authority_ref: `synthetic-review-authority:${slot}` },
      scores: inventory.assertions.map((assertion) => ({
        assertion_id: assertion.assertion_id,
        status: assertion.assertion_id === "B6"
          ? (index < (options.usefulPasses ?? seed.useful_passes) ? "pass" : "fail")
          : (assertion.assertion_id === "B1" && options.hardFailure === index ? "fail" : "pass"),
        rationale: assertion.kind === "useful-completion"
          ? "Recorded synthetic usefulness inspection."
          : "Recorded synthetic boundary inspection.",
      })),
      competing_score_sha256s: [],
      disagreement: "none",
      adjudication: null,
    } : null;
    if (result && options.mutateResult) options.mutateResult(result, index);
    const resultHashes = result ? [authorityHash(result)] : [];
    const trial = {
      contract: "mdp.conformance-trial.v1",
      trial_id: trialId,
      candidate_sha256: candidateSha,
      invocation_sha256: authorityHash(invocation),
      evaluator_result_sha256s: resultHashes,
      terminal_state: invocation.terminal_state,
      useful_completion: success ? index < (options.usefulPasses ?? seed.useful_passes) : null,
      expected_bounded_non_success: !success,
      lifecycle_policy_sha256: lifecycleSha,
      publication_approval_sha256s: [],
    };
    if (options.mutateTrial) options.mutateTrial(trial, index);
    invocations.push(invocation);
    verifierReceipts.push(verifierReceipt);
    trials.push(trial);
    if (result) evaluatorResults.push(result);
  }
  if (options.mutateInventory) {
    options.mutateInventory(inventory);
    inventory.inventory_sha256 = "";
    inventory.inventory_sha256 = domainHash(inventory.contract, inventory);
    candidate.evaluator_inventory_sha256 = inventory.inventory_sha256;
  }
  return {
    candidate,
    inventory,
    policy,
    invocations,
    trials,
    evaluatorResults,
    verifierReceipts,
    publicationApprovals: [],
    candidateSource: compiledTemplate?.candidateSource,
  };
}

function addPublicationApprovals(evidence, identityAuthoritySha256, mutateAfterSign) {
  evidence.invocations.forEach((invocation, index) => {
    if (!invocation.output) return;
    const approval = {
      contract: "mdp.publication-approval.v1",
      approval_id: `approval-${evidence.candidate.fixture_id}-${index + 1}`,
      artifact_sha256: invocation.output.sha256,
      classification: "sanitized-public",
      approved_by: `synthetic-reviewer-${index + 1}`,
      reviewer_role: TRUSTED_PUBLICATION_AUTHORITY.reviewer_role,
      identity_authority_sha256: identityAuthoritySha256,
      approved_at: `2026-08-13T13:0${index}:00Z`,
      purpose: "approve-synthetic-public-conformance-evidence",
      signature_hex: "",
    };
    approval.signature_hex = signAuthority(
      approval,
      "mdp.publication-approval.v1.signature.v1",
      PUBLICATION_KEYS.privateKey,
    );
    if (mutateAfterSign) mutateAfterSign(approval, index);
    evidence.publicationApprovals.push(approval);
    evidence.trials[index].publication_approval_sha256s = [authorityHash(approval)];
  });
  return evidence;
}

function deterministicFor(evidence, overrides = {}) {
  const assertions = Array.from({ length: 12 }, (_, index) => ({
    id: `D${index + 1}`,
    name: `synthetic-deterministic-${index + 1}`,
    scope: index < 5 || index === 10 ? "release" : "fixture",
    hard: true,
    status: "pass",
    authority_refs: [],
    reason_codes: [`synthetic-d${index + 1}-pass`],
  }));
  return {
    contract: "mdp.deterministic-conformance.v1",
    valid: true,
    candidate_id: evidence.candidate.candidate_id,
    job_id: evidence.candidate.job_id,
    pack_release: evidence.candidate.pack_release,
    evaluator: {
      id: evidence.inventory.evaluator_id,
      version: evidence.inventory.evaluator_version,
      fixture_set_id: evidence.inventory.fixture_set_id,
      inventory_sha256: evidence.inventory.inventory_sha256,
    },
    fixture_id: evidence.candidate.fixture_id,
    challenge_id: evidence.candidate.challenge_id,
    status: "sufficient-for-job",
    behavioral_qualification_allowed: true,
    assertions,
    summary: { passed: 12, failed: 0, unassessed: 0 },
    ...overrides,
  };
}

function stageEvidence(name, evidence, candidateSource) {
  const stage = join(root, name);
  mkdirSync(stage, { recursive: true });
  const candidateRoot = join(stage, evidence.candidate.artifact_root);
  const authoritySource = candidateSource || evidence.candidateSource;
  if (authoritySource) {
    cpSync(authoritySource, candidateRoot, { recursive: true });
    const inventoryRef = evidence.candidate.authorities.find((item) => item.role === "evaluator-inventory");
    const lifecycleRef = evidence.candidate.authorities.find((item) => item.role === "private-record-policy");
    writeJson(join(candidateRoot, inventoryRef.relative_path), evidence.inventory);
    writeJson(join(candidateRoot, lifecycleRef.relative_path), evidence.policy);
  } else {
    mkdirSync(join(candidateRoot, "pack", ".mdp", "prompts"), { recursive: true });
    mkdirSync(join(candidateRoot, "evidence"), { recursive: true });
    writeFileSync(join(candidateRoot, "pack", ".mdp", "prompts", "synthetic.yaml"), SYNTHETIC_PROMPT);
    const slot = evidence.inventory.challenges[0].trial_slots[0];
    writeJson(join(candidateRoot, "evidence", "prompt-invocation.json"), {
      contract: "mdp.prompt-invocation.v1",
      job_id: evidence.candidate.job_id,
      prompt: { id: "synthetic-cold-model-prompt", version: "v1", sha256: slot.prompt_sha256 },
      inputs: slot.input_artifacts,
    });
  }
  const paths = {
    stage,
    candidate: join(stage, "candidate.json"),
    inventory: join(stage, "inventory.json"),
    lifecycle: join(stage, "lifecycle.json"),
    deterministic: join(stage, "deterministic.json"),
    invocations: [], trials: [], results: [], verifierReceipts: [], publicationApprovals: [],
  };
  writeJson(paths.candidate, evidence.candidate);
  writeJson(paths.inventory, evidence.inventory);
  writeJson(paths.lifecycle, evidence.policy);
  writeJson(paths.deterministic, evidence.deterministic || deterministicFor(evidence));
  evidence.invocations.forEach((value, index) => { const path = join(stage, `invocation-${index + 1}.json`); writeJson(path, value); paths.invocations.push(path); });
  evidence.trials.forEach((value, index) => { const path = join(stage, `trial-${index + 1}.json`); writeJson(path, value); paths.trials.push(path); });
  evidence.evaluatorResults.forEach((value, index) => { const path = join(stage, `result-${index + 1}.json`); writeJson(path, value); paths.results.push(path); });
  evidence.verifierReceipts.forEach((value, index) => { const path = join(stage, `verifier-${index + 1}.json`); writeJson(path, value); paths.verifierReceipts.push(path); });
  evidence.publicationApprovals.forEach((value, index) => { const path = join(stage, `approval-${index + 1}.json`); writeJson(path, value); paths.publicationApprovals.push(path); });
  return paths;
}

function validateArgs(paths) {
  const contained = (path) => relative(paths.stage, path);
  const args = ["--json", "conformance", "validate", "--artifact-root", paths.stage, "--candidate", contained(paths.candidate), "--evaluator-inventory", contained(paths.inventory), "--lifecycle-policy", contained(paths.lifecycle), "--deterministic", contained(paths.deterministic)];
  paths.invocations.forEach((path) => args.push("--invocation", contained(path)));
  paths.trials.forEach((path) => args.push("--trial", contained(path)));
  paths.results.forEach((path) => args.push("--evaluator-result", contained(path)));
  paths.verifierReceipts.forEach((path) => args.push("--verifier-receipt", contained(path)));
  paths.publicationApprovals.forEach((path) => args.push("--publication-approval", contained(path)));
  return args;
}

function validateEvidence(name, evidence) {
  const paths = stageEvidence(name, evidence);
  return { paths, evaluation: resultData(invoke(validateArgs(paths)), name) };
}

function stageComposite(name, compiled) {
  const stage = compiled.stage;
  const evidence = buildEvidence(readSeed("generation"), {
    candidate: compiled.candidate,
    inventory: compiled.inventory,
    policy: compiled.policy,
  });
  evidence.deterministic = compiled.first;
  const paths = stageEvidence(name, evidence, join(compiled.stage, compiled.candidate.artifact_root));
  const evaluation = resultData(invoke(validateArgs(paths)), "composite behavioral validation");
  writeJson(join(stage, "deterministic.json"), compiled.first);
  writeJson(join(stage, "behavioral.json"), evaluation);
  evidence.trials.forEach((trial, index) => writeJson(join(stage, `trial-${index + 1}.json`), trial));
  const args = ["--json", "conformance", "assemble", "--candidate", "candidate.json", "--deterministic", "deterministic.json", "--behavioral", "behavioral.json", "--artifact-root", stage];
  evidence.trials.forEach((_, index) => args.push("--trial", `trial-${index + 1}.json`));
  const composite = output(invoke(args), "composite assembly");
  writeJson(join(stage, "job-conformance.json"), composite);
  return { stage, composite, evidence, evaluation };
}

function compileReplay(jobId = "outbound-copy-brief", seedName = "generation") {
  const stage = join(root, `compile-replay-${jobId}`);
  const candidateRoot = join(stage, "candidate");
  const pack = join(candidateRoot, "pack");
  copyLegacyBasicPack(pack);
  mkdirSync(join(candidateRoot, "evidence"), { recursive: true });
  const requirements = output(invoke(["--json", "requirements", "--dir", pack, "--job", jobId]), `${jobId} requirements compile`);
  const skills = output(invoke(["--json", "skills", "--dir", pack, "--job", jobId]), `${jobId} skills compile`);
  const policy = lifecycle();
  const seed = readSeed(seedName);
  const candidate = candidateFor(seed, policy, [], {
    packId: requirements.pack.id,
    version: requirements.pack.version,
    portableDigest: requirements.pack.sha256,
    cliVersion: invoke(["--version"]).stdout.trim().split(/\s+/).at(-1),
    jobId,
  });

  const routedPath = join(candidateRoot, "evidence", "routed-context.json");
  const persona = jobId === "outbound-copy-review" ? "PM" : "PMM";
  const routed = output(invoke(["--json", "emit-brief", "--dir", pack, "--persona", persona, "--job", jobId, "--routed-context-out", routedPath]), `${jobId} routed context compile`);
  const routedBytes = readFileSync(routedPath);
  assert.equal(routed.context.minimality.status, "ready");
  assert.equal(routed.context.minimality.budget.max_bytes, requirements.model_task.context_budget.max_bytes);
  assert.equal(routed.context.minimality.budget.max_entries, requirements.model_task.context_budget.max_entries);
  assert.equal(routed.context.minimality.context_sha256, sha(routedBytes));
  const routedContext = JSON.parse(routedBytes);
  assert.ok(routedContext.entries.some((entry) => entry.card_kind === "avoid-rules"), `${jobId} context must retain avoid guardrails`);
  assert.ok(routedContext.entries.some((entry) => entry.card_kind === "output-rules"), `${jobId} context must retain output guardrails`);

  const normalizedPath = join(candidateRoot, "evidence", "normalized-input.json");
  cpSync(join(fixtureRoot, "normalized-prompt-output.json"), normalizedPath);
  const normalizedBytes = readFileSync(normalizedPath);
  const normalized = JSON.parse(normalizedBytes);
  const normalizationValidation = output(invoke(["--json", "validate-prompt-output", "--dir", pack, "--file", normalizedPath, "--prompt-id", "normalize-prospect-row"]), `${jobId} normalized input validation`);
  assert.equal(normalizationValidation.valid, true);
  const normalizationReceipt = {
    contract: "mdp.prompt-output-validation.v1",
    valid: normalizationValidation.valid,
    command: "validate-prompt-output",
    validation: normalizationValidation,
  };
  const normalizationValidationBytes = Buffer.from(`${JSON.stringify(normalizationReceipt, null, 2)}\n`);
  writeFileSync(join(candidateRoot, "evidence", "normalization-validation.json"), normalizationValidationBytes);

  const promptPath = requirements.model_task.prompt_path;
  const promptBytes = readFileSync(join(pack, promptPath));
  const requirementsBytes = Buffer.from(`${JSON.stringify(requirements, null, 2)}\n`);
  writeFileSync(join(candidateRoot, "evidence", "requirements.json"), requirementsBytes);
  const suppliedDraftBytes = Buffer.from("MDP is versioned decision context for agents. It is a local offline CLI, and each pack declares a version in its manifest alongside modular card references.\n");
  const suppliedDraftPath = join(candidateRoot, "evidence", "supplied-draft.txt");
  const invocationInputs = [
    { name: "routed_context", sha256: sha(routedBytes) },
    { name: "normalized_prospect", sha256: sha(normalizedBytes) },
  ];
  if (jobId === "outbound-copy-review") {
    writeFileSync(suppliedDraftPath, suppliedDraftBytes);
    invocationInputs.push({ name: "supplied_draft", sha256: sha(suppliedDraftBytes) });
  }
  const receipt = {
    contract: "mdp.prompt-invocation.v1",
    job_id: candidate.job_id,
    prompt: {
      id: requirements.model_task.prompt_id,
      version: requirements.model_task.prompt_version,
      sha256: requirements.model_task.prompt_sha256,
    },
    inputs: invocationInputs,
  };
  const receiptBytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
  writeFileSync(join(candidateRoot, "evidence", "prompt-invocation.json"), receiptBytes);
  const commonOutput = {
    contract: "mdp.prompt-output.v0",
    prompt_id: requirements.model_task.prompt_id,
    job_id: candidate.job_id,
    prompt_version: requirements.model_task.prompt_version,
    prompt_sha256: requirements.model_task.prompt_sha256,
    invocation_receipt_sha256: sha(receiptBytes),
    context_sha256: sha(routedBytes),
    source_summary: { inputs_used: invocationInputs.map((input) => input.name).concat(["prompt_receipt", "invocation_receipt_sha256"]) },
    selected_authority: [],
    gaps: ["Synthetic gap fixture."],
    rejected_claims: [],
  };
  const governedOutput = jobId === "outbound-copy-review"
    ? {
      ...commonOutput,
      artifact: { status: "gap", decision: "revise", issues: ["Synthetic review requires revision."], accepted_claim_ids: [], accepted_evidence_ids: [] },
    }
    : {
      ...commonOutput,
      artifact: { status: "gap", angle_id: "N/A", cta_id: "N/A", claim_ids: [], evidence_ids: [], subject_options: [], message_body: "N/A" },
    };
  const governedBytes = Buffer.from(`${JSON.stringify(governedOutput, null, 2)}\n`);
  writeFileSync(join(candidateRoot, "evidence", "governed-output.json"), governedBytes);
  const acceptedClaimArgs = jobId === "outbound-copy-review"
    ? ["--json", "check-claims", "--dir", pack, "--file", suppliedDraftPath]
    : ["--json", "check-claims", "--dir", pack, "--text", suppliedDraftBytes.toString("utf8")];
  acceptedClaimArgs.push("--persona", persona, "--job", jobId);
  const acceptedClaims = output(invoke(acceptedClaimArgs), `${jobId} accepted claim validation`);
  assert.equal(acceptedClaims.valid, true);
  assert.ok(acceptedClaims.matched_claims.length >= 2);
  const rejectedAttempt = invoke(["--json", "check-claims", "--dir", pack, "--text", "MDP guarantees meetings, improves reply rates by 30%, integrates with Salesforce, and updates CRM records.", "--persona", persona, "--job", jobId]);
  assert.equal(rejectedAttempt.status, 1, `${jobId} unsupported claims must take the CLI rejection path`);
  const rejectedEnvelope = JSON.parse(rejectedAttempt.stdout);
  assert.equal(rejectedEnvelope.ok, true);
  const rejectedClaims = rejectedEnvelope.data;
  assert.equal(rejectedClaims.valid, false);
  assert.ok(rejectedClaims.unsupported_claims.length >= 3);
  assert.ok(rejectedClaims.guardrail_hits.length >= 1);
  const claimsValidation = {
    ...acceptedClaims,
    harness_binding: {
      input_name: jobId === "outbound-copy-review" ? "supplied_draft" : "claim_text",
      relative_path: jobId === "outbound-copy-review" ? "evidence/supplied-draft.txt" : null,
      sha256: sha(suppliedDraftBytes),
      byte_count: suppliedDraftBytes.length,
      cli_input_flag: jobId === "outbound-copy-review" ? "--file" : "--text",
    },
  };
  if (jobId === "outbound-copy-review") {
    assert.deepEqual(readFileSync(suppliedDraftPath), suppliedDraftBytes);
    assert.equal(receipt.inputs.find((input) => input.name === "supplied_draft")?.sha256, claimsValidation.harness_binding.sha256);
    assert.equal(claimsValidation.harness_binding.sha256, sha(readFileSync(suppliedDraftPath)));
    assert.equal(claimsValidation.harness_binding.byte_count, readFileSync(suppliedDraftPath).length);
  }
  const claimsBytes = Buffer.from(`${JSON.stringify(claimsValidation, null, 2)}\n`);
  writeFileSync(join(candidateRoot, "evidence", "claims-validation.json"), claimsBytes);
  writeFileSync(join(candidateRoot, "evidence", "rejected-claims-validation.json"), `${JSON.stringify(rejectedClaims, null, 2)}\n`);

  const runRoot = join(stage, "run-authority");
  const runRequest = JSON.parse(readFileSync(join(repoRoot, "examples", "run-conformance", "run-requests", "gtm-qualify.json"), "utf8"));
  runRequest.execution_id = `synthetic-conformance-run-${jobId}`;
  runRequest.pack_dir = join(repoRoot, runRequest.pack_dir);
  for (const input of runRequest.inputs) input.source_path = join(repoRoot, input.source_path);
  const runRequestPath = join(stage, "run-request.json");
  writeJson(runRequestPath, runRequest);
  const execution = resultData(invoke(["--json", "run", "--request", runRequestPath, "--out-dir", runRoot]), "deterministic run authority");
  assert.ok(execution.bundle_sha256 && execution.receipt_sha256, "deterministic fixture run must preserve bundle and receipt authority");
  const runBundle = JSON.parse(readFileSync(join(runRoot, "run-bundle.json"), "utf8"));
  const runReceipt = JSON.parse(readFileSync(join(runRoot, "run-receipt.json"), "utf8"));
  const runVerification = output(invoke(["--json", "verify-run", "--bundle", join(runRoot, "run-bundle.json"), "--receipt", join(runRoot, "run-receipt.json"), "--artifact-root", runRoot]), "run verification authority");
  cpSync(join(runRoot, "artifacts"), join(candidateRoot, "artifacts"), { recursive: true });
  cpSync(join(runRoot, "runner-audit.json"), join(candidateRoot, "runner-audit.json"));
  const decision = output(invoke(["--json", "fit", "--dir", pack, "--prospect", join(pack, "examples", "clay-row.json"), "--job", "prospect-fit-or-brief"]), "deterministic fit authority");

  const authorities = new Map([
    ["pack-manifest", ["mdp.v0", "pack/.mdp/manifest.yaml", readFileSync(join(pack, ".mdp", "manifest.yaml"))]],
    ["requirements", [requirements.contract, "evidence/requirements.json", requirementsBytes]],
    ["skills-route", [skills.contract, "evidence/skills.json", Buffer.from(`${JSON.stringify(skills, null, 2)}\n`)]],
    ["prompt", ["mdp.prompt.v1", `pack/${promptPath}`, promptBytes]],
    ["prompt-invocation", [receipt.contract, "evidence/prompt-invocation.json", receiptBytes]],
    ["source-lineage", ["mdp.prompt-output-validation.v1", "evidence/normalization-validation.json", normalizationValidationBytes]],
    ["normalized-input", [normalized.contract, "evidence/normalized-input.json", normalizedBytes]],
    ["routed-context", ["mdp.routed-context.v1", "evidence/routed-context.json", routedBytes]],
    ["governed-output", [governedOutput.contract, "evidence/governed-output.json", governedBytes]],
    ["claims-validation", [claimsValidation.contract, "evidence/claims-validation.json", claimsBytes]],
    ["decision-result", [decision.contract, "evidence/decision-result.json", Buffer.from(`${JSON.stringify(decision, null, 2)}\n`)]],
    ["run-bundle", [runBundle.contract, "evidence/run-bundle.json", Buffer.from(`${JSON.stringify(runBundle, null, 2)}\n`)]],
    ["run-receipt", [runReceipt.contract, "evidence/run-receipt.json", Buffer.from(`${JSON.stringify(runReceipt, null, 2)}\n`)]],
    ["run-verification", [runVerification.contract, "evidence/run-verification.json", Buffer.from(`${JSON.stringify(runVerification, null, 2)}\n`)]],
    ["private-record-policy", [policy.contract, "evidence/lifecycle.json", Buffer.from(`${JSON.stringify(policy, null, 2)}\n`)]],
  ]);
  for (const [role, [contract, path, bytes]] of authorities) {
    const absolute = join(candidateRoot, path);
    if (!existsSync(absolute)) {
      mkdirSync(dirname(absolute), { recursive: true });
      writeFileSync(absolute, bytes);
    }
    candidate.authorities.push({ role, contract, relative_path: path, sha256: sha(bytes), byte_count: bytes.length });
  }
  const inventory = inventoryFor(seed, candidate, {
    promptSha: receipt.prompt.sha256,
    inputArtifacts: receipt.inputs,
  });
  const inventoryBytes = Buffer.from(`${JSON.stringify(inventory, null, 2)}\n`);
  writeFileSync(join(candidateRoot, "evidence", "inventory.json"), inventoryBytes);
  candidate.authorities.push({ role: "evaluator-inventory", contract: inventory.contract, relative_path: "evidence/inventory.json", sha256: sha(inventoryBytes), byte_count: inventoryBytes.length });
  candidate.evaluator_inventory_sha256 = inventory.inventory_sha256;
  candidate.lifecycle_policy_sha256 = authorityHash(policy);
  const candidatePath = join(stage, "candidate.json");
  writeJson(candidatePath, candidate);
  const args = ["--json", "conformance", "compile", "--candidate", candidatePath, "--artifact-root", stage];
  const first = resultData(invoke(args), "first deterministic compile");
  const second = resultData(invoke(args), "second deterministic compile");
  assert.deepEqual(second, first);
  assert.equal(first.valid, true);
  assert.equal(first.status, "sufficient-for-job");
  assert.equal(first.behavioral_qualification_allowed, true);
  assert.deepEqual(first.assertions.map((item) => item.id), ["D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "D12"]);
  assert.ok(first.assertions.every((item) => item.status === "pass"));
  return {
    stage,
    candidatePath,
    first,
    candidate,
    inventory,
    policy,
    normalizationValidation,
    acceptedClaims,
    rejectedClaims,
    candidateSource: candidateRoot,
    promptBinding: { promptSha: receipt.prompt.sha256, inputArtifacts: receipt.inputs },
  };
}

const deterministicFixtures = new Map();

function compileNormalizationJob() {
  const stage = join(root, "compile-replay-prospect-fit-or-brief");
  const pack = join(stage, "pack");
  copyLegacyBasicPack(pack);
  const requirementsArgs = ["--json", "requirements", "--dir", pack, "--job", "prospect-fit-or-brief"];
  const firstRequirements = output(invoke(requirementsArgs), "normalization requirements compile 1");
  const secondRequirements = output(invoke(requirementsArgs), "normalization requirements compile 2");
  assert.deepEqual(secondRequirements, firstRequirements);
  assert.equal(firstRequirements.valid, true);
  assert.equal(firstRequirements.available, false);
  assert.equal(firstRequirements.status, "unavailable");
  assert.equal(firstRequirements.job.id, "prospect-fit-or-brief");
  assert.equal(firstRequirements.model_task, undefined);
  assert.match(JSON.stringify(firstRequirements.diagnostics), /decision_input_contract_not_bound/);

  const skills = output(invoke(["--json", "skills", "--dir", pack, "--job", "prospect-fit-or-brief"]), "normalization skills compile");
  assert.equal(skills.job_routes.length, 1);
  assert.equal(skills.job_routes[0].job_id, "prospect-fit-or-brief");
  assert.equal(skills.job_routes[0].model_task.status, "unassessed");
  const normalizedPath = join(stage, "normalized-prompt-output.json");
  cpSync(join(fixtureRoot, "normalized-prompt-output.json"), normalizedPath);
  const validationArgs = ["--json", "validate-prompt-output", "--dir", pack, "--file", normalizedPath, "--prompt-id", "normalize-prospect-row"];
  const firstValidation = output(invoke(validationArgs), "normalization prompt evaluation 1");
  const secondValidation = output(invoke(validationArgs), "normalization prompt evaluation 2");
  assert.deepEqual(secondValidation, firstValidation);
  assert.equal(firstValidation.valid, true);
  return { requirements: firstRequirements, skills, validation: firstValidation };
}

try {
  assert.ok(existsSync(mdp), `compiled CLI not found at ${mdp}`);

  record("installed discovery exposes closed conformance schemas and external-only model execution", () => {
    for (const target of ["conformance-candidate-v1", "model-invocation-evidence-v1", "conformance-verifier-receipt-v1", "evaluator-inventory-v1", "evaluator-result-v1", "private-record-policy-v1", "publication-approval-v1", "conformance-trial-v1", "job-conformance-v1", "conformance-report-v1", "public-conformance-report-v1", "deterministic-conformance-v1", "behavioral-evaluation-v1"]) {
      assert.ok(output(invoke(["--json", "schema", target]), `schema ${target}`).$schema);
    }
    const capabilities = output(invoke(["--json", "capabilities"]), "capabilities");
    assert.equal(capabilities.cold_model_conformance_contracts.model_execution, "external-only");
    const advertised = capabilities.commands.filter((command) => Array.isArray(command.argv) && command.argv[0] === "conformance");
    assert.deepEqual(advertised.map((command) => command.argv), [
      ["conformance", "compile"],
      ["conformance", "validate"],
      ["conformance", "assemble"],
      ["conformance", "report"],
    ]);
    for (const command of advertised) {
      const help = invoke([...command.argv, "--help"]);
      assert.equal(help.status, 0, `${command.argv.join(" ")} --help failed`);
      for (const flag of [...command.required_args, ...command.repeatable_args, ...command.optional_args]) {
        assert.match(help.stdout, new RegExp(`(^|\\s)${flag.replaceAll("-", "\\-")}(?=([ =<]|$))`, "m"), `${command.argv.join(" ")} does not expose ${flag}`);
      }
    }
  });

  record("deterministic candidate compilation replays byte-equivalent assertions", () => {
    deterministicFixtures.set("generation", compileReplay("outbound-copy-brief", "generation"));
    if (!smoke) deterministicFixtures.set("review", compileReplay("outbound-copy-review", "review"));
  });

  for (const seedName of smoke ? ["generation"] : ["normalization", "generation", "review"]) {
    const proofName = seedName === "normalization"
      ? "normalization job compiles and validates its real prompt output without claiming model qualification"
      : `${seedName} recorded synthetic trials enforce hard 3/3 and useful 2/3`;
    record(proofName, () => {
      if (seedName === "normalization") {
        const normalization = compileNormalizationJob();
        assert.equal(normalization.requirements.status, "unavailable");
        assert.equal(normalization.validation.valid, true);
        return;
      }
      const { evaluation } = validateEvidence(`positive-${seedName}`, buildEvidence(readSeed(seedName)));
      assert.equal(evaluation.behavioral_qualification, "qualified-for-job-under-envelope");
      assert.deepEqual(evaluation.behavioral_assertions.map((item) => item.id), ["B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B9"]);
      assert.equal(evaluation.behavioral_assertions.find((item) => item.id === "B1").passed_trials, 3);
      assert.ok(evaluation.behavioral_assertions.find((item) => item.id === "B6").passed_trials >= 2);
      assert.equal(evaluation.drafting_authority_granted, false);
    });
  }

  if (!smoke) {
    record("usefulness at 1/3 and a hard boundary at 2/3 fail qualification", () => {
      const weak = validateEvidence("weak-usefulness", buildEvidence(readSeed("generation"), { usefulPasses: 1 })).evaluation;
      assert.equal(weak.behavioral_qualification, "not-qualified-for-job-under-envelope");
      const hard = validateEvidence("hard-boundary", buildEvidence(readSeed("generation"), { hardFailure: 1 })).evaluation;
      assert.equal(hard.behavioral_qualification, "not-qualified-for-job-under-envelope");
    });

    record("expected bounded non-success counts without usable output", () => {
      const evaluation = validateEvidence("bounded-non-success", buildEvidence(readSeed("bounded-non-success"))).evaluation;
      assert.equal(evaluation.behavioral_qualification, "qualified-for-job-under-envelope");
      assert.ok(evaluation.trials.every((trial) => trial.status === "bounded-non-success-confirmed" && trial.usable_output === false));
    });

    record("replay, isolation, freshness, oracle, identity, and lifecycle mutations fail closed", () => {
      const mutations = [
        ["replay", { mutateInvocation(value, index, prior) { if (index === 1) value.freshness.session_id = prior[0].freshness.session_id; } }, /trial-replay-or-identity-reuse/],
        ["isolation", { mutateInvocation(value, index) { if (index === 0) { value.isolation[0].state = "unknown"; value.isolation[0].provenance = "unknown"; value.isolation[0].verifier_receipt_sha256 = null; } } }, /cold-isolation-unproven/],
        ["freshness", { mutateInvocation(value, index) { if (index === 0) value.freshness.resumed = true; } }, /resumed sessions/],
        ["oracle", { mutateInvocation(value, index) { if (index === 0) { value.input_artifacts[0].name = "evaluator-rubric"; value.model_visible_context_sha256 = domainHash("mdp.model-visible-context.v1", value.input_artifacts); } } }, /oracle-leak/],
        ["identity", { mutateInvocation(value, index) { if (index === 0) value.job_id = "other-job"; } }, /fresh-host-binding-not-verified/],
        ["wrong-prompt", { mutateInvocation(value, index) { if (index === 0) value.prompt_sha256 = "f".repeat(64); } }, /model-visible-context-oracle-leak-or-hash-mismatch/],
        ["wrong-requested-model", { mutateInvocation(value, index) { if (index === 0) value.requested_model = "undeclared-requested-model"; } }, /model-visible-context-oracle-leak-or-hash-mismatch|model/],
        ["wrong-resolved-model", { mutateInvocation(value, index) { if (index === 0) value.resolved_model = "undeclared-resolved-model"; } }, /model-visible-context-oracle-leak-or-hash-mismatch|model/],
        ["wrong-phase", { mutateInvocation(value, index) { if (index === 0) value.phase = "review"; } }, /model-visible-context-oracle-leak-or-hash-mismatch/],
        ["wrong-slot", { mutateInvocation(value, index) { if (index === 0) value.trial_id = "undeclared-trial-slot"; } }, /behavioral evaluation aggregate contradicts assertion results|required-sampling-incomplete|model-visible-context-oracle-leak-or-hash-mismatch|invocation-missing/],
        ["missing-verifier", { mutateInvocation(value, index) { if (index === 0) value.freshness.verifier_receipt_sha256 = "f".repeat(64); } }, /fresh-host-binding-not-verified|verifier/],
        ["untrusted-verifier", { mutateVerifierReceipt(value, index) { if (index === 0) value.identity_authority_sha256 = "f".repeat(64); } }, /trusted|verifier|authority/],
        ["forged-verifier-signature", { mutateVerifierReceiptAfterSign(value, index) { if (index === 0) value.signature_hex = "f".repeat(128); } }, /signature|verifier|fresh-host-binding-not-verified/],
        ["tampered-verifier-payload", { mutateVerifierReceiptAfterSign(value, index) { if (index === 0) value.receipt_id = "tampered-after-signing"; } }, /signature|verifier|fresh-host-binding-not-verified/],
        ["fractional-timestamp", { mutateInvocation(value, index) { if (index === 0) value.started_at = "2026-08-13T12:00:00.000Z"; } }, /timestamp|UTC/],
        ["lifecycle", { mutateInvocation(value, index) { if (index === 0) value.output.lifecycle_policy_sha256 = "f".repeat(64); } }, /output-lifecycle-policy-mismatch/],
      ];
      for (const [name, options, pattern] of mutations) {
        const paths = stageEvidence(`mutation-${name}`, buildEvidence(readSeed("generation"), options));
        const attempted = invoke(validateArgs(paths));
        if (attempted.status === 0) assert.match(JSON.stringify(output(attempted, name)), pattern);
        else assert.match(`${attempted.stdout}\n${attempted.stderr}`, pattern);
      }

      const unrelatedPrompt = buildEvidence(readSeed("generation"), {
        inventoryBinding: { promptSha: "f".repeat(64) },
      });
      expectFail(
        invoke(validateArgs(stageEvidence("candidate-slot-prompt-forgery", unrelatedPrompt))),
        "candidate slot prompt forgery",
        /candidate|prompt|authority|inventory/,
      );
    });

    record("challenge provenance, injection, adjudication, and exact-hash privacy mutations fail closed", () => {
      const challenge = buildEvidence(readSeed("generation"));
      challenge.inventory.challenges[0].model_visible = true;
      challenge.inventory.inventory_sha256 = "";
      challenge.inventory.inventory_sha256 = domainHash(challenge.inventory.contract, challenge.inventory);
      expectFail(invoke(validateArgs(stageEvidence("challenge-forgery", challenge))), "challenge provenance", /protected|model context/);

      const injected = buildEvidence(readSeed("generation"));
      injected.candidate.expected_result = "pass";
      expectFail(invoke(validateArgs(stageEvidence("candidate-injection", injected))), "candidate injection", /unknown field|expected_result/);

      const disputed = buildEvidence(readSeed("generation"), { mutateResult(result, index) {
        if (index !== 0) return;
        result.scores[0].status = "disputed";
        result.competing_score_sha256s = ["a".repeat(64), "b".repeat(64)];
        result.disagreement = "resolved";
        result.adjudication = { adjudicator_name: "Release Author", reviewer_role: "release-approver", identity_authority_ref: "review:1", approval_receipt_sha256: "c".repeat(64), output_sha256: result.output_sha256, competing_score_sha256s: result.competing_score_sha256s, decision: "pass", purpose: "resolve-hard-boundary", approved_at: "2026-08-13T13:00:00Z" };
      } });
      expectFail(invoke(validateArgs(stageEvidence("bad-adjudication", disputed))), "bad adjudication", /independent customer adjudicator/);

      const privateApproval = buildEvidence(readSeed("generation"), { accessClass: "sanitized-public" });
      const privatePaths = stageEvidence("missing-public-approval", privateApproval);
      const privacy = resultData(invoke(validateArgs(privatePaths)), "missing public approval");
      assert.equal(privacy.behavioral_qualification, "not-qualified-for-job-under-envelope");
      assert.match(JSON.stringify(privacy), /sanitized-public-exact-hash-approval-missing/);

      const trustedApproval = addPublicationApprovals(
        buildEvidence(readSeed("generation"), { accessClass: "sanitized-public" }),
        TRUSTED_PUBLICATION_AUTHORITY.identity_authority_sha256,
      );
      const trustedPrivacy = resultData(
        invoke(validateArgs(stageEvidence("trusted-public-approval", trustedApproval))),
        "trusted public approval",
      );
      assert.equal(trustedPrivacy.behavioral_qualification, "qualified-for-job-under-envelope");

      const untrustedApproval = addPublicationApprovals(
        buildEvidence(readSeed("generation"), { accessClass: "sanitized-public" }),
        "f".repeat(64),
      );
      const untrustedPrivacy = resultData(
        invoke(validateArgs(stageEvidence("untrusted-public-approval", untrustedApproval))),
        "untrusted public approval",
      );
      assert.equal(untrustedPrivacy.behavioral_qualification, "not-qualified-for-job-under-envelope");
      assert.match(JSON.stringify(untrustedPrivacy), /sanitized-public-exact-hash-approval-missing/);

      for (const [name, mutate] of [
        ["forged-public-approval-signature", (approval, index) => { if (index === 0) approval.signature_hex = "f".repeat(128); }],
        ["tampered-public-approval-payload", (approval, index) => { if (index === 0) approval.purpose = "tampered-after-signing"; }],
      ]) {
        const forgedApproval = addPublicationApprovals(
          buildEvidence(readSeed("generation"), { accessClass: "sanitized-public" }),
          TRUSTED_PUBLICATION_AUTHORITY.identity_authority_sha256,
          mutate,
        );
        const forgedPrivacy = resultData(
          invoke(validateArgs(stageEvidence(name, forgedApproval))),
          name,
        );
        assert.equal(forgedPrivacy.behavioral_qualification, "not-qualified-for-job-under-envelope");
        assert.match(JSON.stringify(forgedPrivacy), /sanitized-public-exact-hash-approval-missing/);
      }
    });

    record("containment and resource limits reject traversal, links, oversized, deep, and amplified inputs", () => {
      const deterministicFixture = deterministicFixtures.get("generation");
      const evidence = buildEvidence(readSeed("generation"));
      const paths = stageEvidence("resource-baseline", evidence);
      const outside = join(root, "outside-candidate.json");
      writeJson(outside, evidence.candidate);
      expectFail(invoke(["--json", "conformance", "compile", "--candidate", outside, "--artifact-root", paths.stage]), "candidate traversal", /escapes staged artifact root|must be named beneath staged artifact root/);
      const linked = join(paths.stage, "linked-candidate.json");
      symlinkSync(outside, linked);
      expectFail(invoke(["--json", "conformance", "compile", "--candidate", linked, "--artifact-root", paths.stage]), "candidate symlink", /regular non-symlink|symbolic links|contained authority component safely/);
      const hard = join(paths.stage, "hard-candidate.json");
      linkSync(paths.candidate, hard);
      expectFail(invoke(["--json", "conformance", "compile", "--candidate", hard, "--artifact-root", paths.stage]), "candidate hardlink", /hard linked/);
      unlinkSync(hard);
      const huge = join(paths.stage, "huge.json");
      writeFileSync(huge, " ".repeat(1_048_577));
      const hugeArgs = validateArgs({ ...paths, candidate: huge });
      expectFail(invoke(hugeArgs), "oversized authority", /exceeds 1048576 byte limit/);
      const deep = join(paths.stage, "deep.json");
      writeJson(deep, { nested: Array.from({ length: 40 }).reduce((value) => [value], "end") });
      expectFail(invoke(validateArgs({ ...paths, candidate: deep })), "deep authority", /depth|missing field/);
      const amplified = structuredClone(evidence.invocations[0]);
      amplified.input_artifacts = Array.from({ length: 65 }, (_, index) => ({ name: `input-${index}`, sha256: HASH.source }));
      writeJson(paths.invocations[0], amplified);
      expectFail(invoke(validateArgs(paths)), "amplified context", /too many model-visible inputs/);

      const forged = structuredClone(deterministicFixture.candidate);
      forged.authorities.find((authority) => authority.role === "requirements").contract = "mdp.forged-contract.v1";
      const forgedPath = join(deterministicFixture.stage, "forged-candidate.json");
      writeJson(forgedPath, forged);
      expectFail(invoke(["--json", "conformance", "compile", "--candidate", forgedPath, "--artifact-root", deterministicFixture.stage]), "authority role/contract forgery", /contract discriminator|contract/);
    });

    let compositeStage;
    record("composite authority, public/private reports, and JSON/Mermaid traces replay deterministically", () => {
      const deterministicFixture = deterministicFixtures.get("generation");
      compositeStage = stageComposite("composite-source", deterministicFixture);
      const traceArgs = ["--json", "trace", "--file", "job-conformance.json", "--artifact-root", compositeStage.stage];
      assert.deepEqual(output(invoke(traceArgs), "trace replay 1"), output(invoke(traceArgs), "trace replay 2"));
      const mermaid1 = output(invoke([...traceArgs, "--format", "mermaid"]), "mermaid replay 1");
      const mermaid2 = output(invoke([...traceArgs, "--format", "mermaid"]), "mermaid replay 2");
      assert.deepEqual(mermaid2, mermaid1);
      const publicReport = output(invoke(["--json", "conformance", "report", "--conformance", "job-conformance.json", "--artifact-root", compositeStage.stage, "--visibility", "public", "--generated-at", "2026-08-13T14:00:00Z"]), "public report");
      assert.equal(publicReport.contract, "mdp.public-conformance-report.v1");
      const privateReport = output(invoke(["--json", "conformance", "report", "--conformance", "job-conformance.json", "--artifact-root", compositeStage.stage, "--visibility", "private", "--generated-at", "2026-08-13T14:00:00Z"]), "private report");
      assert.equal(privateReport.contract, "mdp.conformance-report.v1");
    });

    record("composite tampering, missing links, cycles, privacy leakage, and fan-out fail closed", () => {
      const deterministicFixture = deterministicFixtures.get("generation");
      const stage = compositeStage.stage;
      const original = readFileSync(join(stage, "deterministic.json"));
      writeFileSync(join(stage, "deterministic.json"), "{}\n");
      expectFail(invoke(["--json", "conformance", "report", "--conformance", "job-conformance.json", "--artifact-root", stage, "--visibility", "private", "--generated-at", "2026-08-13T14:00:00Z"]), "member tamper");
      writeFileSync(join(stage, "deterministic.json"), original);

      const missing = structuredClone(compositeStage.composite);
      missing.journey.links = missing.journey.links.filter((link) => !(link.relation === "bound-to" && link.from_artifact_id === "deterministic-evaluation"));
      writeJson(join(stage, "missing-link.json"), missing);
      expectFail(invoke(["--json", "trace", "--file", "missing-link.json", "--artifact-root", stage]), "missing link", /chain is incomplete/);

      const cyclic = structuredClone(compositeStage.composite);
      cyclic.journey.links.push({ from_artifact_id: "behavioral-evaluation", to_artifact_id: "deterministic-evaluation", relation: "bound-to" });
      writeJson(join(stage, "cyclic.json"), cyclic);
      expectFail(invoke(["--json", "trace", "--file", "cyclic.json", "--artifact-root", stage]), "cyclic graph", /acyclic/);

      const privateComposite = structuredClone(compositeStage.composite);
      privateComposite.journey.artifacts.forEach((artifact) => { artifact.access_class = "private"; artifact.publication_approval_sha256 = null; });
      writeJson(join(stage, "private.json"), privateComposite);
      expectFail(invoke(["--json", "conformance", "report", "--conformance", "private.json", "--artifact-root", stage, "--visibility", "public", "--generated-at", "2026-08-13T14:00:00Z"]), "synthetic-to-private relabel", /access classification/);

      const privateToSynthetic = structuredClone(privateComposite);
      privateToSynthetic.journey.artifacts[0].access_class = "synthetic";
      writeJson(join(stage, "private-to-synthetic.json"), privateToSynthetic);
      expectFail(invoke(["--json", "conformance", "report", "--conformance", "private-to-synthetic.json", "--artifact-root", stage, "--visibility", "public", "--generated-at", "2026-08-13T14:00:00Z"]), "private-to-synthetic relabel", /access classification/);

      const leakedTrialId = structuredClone(compositeStage.composite);
      leakedTrialId.limitations = [compositeStage.evidence.trials[0].trial_id];
      writeJson(join(stage, "trial-id-limitation.json"), leakedTrialId);
      expectFail(invoke(["--json", "conformance", "report", "--conformance", "trial-id-limitation.json", "--artifact-root", stage, "--visibility", "public", "--generated-at", "2026-08-13T14:00:00Z"]), "trial id limitation leak", /unsafe|unknown public reason|limitation/);

      const mismatchedDeterministic = structuredClone(deterministicFixture.first);
      mismatchedDeterministic.assertions[0].status = "fail";
      mismatchedDeterministic.assertions[0].reason_codes = ["synthetic-d1-fail"];
      mismatchedDeterministic.summary = { passed: 11, failed: 1, unassessed: 0 };
      mismatchedDeterministic.status = "not-sufficient-for-job";
      mismatchedDeterministic.valid = false;
      mismatchedDeterministic.behavioral_qualification_allowed = false;
      writeJson(join(stage, "mismatched-deterministic.json"), mismatchedDeterministic);
      const mismatchedBehavioral = structuredClone(compositeStage.evaluation);
      mismatchedBehavioral.deterministic_evaluation_sha256 = authorityHash(mismatchedDeterministic);
      writeJson(join(stage, "mismatched-behavioral.json"), mismatchedBehavioral);
      const mismatchArgs = ["--json", "conformance", "assemble", "--candidate", "candidate.json", "--deterministic", "mismatched-deterministic.json", "--behavioral", "mismatched-behavioral.json", "--artifact-root", stage];
      compositeStage.evidence.trials.forEach((_, index) => mismatchArgs.push("--trial", `trial-${index + 1}.json`));
      expectFail(invoke(mismatchArgs), "deterministic status mismatch", /deterministic evaluation does not equal authoritative staged compilation|top-level fields|status/);

      const amplified = structuredClone(compositeStage.composite);
      while (amplified.journey.links.length <= 64) amplified.journey.links.push({ from_artifact_id: "candidate", to_artifact_id: "deterministic-evaluation", relation: "declares" });
      writeJson(join(stage, "amplified.json"), amplified);
      expectFail(invoke(["--json", "trace", "--file", "amplified.json", "--artifact-root", stage]), "composite fanout", /fan-out exceeds limit/);
    });
  }
} finally {
  if (keep) process.stderr.write(`kept artifacts at ${root}\n`);
  else rmSync(root, { recursive: true, force: true });
}

const failed = results.filter((item) => item.status === "fail");
assert.ok(observedMdpCommands.size > 0, "harness must exercise at least one allowlisted MDP command");
for (const providerCommand of ["model", "provider", "native-normalize-openai", "mcp"]) {
  assert.equal(allowedMdpCommands.has(providerCommand), false, `${providerCommand} must remain outside the harness command allowlist`);
  assert.equal(observedMdpCommands.has(providerCommand), false, `${providerCommand} must not be observed`);
}
process.stdout.write(`1..${results.length}\n`);
process.stdout.write(`${results.length - failed.length} passed; ${failed.length} failed; mode=${smoke ? "smoke" : "full"}; provider_command_allowlist=enforced; provider_command_execution=blocked; network_isolation=unobserved\n`);
if (failed.length) process.exitCode = 1;
