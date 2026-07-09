import { defineTool } from "eve/tools";
import { z } from "zod";
import { loadSourceStrategy, selectScoutQuery, summarizeSourceStrategy } from "../lib/source-strategy.ts";

export default defineTool({
  description: "Load the active .mdp/source-strategy.json and return the selected scout query plus source policy summary.",
  inputSchema: z.object({ overrideQuery: z.string().optional() }),
  async execute({ overrideQuery }) {
    const strategy = await loadSourceStrategy();
    const selected = selectScoutQuery(strategy, overrideQuery);
    return { strategy: summarizeSourceStrategy(strategy), selected };
  }
});
