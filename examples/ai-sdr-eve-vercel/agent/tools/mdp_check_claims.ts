import { defineTool } from "eve/tools";
import { z } from "zod";
import { checkClaims } from "../lib/mdp-runner.ts";

export default defineTool({
  description: "Check claim-bearing text against the active MDP claims policy. Use before any brief-derived copy is considered safe.",
  inputSchema: z.object({ text: z.string().min(1), mode: z.enum(["simulated", "native"]).optional() }),
  async execute({ text, mode }) {
    return checkClaims(text, mode);
  }
});
