import { defineAgent } from "eve";

export default defineAgent({
  model: process.env.MDP_SCOUT_MODEL ?? "openai/gpt-5.4-mini",
  reasoning: "low",
  limits: {
    maxInputTokensPerSession: 400_000,
    maxOutputTokensPerSession: 40_000,
    maxSubagentDepth: 2
  }
});
