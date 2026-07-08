import type { CandidateWithEvidence } from "../schemas/candidate.ts";

export type ExaDiscoveryInput = {
  query: string;
  limit: number;
  dryRun?: boolean;
  fixture?: CandidateWithEvidence;
};

export async function discoverWithExa(input: ExaDiscoveryInput): Promise<CandidateWithEvidence[]> {
  if (input.dryRun || !process.env.EXA_API_KEY) {
    return input.fixture ? [input.fixture] : [];
  }

  throw new Error("Live Exa discovery is intentionally not enabled in this scaffold. Add @exalabs/ai-sdk or Exa API wiring with rate limits first.");
}
