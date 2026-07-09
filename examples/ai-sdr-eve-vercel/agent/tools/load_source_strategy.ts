import { defineTool } from "eve/tools";
import { z } from "zod";
import { loadSourceStrategy, selectScoutQueries, summarizeSourceStrategy } from "../lib/source-strategy.ts";

export default defineTool({
  description: "Load the active .mdp/source-strategy.json and return the selected scout query plus source policy summary.",
  inputSchema: z.object({ overrideQuery: z.string().optional() }),
  async execute({ overrideQuery }) {
    const strategy = await loadSourceStrategy();
    const selectedQueries = selectScoutQueries(strategy, overrideQuery);
    return { strategy: summarizeSourceStrategy(strategy), selected: selectedQueries[0], selected_queries: selectedQueries };
  }
});
