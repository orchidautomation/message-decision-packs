import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { Candidate } from "../schemas/candidate.ts";
import type { MdpRunnerInput } from "./runner.ts";
import type { MdpDecision } from "../schemas/ledger.ts";

const execFileAsync = promisify(execFile);

type MdpJson = {
  ok?: boolean;
  data?: Record<string, unknown>;
  error?: { message?: string; details?: unknown[] };
};

export async function runNativeMdp(input: MdpRunnerInput): Promise<MdpDecision> {
  const mdpBin = process.env.MDP_BIN ?? "mdp";
  const packDir = process.env.MDP_PACK_DIR;
  if (!packDir) throw new Error("MDP_PACK_DIR is required for native MDP runner mode");

  const scratch = await mkdtemp(join(tmpdir(), "mdp-bdr-scout-"));
  try {
    const prospectPath = join(scratch, "prospect.json");
    await writeFile(prospectPath, JSON.stringify(toMdpProspect(input.candidate), null, 2));

    const fit = await runMdpJson(mdpBin, ["--json", "fit", "--dir", packDir, "--prospect", prospectPath]);
    if (!fit.ok) throw new Error(formatMdpError("fit", fit));

    let brief: MdpJson | null = null;
    try {
      brief = await runMdpJson(mdpBin, ["--json", "brief", "--context", "--dir", packDir, "--prospect", prospectPath]);
    } catch {
      brief = null;
    }

    const fitData = fit.data ?? {};
    const briefData = brief?.data ?? {};
    const personaResolution = asRecord(fitData.persona_resolution);
    const context = asRecord(fitData.context);
    const missing = Array.isArray(context.missing) ? context.missing.map(String) : [];

    return {
      fit_status: fitData.status === "fit" ? "fit" : "insufficient_context",
      persona: stringOrNull(personaResolution.persona) ?? input.candidate.persona ?? input.candidate.title,
      route: stringOrNull(briefData.channel) ?? "operator_review",
      brief_json_url: null,
      brief_md_url: null,
      gaps: missing
    };
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

async function runMdpJson(bin: string, args: string[]): Promise<MdpJson> {
  const output = await execFileAsync(bin, args, { timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
  return JSON.parse(output.stdout) as MdpJson;
}

function toMdpProspect(candidate: Candidate): Record<string, unknown> {
  const prospect: Record<string, unknown> = {
    name: candidate.name ?? "Unknown Contact",
    title: candidate.title ?? "Unknown Role",
    company: candidate.company,
    trigger: candidate.trigger,
    source_kind: candidate.source_kind,
    synthetic: true
  };

  setIfString(prospect, "company_domain", candidate.company_domain);
  setIfString(prospect, "linkedin_url", candidate.linkedin_url);
  setIfString(prospect, "persona", candidate.persona ?? null);
  setIfString(prospect, "segment", candidate.segment ?? null);

  if (candidate.signals?.length) {
    prospect.signals = candidate.signals.map((signal, index) => ({
      id: slugSignal(signal, index),
      title: signal,
      source: "mdp-bdr-scout"
    }));
  }

  return prospect;
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
  return typeof value === "string" && value.length > 0 ? value : null;
}

function formatMdpError(command: string, payload: MdpJson): string {
  const details = payload.error?.details?.length ? ` (${payload.error.details.map(String).join("; ")})` : "";
  return `mdp ${command} failed: ${payload.error?.message ?? "unknown error"}${details}`;
}
