import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { packRoot } from "./paths.ts";
import type { Candidate, CandidateWithEvidence, MdpDecision } from "./schemas.ts";

const execFileAsync = promisify(execFile);

type MdpJson = { ok?: boolean; data?: Record<string, unknown>; error?: { message?: string; details?: unknown[] } };

export async function validateMdpPack(): Promise<{ ok: boolean; cliAvailable: boolean; stdout?: string; stderr?: string; error?: string }> {
  try {
    const result = await execFileAsync(mdpBin(), ["--json", "validate", "--dir", packRoot()], { timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
    return { ok: true, cliAvailable: true, stdout: result.stdout, stderr: result.stderr };
  } catch (error) {
    return { ok: false, cliAvailable: !isMissingBinary(error), error: formatExecError(error) };
  }
}

export async function runMdpFit(input: CandidateWithEvidence, mode = process.env.MDP_RUNNER_MODE ?? "native"): Promise<MdpDecision> {
  if (mode === "native") return runNativeFit(input);
  return runSimulatedFit(input);
}

export async function runMdpBrief(input: CandidateWithEvidence, channel = "linkedin", mode = process.env.MDP_RUNNER_MODE ?? "native"): Promise<MdpDecision & { brief?: unknown }> {
  if (mode !== "native") {
    const fit = await runSimulatedFit(input);
    return { ...fit, route: fit.fit_status === "fit" ? channel : fit.route, brief: { mode: "simulated", channel, summary: input.candidate.trigger } };
  }

  const scratch = await mkdtemp(join(tmpdir(), "mdp-eve-brief-"));
  try {
    const prospectPath = join(scratch, "prospect.json");
    await writeFile(prospectPath, JSON.stringify(toMdpProspect(input.candidate), null, 2));
    const fit = await runMdpJson(["--json", "fit", "--dir", packRoot(), "--prospect", prospectPath]);
    if (!fit.ok) throw new Error(formatMdpError("fit", fit));
    const brief = await runMdpJson(["--json", "brief", "--context", "--dir", packRoot(), "--prospect", prospectPath, "--channel", channel]);
    const fitData = fit.data ?? {};
    const briefData = brief.data ?? {};
    return {
      fit_status: fitData.status === "fit" ? "fit" : "insufficient_context",
      persona: stringOrNull(asRecord(fitData.persona_resolution).persona) ?? input.candidate.persona ?? input.candidate.title,
      route: stringOrNull(briefData.channel) ?? channel,
      brief_json_url: null,
      brief_md_url: null,
      gaps: missingFromFit(fitData),
      brief: briefData
    };
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

export async function checkClaims(text: string, mode = process.env.MDP_RUNNER_MODE ?? "simulated"): Promise<{ ok: boolean; mode: string; result?: unknown; warnings: string[] }> {
  if (mode !== "native") {
    const risky = /guarantee|guaranteed|#1|best-in-class|approved by|certified/i.test(text);
    return { ok: !risky, mode: "simulated", warnings: risky ? ["Text contains unsupported superlative or guarantee language."] : [] };
  }
  const result = await runMdpJson(["--json", "check-claims", "--dir", packRoot(), "--text", text]);
  return { ok: Boolean(result.ok), mode: "native", result, warnings: [] };
}

async function runNativeFit(input: CandidateWithEvidence): Promise<MdpDecision> {
  const scratch = await mkdtemp(join(tmpdir(), "mdp-eve-fit-"));
  try {
    const prospectPath = join(scratch, "prospect.json");
    await writeFile(prospectPath, JSON.stringify(toMdpProspect(input.candidate), null, 2));
    const fit = await runMdpJson(["--json", "fit", "--dir", packRoot(), "--prospect", prospectPath]);
    if (!fit.ok) throw new Error(formatMdpError("fit", fit));
    const fitData = fit.data ?? {};
    return {
      fit_status: fitData.status === "fit" ? "fit" : "insufficient_context",
      persona: stringOrNull(asRecord(fitData.persona_resolution).persona) ?? input.candidate.persona ?? input.candidate.title,
      route: "operator_review",
      brief_json_url: null,
      brief_md_url: null,
      gaps: missingFromFit(fitData)
    };
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

function runSimulatedFit(input: CandidateWithEvidence): MdpDecision {
  return {
    fit_status: "insufficient_context",
    persona: input.candidate.persona ?? input.candidate.title,
    route: null,
    brief_json_url: null,
    brief_md_url: null,
    gaps: ["Native MDP fit was not run; set MDP_RUNNER_MODE=native and rerun mdp fit for the pack-owned qualification decision."]
  };
}

async function runMdpJson(args: string[]): Promise<MdpJson> {
  const result = await execFileAsync(mdpBin(), args, { timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
  return JSON.parse(result.stdout) as MdpJson;
}

function mdpBin(): string {
  return process.env.MDP_BIN ?? "mdp";
}

function toMdpProspect(candidate: Candidate): Record<string, unknown> {
  const prospect: Record<string, unknown> = {
    name: stringOrNull(candidate.name) ?? "N/A",
    title: stringOrNull(candidate.title) ?? "N/A",
    company: candidate.company,
    trigger: candidate.trigger,
    source_kind: candidate.source_kind,
    synthetic: candidate.source_kind === "public_web" && candidate.company_domain === "example.com"
  };
  setIfString(prospect, "company_domain", candidate.company_domain);
  setIfString(prospect, "linkedin_url", candidate.linkedin_url);
  setIfString(prospect, "persona", candidate.persona ?? null);
  setIfString(prospect, "segment", candidate.segment ?? null);
  if (candidate.signals?.length) prospect.signals = candidate.signals.map((signal, index) => ({ id: slugSignal(signal, index), title: signal, source: "mdp-eve-scout" }));
  return prospect;
}

function missingFromFit(fitData: Record<string, unknown>): string[] {
  const context = asRecord(fitData.context);
  return Array.isArray(context.missing) ? context.missing.map(String) : [];
}

function setIfString(target: Record<string, unknown>, key: string, value: string | null | undefined): void {
  if (typeof value === "string" && value.length > 0) target[key] = value;
}

function slugSignal(signal: string, index: number): string {
  const slug = signal.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "").slice(0, 48);
  return slug || `signal-${index + 1}`;
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {};
}

function stringOrNull(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function formatMdpError(command: string, payload: MdpJson): string {
  const details = payload.error?.details?.length ? ` (${payload.error.details.map(String).join("; ")})` : "";
  return `mdp ${command} failed: ${payload.error?.message ?? "unknown error"}${details}`;
}

function isMissingBinary(error: unknown): boolean {
  return typeof error === "object" && error !== null && "code" in error && (error as { code?: unknown }).code === "ENOENT";
}

function formatExecError(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    const maybe = error as { message?: string; stdout?: string; stderr?: string };
    return [maybe.message, maybe.stderr, maybe.stdout].filter(Boolean).join("\n");
  }
  return String(error);
}
