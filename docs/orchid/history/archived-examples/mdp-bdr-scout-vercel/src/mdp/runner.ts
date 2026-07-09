import type { Candidate, EvidenceSource } from "../schemas/candidate.ts";
import type { MdpDecision } from "../schemas/ledger.ts";
import { runNativeMdp } from "./native-runner.ts";
import { runSandboxMdp } from "./sandbox-runner.ts";

export type MdpRunnerInput = {
  candidate: Candidate;
  evidence: EvidenceSource[];
};

export type MdpRunner = {
  runFitAndBrief(input: MdpRunnerInput): Promise<MdpDecision>;
};

export function createMdpRunner(options: { mode?: string } = {}): MdpRunner {
  const mode = options.mode ?? process.env.MDP_RUNNER_MODE ?? "simulated";
  if (mode === "native") return { runFitAndBrief: runNativeMdp };
  if (mode === "sandbox") return { runFitAndBrief: runSandboxMdp };
  return { runFitAndBrief: runSimulatedMdp };
}

async function runSimulatedMdp(input: MdpRunnerInput): Promise<MdpDecision> {
  const hasEvidence = input.evidence.length > 0;
  const hasTrigger = input.candidate.trigger.length > 0;
  const fit = hasEvidence && hasTrigger;

  return {
    fit_status: fit ? "fit" : "insufficient_context",
    persona: input.candidate.persona ?? input.candidate.title ?? null,
    route: fit ? "operator_review" : null,
    brief_json_url: null,
    brief_md_url: null,
    gaps: fit ? [] : ["Need source-backed trigger and enough context before qualification."]
  };
}
