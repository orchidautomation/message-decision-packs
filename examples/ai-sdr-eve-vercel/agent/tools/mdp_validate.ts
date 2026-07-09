import { defineTool } from "eve/tools";
import { z } from "zod";
import { validateMdpPack } from "../lib/mdp-runner.ts";
import { loadSourceStrategy, summarizeSourceStrategy } from "../lib/source-strategy.ts";

export default defineTool({
  description: "Validate the active MDP pack. Uses the mdp CLI when available and always checks the source-strategy artifact shape.",
  inputSchema: z.object({}),
  async execute() {
    const [cli, strategy] = await Promise.all([validateMdpPack(), loadSourceStrategy()]);
    return { cli, source_strategy: summarizeSourceStrategy(strategy) };
  }
});
