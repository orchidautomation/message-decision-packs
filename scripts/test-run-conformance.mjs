#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  linkSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const mdp = process.env.MDP_BIN || join(repoRoot, "cli", "target", "debug", "mdp");
const keep = process.argv.includes("--keep");
const root = mkdtempSync(join(tmpdir(), "mdp-run-conformance-"));
const pack = join(root, "pack");
const gtmPack = join(root, "gtm-pack");
const validOutput = join(root, "valid-output.json");
const invalidOutput = join(root, "invalid-output.json");
const gtmDisqualifiedNormalized = join(root, "gtm-disqualified-normalized.json");
const gtmDisqualifiedResults = join(root, "gtm-disqualified-results.json");
const results = [];

function invoke(args, options = {}) {
  return spawnSync(mdp, args, {
    cwd: options.cwd || repoRoot,
    encoding: "utf8",
    maxBuffer: options.maxBuffer || 16 * 1024 * 1024,
    env: { PATH: process.env.PATH || "", MDP_CONFORMANCE_SENTINEL: "not-declared" },
  });
}

function record(name, fn, coverage = { profile: "proposal", adapter: "direct-cli" }) {
  try {
    fn();
    results.push({ name, status: "pass", ...coverage });
    process.stdout.write(`ok ${results.length} - ${name}\n`);
  } catch (error) {
    results.push({ name, status: "fail", error: error.message, ...coverage });
    process.stderr.write(`not ok ${results.length} - ${name}\n${error.stack}\n`);
  }
}

