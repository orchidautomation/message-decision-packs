import { defineTool } from "eve/tools";
import { z } from "zod";
import { discoverCandidates } from "../lib/discovery.ts";
import { loadSourceStrategy, selectPersonResolutionQuery, selectScoutQuery } from "../lib/source-strategy.ts";

export default defineTool({
  description: "Discover public-source candidates from the active MDP source strategy. Uses live Exa via an AI SDK-compatible tool when EXA_API_KEY is present; only dryRun=true returns the public-safe fixture.",
  inputSchema: z.object({ query: z.string().optional(), limit: z.number().int().min(1).max(20).optional(), dryRun: z.boolean().optional() }),
  async execute(input) {
    const strategy = await loadSourceStrategy();
    const selected = selectScoutQuery(strategy, input.query);
    const personResolutionQuery = selectPersonResolutionQuery(strategy);
    const limit = input.limit ?? parseIntegerSetting(process.env.SCOUT_MAX_CANDIDATES, 5, 1, 20);
    const discovery = await discoverCandidates({
      query: selected.query,
      limit,
      dryRun: input.dryRun,
      personResolutionQueryTemplate: personResolutionQuery?.query_template ?? null
    });
    return {
      selected,
      person_resolution_query: personResolutionQuery ? {
        query_id: personResolutionQuery.id,
        target_source_ids: personResolutionQuery.target_source_ids ?? [],
        query_usage: personResolutionQuery.query_usage ?? null,
        agent_instruction: personResolutionQuery.agent_instruction,
        construction_rules: personResolutionQuery.construction_rules,
        negative_filters: personResolutionQuery.negative_filters,
        expected_signals: personResolutionQuery.expected_signals,
        required_receipts: personResolutionQuery.required_receipts ?? [],
        review_required: personResolutionQuery.review_required ?? true
      } : null,
      provider: discovery.provider,
      mode: discovery.mode,
      provider_capabilities: discovery.providerCapabilities,
      fallback_reason: discovery.fallbackReason,
      count: discovery.candidates.length,
      candidates: discovery.candidates
    };
  }
});

function parseIntegerSetting(value: string | undefined, fallback: number, min: number, max: number): number {
  const parsed = Number(value ?? fallback);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(min, Math.min(max, Math.trunc(parsed)));
}
