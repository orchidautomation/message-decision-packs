import { createMdpRunner } from "../../src/mdp/runner.ts";
import type { Candidate, EvidenceSource } from "../../src/schemas/candidate.ts";

export async function runMdp(input: { candidate: Candidate; evidence: EvidenceSource[] }) {
  const runner = createMdpRunner({ mode: process.env.MDP_RUNNER_MODE ?? "simulated" });
  return runner.runFitAndBrief(input);
}