function expectOk(result, label) {
  assert.equal(
    result.status,
    0,
    `${label} failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  return JSON.parse(result.stdout).data;
}

function expectFail(result, label, pattern) {
  assert.notEqual(result.status, 0, `${label} unexpectedly succeeded: ${result.stdout}`);
  if (pattern) {
    assert.match(`${result.stdout}\n${result.stderr}`, pattern, `${label} did not report the expected refusal`);
  }
}

function expectInvalidJson(result, label) {
  assert.notEqual(result.status, 0, `${label} unexpectedly returned a valid exit status`);
  assert.ok(result.stdout.trim(), `${label} returned no JSON result`);
  return JSON.parse(result.stdout).data;
}

function expectPreflightRefusal(result, label, reasonCode) {
  const data = expectInvalidJson(result, label);
  assert.equal(data.contract, "mdp.run-execution.v1");
  assert.equal(data.valid, false);
  assert.equal(data.terminal_state, "no-draft:preflight-refused");
  assert.equal(data.run_dir, null);
  assert.equal(data.bundle_sha256, null);
  assert.equal(data.receipt_sha256, null);
  if (reasonCode) {
    assert.ok(
      data.authority_block.reason_codes.includes(reasonCode),
      `${label} omitted sanitized reason code ${reasonCode}: ${result.stdout}`,
    );
  }
  return data;
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
}

function request(executionId, sourcePath = validOutput) {
  return {
    contract: "mdp.run-request.v1",
    execution_id: executionId,
    created_at: "2026-08-03T00:00:00Z",
    profile: "proposal",
    operation: "validate-existing-output",
    mode: "deterministic",
    job_identity: null,
    pack_dir: pack,
    pack_release_id: "synthetic-proposal-release-1",
    prompt: null,
    inputs: [
      {
        logical_name: "prompt-output",
        source_path: sourcePath,
        schema_id: "mdp.prompt-output.v0",
        media_type: "application/json",
        provenance_refs: [],
      },
    ],
    execution_policy: {
      environment_allowlist: [],
      filesystem_mode: "private-staging",
      tool_mode: "none",
      network_mode: "none",
      authorized_endpoints: [],
      max_input_bytes: 1048576,
      max_output_bytes: 1048576,
      timeout_ms: 30000,
      retention_policy: "receipt-only",
    },
    driver: null,
    model: null,
  };
}

function gtmRequest(
  executionId,
  normalizedPath = join(gtmPack, "fixtures", "normalized-response-ready.json"),
  resultsPath = join(gtmPack, "fixtures", "collected-attempt-results.json"),
  packRoot = gtmPack,
) {
  const value = request(executionId, normalizedPath);
  value.profile = "gtm";
  value.operation = "qualify";
  value.pack_dir = packRoot;
  value.pack_release_id = "synthetic-gtm-release-1";
  value.job_identity = { job_id: "prospect-fit-or-brief", idempotency_key: `${executionId}-v1` };
  value.inputs = [
    { logical_name: "normalized-decision-input", source_path: normalizedPath, schema_id: "mdp.normalized-decision-input.v2", media_type: "application/json", provenance_refs: [] },
    { logical_name: "source-binding", source_path: join(gtmPack, "fixtures", "source-binding-clay-adapter.json"), schema_id: "mdp.source-binding.v2", media_type: "application/json", provenance_refs: [] },
    { logical_name: "source-attempt-request", source_path: join(gtmPack, "fixtures", "source-attempt-request.json"), schema_id: "mdp.source-attempt-request.v2", media_type: "application/json", provenance_refs: [] },
    { logical_name: "collected-attempt-results", source_path: resultsPath, schema_id: "mdp.collected-attempt-results.v2", media_type: "application/json", provenance_refs: [] },
    { logical_name: "bound-prompt", source_path: join(gtmPack, ".mdp", "prompts", "normalize-prospect.yaml"), schema_id: "mdp.prompt.v0", media_type: "application/yaml", provenance_refs: [] },
  ];
  return value;
}

function runRequest(value, label) {
  const requestPath = join(root, `${label}.request.json`);
  const outDir = join(root, `${label}.run`);
  writeJson(requestPath, value);
  return { requestPath, outDir, result: invoke(["--json", "run", "--request", requestPath, "--out-dir", outDir]) };
}

function verify(runDir) {
  return invoke([
    "--json",
    "verify-run",
    "--bundle",
    join(runDir, "run-bundle.json"),
    "--receipt",
    join(runDir, "run-receipt.json"),
    "--artifact-root",
    runDir,
  ]);
}

function consume(ledger, job, key, receipt, prior, permitReplay = false) {
  const args = [
    "--json",
    "consume-run",
    "--ledger",
    ledger,
    "--job-id",
    job,
    "--idempotency-key",
    key,
    "--receipt-sha256",
    receipt,
    "--expected-prior-version",
    String(prior),
  ];
  if (permitReplay) args.push("--permit-exact-replay");
  return invoke(args);
}

try {
  assert.ok(existsSync(mdp), `compiled CLI not found at ${mdp}; run cargo build --manifest-path cli/Cargo.toml`);
  cpSync(join(repoRoot, "plugin", "assets", "templates", "proposal"), pack, { recursive: true });
  cpSync(join(repoRoot, "examples", "clay-audiences-self-serve-enterprise-expansion"), gtmPack, { recursive: true });
  copyFileSync(join(repoRoot, "examples", "proposal-flow-video", "fixtures", "normalize-opportunity-output.json"), validOutput);
  writeJson(invalidOutput, { contract: "mdp.prompt-output.v0", prompt_id: "normalize-opportunity" });
  const disqualifiedResults = JSON.parse(readFileSync(join(gtmPack, "fixtures", "collected-attempt-results.json"), "utf8"));
  disqualifiedResults.attributes.enterprise_eligibility.value = "ineligible";
  disqualifiedResults.attempt_results.find((item) => item.attribute_id === "enterprise_eligibility").value = "ineligible";
  writeJson(gtmDisqualifiedResults, disqualifiedResults);
  const disqualifiedNormalized = JSON.parse(readFileSync(join(gtmPack, "fixtures", "normalized-response-ready.json"), "utf8"));
  disqualifiedNormalized.attributes.enterprise_eligibility.value = "ineligible";
  disqualifiedNormalized.normalized_prospect.attributes.enterprise_eligibility = "ineligible";
  disqualifiedNormalized.signal_observations.find((item) => item.projection_id === "account-fit").value = "ineligible";
  disqualifiedNormalized.outcome = "disqualified";
  disqualifiedNormalized.collected_attempt_results_sha256 = createHash("sha256")
    .update(readFileSync(gtmDisqualifiedResults))
    .digest("hex");
  for (const observation of disqualifiedNormalized.signal_observations) {
    observation.receipt.collected_results_sha256 = disqualifiedNormalized.collected_attempt_results_sha256;
  }
  writeJson(gtmDisqualifiedNormalized, disqualifiedNormalized);

  let baseline;
  record("valid declared-input run publishes a verifiable immutable transaction", () => {
    baseline = runRequest(request("conformance-success"), "success");
    const run = expectOk(baseline.result, "baseline run");
    assert.equal(run.terminal_state, "success");
    assert.equal(expectOk(verify(baseline.outDir), "baseline verification").valid, true);
    assert.equal(existsSync(join(baseline.outDir, "private")), false);
  });

  record("valid GTM qualification publishes a verifiable immutable transaction", () => {
    const attempted = runRequest(
      gtmRequest(
        "gtm-success",
        join(gtmPack, "fixtures", "normalized-response-ready.json"),
        join(gtmPack, "fixtures", "collected-attempt-results.json"),
        gtmPack,
      ),
      "gtm-success",
    );
    const run = expectOk(attempted.result, "GTM baseline run");
    assert.equal(run.terminal_state, "success");
    const receipt = JSON.parse(readFileSync(join(attempted.outDir, "run-receipt.json"), "utf8"));
    assert.equal(receipt.decision.decision, "qualified");
    assert.deepEqual(receipt.decision.reason_codes, ["ready"]);
    const context = JSON.parse(readFileSync(join(attempted.outDir, "artifacts", "compiled-context.json"), "utf8"));
    assert.equal(context.qualification.status, "fit");
    assert.equal(context.drafting_authority, "not-granted");
    assert.equal(expectOk(verify(attempted.outDir), "GTM baseline verification").valid, true);
  }, { profile: "gtm", adapter: "direct-cli" });

  record("GTM disqualifying normalized evidence returns a verified no-draft transaction", () => {
    const attempted = runRequest(
      gtmRequest("gtm-disqualified", gtmDisqualifiedNormalized, gtmDisqualifiedResults),
      "gtm-disqualified",
    );
    const run = expectOk(attempted.result, "GTM disqualified run");
    assert.equal(run.terminal_state, "success");
    const receipt = JSON.parse(readFileSync(join(attempted.outDir, "run-receipt.json"), "utf8"));
    assert.equal(receipt.decision.decision, "no-draft");
    assert.deepEqual(receipt.decision.reason_codes, ["disqualified"]);
    const context = JSON.parse(readFileSync(join(attempted.outDir, "artifacts", "compiled-context.json"), "utf8"));
    const output = JSON.parse(readFileSync(join(attempted.outDir, "artifacts", "output.json"), "utf8"));
    assert.equal(output.status, "disqualified");
    assert.equal(context.qualification.status, "disqualified");
    assert.equal(context.drafting_authority, "not-granted");
    assert.equal(expectOk(verify(attempted.outDir), "GTM disqualified verification").valid, true);
  }, { profile: "gtm", adapter: "direct-cli" });

  record("unified stdio MCP preserves GTM qualified and disqualified branches", () => {
    const qualifiedRequest = join(root, "mcp-gtm-qualified.request.json");
    const disqualifiedRequest = join(root, "mcp-gtm-disqualified.request.json");
    const qualifiedOut = join(root, "mcp-gtm-qualified.run");
    const disqualifiedOut = join(root, "mcp-gtm-disqualified.run");
    writeJson(
      qualifiedRequest,
      gtmRequest(
        "mcp-gtm-qualified",
        join(gtmPack, "fixtures", "normalized-response-ready.json"),
        join(gtmPack, "fixtures", "collected-attempt-results.json"),
        gtmPack,
      ),
    );
    writeJson(
      disqualifiedRequest,
      gtmRequest("mcp-gtm-disqualified", gtmDisqualifiedNormalized, gtmDisqualifiedResults),
    );
    const call = (id, name, args) => JSON.stringify({
      jsonrpc: "2.0", id, method: "tools/call", params: { name, arguments: args },
    });
    const input = [
      call(1, "mdp_run", { request_path: qualifiedRequest, output_dir: qualifiedOut }),
      call(2, "mdp_verify_run", { bundle_path: join(qualifiedOut, "run-bundle.json"), receipt_path: join(qualifiedOut, "run-receipt.json"), artifact_root: qualifiedOut }),
      call(3, "mdp_run", { request_path: disqualifiedRequest, output_dir: disqualifiedOut }),
      call(4, "mdp_verify_run", { bundle_path: join(disqualifiedOut, "run-bundle.json"), receipt_path: join(disqualifiedOut, "run-receipt.json"), artifact_root: disqualifiedOut }),
    ].join("\n");
    const invoked = spawnSync(process.execPath, [join(repoRoot, "scripts", "mdp-run-mcp-server.mjs")], {
      cwd: root, input: `${input}\n`, encoding: "utf8",
      env: { PATH: process.env.PATH || "", MDP_BIN: mdp }, maxBuffer: 16 * 1024 * 1024,
    });
    assert.equal(invoked.status, 0, invoked.stderr);
    const replies = invoked.stdout.trim().split("\n").map((line) => JSON.parse(line));
    assert.equal(replies[0].result.structuredContent.terminal_state, "success");
    assert.equal(replies[1].result.structuredContent.valid, true);
    assert.equal(replies[2].result.structuredContent.terminal_state, "success");
    assert.equal(replies[3].result.structuredContent.valid, true);
    assert.equal(JSON.parse(readFileSync(join(qualifiedOut, "run-receipt.json"), "utf8")).decision.decision, "qualified");
    const disqualifiedReceipt = JSON.parse(readFileSync(join(disqualifiedOut, "run-receipt.json"), "utf8"));
    assert.equal(disqualifiedReceipt.decision.decision, "no-draft");
    assert.deepEqual(disqualifiedReceipt.decision.reason_codes, ["disqualified"]);
  }, { profile: "gtm", adapter: "unified-stdio-mcp" });

  record("GTM rejects undeclared ambient fields before publication", () => {
    const value = gtmRequest("gtm-ambient");
    value.hidden_clay_row = { private_column: "must-not-cross" };
    const attempted = runRequest(value, "gtm-ambient");
    expectPreflightRefusal(attempted.result, "GTM ambient field", "request-invalid");
    assert.equal(existsSync(attempted.outDir), false);
  }, { profile: "gtm", adapter: "direct-cli" });

  record("GTM wrong-contract normalized input produces no draft authority", () => {
    const wrong = join(root, "gtm-wrong-contract.json");
    writeJson(wrong, { contract: "mdp.prompt-output.v0", prompt_id: "normalize-opportunity" });
    const attempted = runRequest(gtmRequest("gtm-wrong-contract", wrong), "gtm-wrong-contract");
    expectFail(attempted.result, "GTM wrong-contract input");
    const run = JSON.parse(attempted.result.stdout).data;
    assert.match(run.terminal_state, /^no-draft:/);
    if (existsSync(join(attempted.outDir, "run-receipt.json"))) {
      assert.equal(expectOk(verify(attempted.outDir), "GTM no-draft verification").valid, true);
    }
    assert.equal(existsSync(join(attempted.outDir, "artifacts", "output.json")), false);
  }, { profile: "gtm", adapter: "direct-cli" });

  record("unified stdio MCP delegates to the real CLI without changing authority", () => {
    const requestPath = join(root, "mcp-real.request.json");
    const outDir = join(root, "mcp-real.run");
    writeJson(requestPath, request("mcp-real"));
    const runMessage = JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: {
      name: "mdp_run", arguments: { request_path: requestPath, output_dir: outDir },
    } });
    const verifyMessage = JSON.stringify({ jsonrpc: "2.0", id: 2, method: "tools/call", params: {
      name: "mdp_verify_run", arguments: {
        bundle_path: join(outDir, "run-bundle.json"),
        receipt_path: join(outDir, "run-receipt.json"),
        artifact_root: outDir,
      },
    } });
    const invoked = spawnSync(process.execPath, [join(repoRoot, "scripts", "mdp-run-mcp-server.mjs")], {
      cwd: root,
      input: `${runMessage}\n${verifyMessage}\n`,
      encoding: "utf8",
      env: { PATH: process.env.PATH || "", MDP_BIN: mdp },
      maxBuffer: 16 * 1024 * 1024,
    });
    assert.equal(invoked.status, 0, invoked.stderr);
    const [reply, verificationReply] = invoked.stdout.trim().split("\n").map((line) => JSON.parse(line));
    assert.equal(reply.result.isError, false);
    assert.equal(reply.result.structuredContent.terminal_state, "success");
    assert.equal(verificationReply.result.isError, false);
    assert.equal(verificationReply.result.structuredContent.valid, true);
    const receipt = JSON.parse(readFileSync(join(outDir, "run-receipt.json"), "utf8"));
    assert.equal(reply.result.structuredContent.authority_block.execution_id, receipt.execution_id);
    assert.equal(reply.result.structuredContent.receipt_sha256, receipt.receipt_sha256);
  }, { profile: "proposal", adapter: "unified-stdio-mcp" });

  record("undeclared ambient fields and hidden-input smuggling are rejected", () => {
    const value = request("undeclared-field");
    value.ambient_context = { secret: "must-not-enter-authority" };
    const attempted = runRequest(value, "undeclared-field");
    expectPreflightRefusal(attempted.result, "undeclared authority", "request-invalid");
    assert.equal(existsSync(attempted.outDir), false);
  });

  record("an adjacent undeclared file is absent from all published authority", () => {
    const sentinel = "MDP_UNDECLARED_SENTINEL_8ce861dd";
    writeFileSync(join(root, "ambient-secret.txt"), sentinel);
    const attempted = runRequest(request("adjacent-hidden"), "adjacent-hidden");
    expectOk(attempted.result, "adjacent hidden input run");
    const published = [
      "run-bundle.json",
      "run-receipt.json",
      "runner-audit.json",
      "artifacts/output.json",
      "artifacts/compiled-context.json",
      "artifacts/validation.json",
    ].map((name) => readFileSync(join(attempted.outDir, name), "utf8")).join("\n");
    assert.equal(published.includes(sentinel), false);
    assert.equal(published.includes("MDP_CONFORMANCE_SENTINEL"), false);
  });

  if (process.platform !== "win32") {
    record("declared input symlinks are refused before publication", () => {
      const linked = join(root, "symlink-output.json");
      symlinkSync(validOutput, linked);
      const attempted = runRequest(request("symlink-input", linked), "symlink-input");
      const data = expectInvalidJson(attempted.result, "symlink input");
      assert.match(data.terminal_state, /^no-draft:(preflight-refused|policy-blocked)$/);
      assert.ok(data.authority_block.reason_codes.includes("declared-input-refused"));
      assert.equal(existsSync(join(attempted.outDir, "artifacts", "output.json")), false);
    });

    record("declared input hard links are refused before publication", () => {
      const source = join(root, "hardlink-source.json");
      const linked = join(root, "hardlink-output.json");
      copyFileSync(validOutput, source);
      linkSync(source, linked);
      try {
        const attempted = runRequest(request("hardlink-input", linked), "hardlink-input");
        const data = expectInvalidJson(attempted.result, "hard-link input");
        assert.match(data.terminal_state, /^no-draft:(preflight-refused|policy-blocked)$/);
        assert.ok(data.authority_block.reason_codes.includes("declared-input-refused"));
        assert.equal(existsSync(join(attempted.outDir, "artifacts", "output.json")), false);
      } finally {
        rmSync(linked, { force: true });
        rmSync(source, { force: true });
      }
    });
  }

  record("logical-name path escape is rejected", () => {
    const value = request("path-escape");
    value.inputs[0].logical_name = "../prompt-output";
    const attempted = runRequest(value, "path-escape");
    expectPreflightRefusal(attempted.result, "logical path escape", "request-policy-invalid");
    assert.equal(existsSync(join(root, "prompt-output")), false);
  });

  record("malformed, duplicate-member, and oversized authority JSON fail closed", () => {
    const malformed = join(root, "malformed.request.json");
    writeFileSync(malformed, "{\"contract\":\n");
    expectPreflightRefusal(invoke(["--json", "run", "--request", malformed, "--out-dir", join(root, "malformed.run")]), "malformed JSON", "request-invalid");

    const duplicate = join(root, "duplicate.request.json");
    const text = JSON.stringify(request("duplicate-member")).replace(
      '"execution_id":"duplicate-member"',
      '"execution_id":"first","execution_id":"second"',
    );
    writeFileSync(duplicate, text);
    expectPreflightRefusal(invoke(["--json", "run", "--request", duplicate, "--out-dir", join(root, "duplicate.run")]), "duplicate JSON member", "request-invalid");

    const oversized = join(root, "oversized.request.json");
    writeFileSync(oversized, `${JSON.stringify(request("oversized"))}${" ".repeat(1024 * 1024)}\n`);
    expectPreflightRefusal(invoke(["--json", "run", "--request", oversized, "--out-dir", join(root, "oversized.run")], { maxBuffer: 2 * 1024 * 1024 }), "oversized JSON", "request-invalid");
  });

  record("an existing output directory is never reused or overwritten", () => {
    const marker = join(baseline.outDir, "operator-marker.txt");
    writeFileSync(marker, "preserve\n", { flag: "wx" });
    const retryRequest = join(root, "reuse.request.json");
    writeJson(retryRequest, request("reuse-attempt"));
    expectPreflightRefusal(invoke(["--json", "run", "--request", retryRequest, "--out-dir", baseline.outDir]), "output directory reuse", "output-directory-reused");
    assert.equal(readFileSync(marker, "utf8"), "preserve\n");
  });

  record("invalid output produces an explicit no-draft receipt and no output authority", () => {
    const attempted = runRequest(request("invalid-output", invalidOutput), "invalid-output");
    expectFail(attempted.result, "invalid output run");
    const run = JSON.parse(attempted.result.stdout).data;
    assert.equal(run.terminal_state, "no-draft:output-invalid");
    const receipt = JSON.parse(readFileSync(join(attempted.outDir, "run-receipt.json"), "utf8"));
    assert.equal(receipt.output, null);
    assert.equal(receipt.decision, null);
    assert.equal(receipt.compiled_context, null);
    assert.equal(existsSync(join(attempted.outDir, "artifacts", "output.json")), false);
    assert.equal(expectOk(verify(attempted.outDir), "no-draft verification").valid, true);
  });

  record("artifact and receipt tampering are detected independently", () => {
    const artifactCopy = join(root, "tampered-artifact.run");
    cpSync(baseline.outDir, artifactCopy, { recursive: true });
    writeFileSync(join(artifactCopy, "artifacts", "output.json"), "{\"tampered\":true}\n");
    const artifactResult = expectInvalidJson(verify(artifactCopy), "tampered artifact verification");
    assert.equal(artifactResult.valid, false);
    assert.ok(artifactResult.issues.some((issue) => issue.startsWith("artifact-")));

    const receiptCopy = join(root, "tampered-receipt.run");
    cpSync(baseline.outDir, receiptCopy, { recursive: true });
    const receiptPath = join(receiptCopy, "run-receipt.json");
    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    receipt.operation = "forged-operation";
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
    const receiptResult = expectInvalidJson(verify(receiptCopy), "tampered receipt verification");
    assert.equal(receiptResult.valid, false);
    assert.ok(receiptResult.issues.includes("profile-operation-mismatch"));
    assert.ok(receiptResult.issues.includes("receipt-hash-mismatch"));
  });

  record("driver-attested evidence cannot elevate enforced or verified assurance", () => {
    const copy = join(root, "false-elevation.run");
    cpSync(baseline.outDir, copy, { recursive: true });
    const receiptPath = join(copy, "run-receipt.json");
    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    receipt.assurance[0].provenance = "driver-attested";
    receipt.assurance[0].state = "verified";
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
    const verification = expectInvalidJson(verify(copy), "false assurance elevation verification");
    assert.equal(verification.valid, false);
    assert.ok(verification.issues.some((issue) => issue.startsWith("driver-attestation-cannot-elevate:")));
  });

  record("local replay ledger classifies first, exact replay, duplicate, cross-job, and prior-version mismatch", () => {
    const ledger = join(root, "replay.jsonl");
    const hashA = "a".repeat(64);
    const hashB = "b".repeat(64);
    assert.equal(expectOk(consume(ledger, "job-a", "key-a", hashA, 0), "first consume").outcome.outcome, "accepted-first");
    assert.equal(expectOk(consume(ledger, "job-a", "key-a", hashA, 0, true), "exact replay").outcome.outcome, "permitted-exact-replay");
    assert.equal(expectOk(consume(ledger, "job-a", "key-a", hashB, 0, true), "duplicate consume").outcome.outcome, "duplicate");
    assert.equal(expectOk(consume(ledger, "job-b", "key-b", hashA, 0, true), "cross-job consume").outcome.outcome, "cross-job");
    assert.equal(expectOk(consume(ledger, "job-b", "key-b", hashB, 0), "stale prior version").outcome.outcome, "prior-version-mismatch");
  });

  record("replay corruption, interrupted append, and stale lock fail closed", () => {
    const hash = "c".repeat(64);
    const corrupt = join(root, "corrupt.jsonl");
    expectOk(consume(corrupt, "job-c", "key-c", hash, 0), "seed corrupt ledger");
    const bytes = readFileSync(corrupt, "utf8").replace('"job-c"', '"job-x"');
    writeFileSync(corrupt, bytes);
    expectFail(consume(corrupt, "job-d", "key-d", "d".repeat(64), 1), "corrupt ledger", /hash mismatch/i);

    const interrupted = join(root, "interrupted.jsonl");
    writeFileSync(interrupted, "{\"partial\":true}");
    expectFail(consume(interrupted, "job-e", "key-e", "e".repeat(64), 0), "interrupted ledger", /unterminated append/i);

    const locked = join(root, "locked.jsonl");
    writeFileSync(`${locked}.lock`, "pid=stale\n");
    expectFail(consume(locked, "job-f", "key-f", "f".repeat(64), 0), "stale lock", /lock .* exists|cannot be created/i);
    assert.equal(existsSync(locked), false);
  });

  record("rollback and cloned ledgers are identified as an explicit non-guarantee", () => {
    const ledger = join(root, "rollback-source.jsonl");
    const clone = join(root, "rollback-clone.jsonl");
    const first = expectOk(consume(ledger, "job-r", "key-r", "1".repeat(64), 0), "seed rollback ledger");
    assert.match(first.limitation, /cannot detect filesystem rollback, snapshot restore, or cloning/i);
    copyFileSync(ledger, clone);
    const clonedReplay = expectOk(consume(clone, "job-r", "key-r", "1".repeat(64), 0, true), "cloned ledger replay");
    assert.equal(clonedReplay.outcome.outcome, "permitted-exact-replay");
    assert.match(clonedReplay.limitation, /host-owned durable, atomic storage/i);
  });
} finally {
  const failed = results.filter((result) => result.status === "fail");
  const coverageCounts = new Map();
  for (const result of results) {
    const key = `${result.profile}/${result.adapter}`;
    const value = coverageCounts.get(key) || { passed: 0, total: 0 };
    value.total += 1;
    if (result.status === "pass") value.passed += 1;
    coverageCounts.set(key, value);
  }
  const coverage = [...coverageCounts.entries()]
    .map(([key, value]) => `${key}=${value.passed}/${value.total}`)
    .join(", ");
  process.stdout.write(`\n${results.length - failed.length}/${results.length} conformance checks passed\n`);
  process.stdout.write(`coverage: ${coverage}\n`);
  if (keep) {
    process.stdout.write(`artifacts retained at ${root}\n`);
  } else {
    try {
      chmodSync(root, 0o700);
      rmSync(root, { recursive: true, force: true });
    } catch (error) {
      process.stderr.write(`warning: could not remove conformance scratch: ${error.message}\n`);
    }
  }
  if (failed.length > 0) process.exitCode = 1;
}
