import { defineAgent } from "eve";

export default defineAgent({
  model: process.env.MDP_SCOUT_MODEL ?? "openai/gpt-5.4-mini"
});

export const bdrScoutAgentMetadata = {
  name: "mdp-bdr-scout",
  description: "Scheduled Vercel-first BDR Scout powered by Message Decision Packs.",
  tools: ["search", "extract", "mdp", "ledger"]
};
