import { defineTool } from "eve/tools";
import { z } from "zod";
import { createMdpRunner } from "../../src/mdp/runner.ts";
import type { Candidate, EvidenceSource } from "../../src/schemas/candidate.ts";

export async function runMdp(input: { candidate: Candidate; evidence: EvidenceSource[] }) {
  const runner = createMdpRunner({ mode: process.env.MDP_RUNNER_MODE ?? "simulated" });
  return runner.runFitAndBrief(input);
}

const evidenceSchema = z.object({
  id: z.string(),
  url: z.string().url(),
  title: z.string(),
  observed_at: z.string(),
  snippet: z.string(),
  content_hash: z.string(),
  confidence: z.number().min(0).max(1),
  provider: z.enum(["fixture", "exa", "firecrawl", "apify", "manual"])
});

export default defineTool({
  description: "Run trusted MDP fit and brief gates for a normalized candidate. The model never receives a generic shell.",
  inputSchema: z.object({
    candidate: z.object({
      name: z.string().nullable(),
      title: z.string().nullable(),
      company: z.string(),
      company_domain: z.string().nullable(),
      linkedin_url: z.string().nullable(),
      source_kind: z.enum(["public_web", "news", "github", "community", "dataset"]),
      trigger: z.string(),
      persona: z.string().nullable().optional(),
      segment: z.string().nullable().optional(),
      signals: z.array(z.string()).optional()
    }),
    evidence: z.array(evidenceSchema).min(1)
  }),
  async execute(input) {
    return runMdp(input);
  }
});
