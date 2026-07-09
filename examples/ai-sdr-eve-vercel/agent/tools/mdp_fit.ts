import { defineTool } from "eve/tools";
import { z } from "zod";
import { candidateSchema, evidenceSchema } from "../lib/schemas.ts";
import { runMdpFit } from "../lib/mdp-runner.ts";

export default defineTool({
  description: "Run the bounded MDP fit gate for a normalized candidate and evidence bundle. Does not draft, send, enrich, or update CRM.",
  inputSchema: z.object({ candidate: candidateSchema, evidence: z.array(evidenceSchema).min(1), mode: z.enum(["simulated", "native"]).optional() }),
  async execute(input) {
    const mdp = await runMdpFit({ candidate: input.candidate, evidence: input.evidence }, input.mode);
    return { mdp };
  }
});
