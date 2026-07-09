import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { packDir } from "./paths.ts";
import type { SourceStrategyTrace } from "./schemas.ts";

export type SourceStrategy = {
  format: "mdp.source-strategy.v0";
  profile: { id: string; label: string };
  objective: Record<string, unknown> & { decision_needed: string };
  source_targets: Array<{ id: string; scout_family: string; source_kind: string }>;
  queries_prompts: Array<{
    id: string;
    scout_family: string;
    query_or_prompt: string;
    target_source_ids?: string[];
    negative_filters: string[];
    expected_signals: string[];
    max_results?: number;
  }>;
  review_status: "draft" | "needs-human-review" | "accepted" | "blocked";
};

export type SelectedScoutQuery = {
  queryId: string;
  scoutFamily: string;
  query: string;
  targetSourceIds: string[];
  trace: SourceStrategyTrace;
};

export async function loadSourceStrategy(): Promise<SourceStrategy> {
  const path = join(packDir(), "source-strategy.json");
  const raw = JSON.parse(await readFile(path, "utf8")) as unknown;
  assertSourceStrategy(raw);
  return raw;
}

export function selectScoutQuery(strategy: SourceStrategy, override?: string | null): SelectedScoutQuery {
  const query = strategy.queries_prompts.find((item) => item.scout_family === "exa") ?? strategy.queries_prompts[0];
  if (!query) throw new Error("source strategy has no query prompts");
  return {
    queryId: query.id,
    scoutFamily: query.scout_family,
    query: override && override.trim() ? override.trim() : query.query_or_prompt,
    targetSourceIds: query.target_source_ids ?? [],
    trace: {
      strategy_id: strategy.format,
      profile_id: strategy.profile.id,
      review_status: strategy.review_status,
      query_id: query.id,
      scout_family: query.scout_family,
      source_target_ids: query.target_source_ids ?? []
    }
  };
}

export function summarizeSourceStrategy(strategy: SourceStrategy) {
  return {
    format: strategy.format,
    profile: strategy.profile,
    review_status: strategy.review_status,
    objective: strategy.objective.decision_needed,
    queries: strategy.queries_prompts.map((item) => ({ id: item.id, scout_family: item.scout_family, max_results: item.max_results ?? null })),
    source_targets: strategy.source_targets.map((item) => ({ id: item.id, scout_family: item.scout_family, source_kind: item.source_kind }))
  };
}

function assertSourceStrategy(value: unknown): asserts value is SourceStrategy {
  if (!isRecord(value)) throw new Error("source strategy must be an object");
  if (value.format !== "mdp.source-strategy.v0") throw new Error("unexpected source strategy format");
  if (!isRecord(value.profile) || typeof value.profile.id !== "string") throw new Error("source strategy profile.id is required");
  if (!isRecord(value.objective) || typeof value.objective.decision_needed !== "string") throw new Error("source strategy objective.decision_needed is required");
  if (!Array.isArray(value.source_targets)) throw new Error("source strategy source_targets must be an array");
  if (!Array.isArray(value.queries_prompts) || value.queries_prompts.length === 0) throw new Error("source strategy queries_prompts must be non-empty");
  if (!["draft", "needs-human-review", "accepted", "blocked"].includes(String(value.review_status))) throw new Error("source strategy review_status is invalid");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
