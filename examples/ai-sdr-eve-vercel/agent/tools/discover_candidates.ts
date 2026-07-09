import { defineTool } from "eve/tools";
import { z } from "zod";
import { discoverCandidates } from "../lib/discovery.ts";
import { loadSourceStrategy, selectScoutQuery } from "../lib/source-strategy.ts";

export default defineTool({
  description: "Discover public-source candidates with Exa when EXA_API_KEY is present, otherwise return the public-safe fixture candidate.",
  inputSchema: z.object({ query: z.string().optional(), limit: z.number().int().min(1).max(20).optional(), dryRun: z.boolean().optional() }),
  async execute(input) {
    const strategy = await loadSourceStrategy();
    const selected = selectScoutQuery(strategy, input.query);
    const candidates = await discoverCandidates({ query: selected.query, limit: input.limit ?? Number(process.env.SCOUT_MAX_CANDIDATES ?? 5), dryRun: input.dryRun });
    return { selected, count: candidates.length, candidates };
  }
});
