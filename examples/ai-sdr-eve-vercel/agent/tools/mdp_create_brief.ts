import { defineTool } from "eve/tools";
import { z } from "zod";
import { candidateSchema, evidenceSchema } from "../lib/schemas.ts";
import { runMdpBrief } from "../lib/mdp-runner.ts";

export default defineTool({
  description: "Create an MDP brief context for a fit candidate. This produces review context only; it never sends outreach.",
  inputSchema: z.object({
    candidate: candidateSchema,
    evidence: z.array(evidenceSchema).min(1),
    channel: z.string().min(1).default("linkedin"),
    mode: z.enum(["simulated", "native"]).optional()
  }),
  async execute(input) {
    const mdp = await runMdpBrief({ candidate: input.candidate, evidence: input.evidence }, input.channel, input.mode);
    return { mdp, outreach_sent: false };
  }
});
