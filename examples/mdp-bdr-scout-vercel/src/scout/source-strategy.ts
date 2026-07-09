import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import defaultStrategyJson from "../../samples/source-strategy.json" with { type: "json" };
import type { SourceStrategyTrace } from "../schemas/ledger.ts";

export type SourceStrategy = {
  format: "mdp.source-strategy.v0";
  profile: { id: string; label: string };
  objective: Record<string, unknown> & { decision_needed: string };
  agent_operating_plan?: {
    role: string;
    operating_instructions: string[];
    stop_conditions: string[];
    insufficient_evidence_action: string;
    downstream_handoff_prompt: string;
  };
  primitive_mappings: Array<{ primitive: string; known: string[]; evidence_needed: string[]; gaps: string[] }>;
  source_targets: SourceTarget[];
  queries_prompts: SourceQueryPrompt[];
  exclusions: string[];
  evidence_requirements: Record<string, unknown>;
  routing_jobs: Array<{ id: string; next_skill: string; review_job?: string; handoff: string }>;
  gaps: string[];
  eval_checks: Array<{ id: string; scenario: string; expected: string }>;
  review_status: "draft" | "needs-human-review" | "accepted" | "blocked";
};

export type SourceTarget = {
  id: string;
  source_kind: "user-provided-approved" | "approved-corpus" | "public-source" | "synthetic-example" | "sanitized-example" | "needs-approval" | "excluded";
  scout_family: string;
  target: string;
  purpose: string;
  allowed_access: string;
  freshness: string;
  primitives: string[];
};

export type SourceQueryPrompt = {
  id: string;
  scout_family: string;
  query_or_prompt: string;
  agent_instruction: string;
  construction_rules: string[];
  target_source_ids?: string[];
  negative_filters: string[];
  expected_signals: string[];
  max_results?: number;
  required_receipts?: string[];
  review_required?: boolean;
};

export type SelectedScoutQuery = {
  strategyId: string;
  profileId: string;
  reviewStatus: SourceStrategy["review_status"];
  queryId: string;
  scoutFamily: string;
  query: string;
  targetSourceIds: string[];
};

export async function loadSourceStrategy(pathOrUrl?: string | URL | null): Promise<SourceStrategy> {
  const raw = pathOrUrl ? JSON.parse(await readFile(toFilePath(pathOrUrl), "utf8")) : defaultStrategyJson;
  assertSourceStrategy(raw);
  return raw;
}

export function selectScoutQuery(strategy: SourceStrategy, overrideQuery?: string | null): SelectedScoutQuery {
  const queryPrompt = strategy.queries_prompts.find((prompt) => prompt.scout_family === "exa") ?? strategy.queries_prompts[0];
  if (!queryPrompt) throw new Error("Source strategy must include at least one query prompt");

  return {
    strategyId: strategy.format,
    profileId: strategy.profile.id,
    reviewStatus: strategy.review_status,
    queryId: queryPrompt.id,
    scoutFamily: queryPrompt.scout_family,
    query: overrideQuery && overrideQuery.trim().length > 0 ? overrideQuery.trim() : queryPrompt.query_or_prompt,
    targetSourceIds: queryPrompt.target_source_ids ?? []
  };
}

export function toSourceStrategyTrace(selected: SelectedScoutQuery): SourceStrategyTrace {
  return {
    strategy_id: selected.strategyId,
    profile_id: selected.profileId,
    review_status: selected.reviewStatus,
    query_id: selected.queryId,
    scout_family: selected.scoutFamily,
    source_target_ids: selected.targetSourceIds
  };
}

export function summarizeSourceStrategy(strategy: SourceStrategy): { profile: string; review_status: string; queries: string[]; source_targets: string[] } {
  return {
    profile: strategy.profile.id,
    review_status: strategy.review_status,
    queries: strategy.queries_prompts.map((query) => query.id),
    source_targets: strategy.source_targets.map((target) => target.id)
  };
}

export function assertSourceStrategy(value: unknown): asserts value is SourceStrategy {
  if (!isRecord(value)) throw new Error("source strategy must be an object");
  if (value.format !== "mdp.source-strategy.v0") throw new Error("source strategy format must be mdp.source-strategy.v0");
  if (!isRecord(value.profile)) throw new Error("source strategy profile is required");
  requireString(value.profile, "id", "profile");
  requireString(value.profile, "label", "profile");
  if (!isRecord(value.objective)) throw new Error("source strategy objective is required");
  requireString(value.objective, "decision_needed", "objective");
  if (value.agent_operating_plan !== undefined) {
    if (!isRecord(value.agent_operating_plan)) throw new Error("source strategy agent_operating_plan must be an object");
    requireString(value.agent_operating_plan, "role", "agent_operating_plan");
    requireArray(value.agent_operating_plan.operating_instructions, "agent_operating_plan.operating_instructions");
    requireArray(value.agent_operating_plan.stop_conditions, "agent_operating_plan.stop_conditions");
    requireString(value.agent_operating_plan, "insufficient_evidence_action", "agent_operating_plan");
    requireString(value.agent_operating_plan, "downstream_handoff_prompt", "agent_operating_plan");
  }
  requireArray(value.primitive_mappings, "primitive_mappings");
  requireArray(value.source_targets, "source_targets");
  requireArray(value.queries_prompts, "queries_prompts");
  requireArray(value.exclusions, "exclusions");
  if (!isRecord(value.evidence_requirements)) throw new Error("source strategy evidence_requirements is required");
  requireArray(value.routing_jobs, "routing_jobs");
  requireArray(value.gaps, "gaps");
  requireArray(value.eval_checks, "eval_checks");
  if (!["draft", "needs-human-review", "accepted", "blocked"].includes(String(value.review_status))) {
    throw new Error("source strategy review_status is invalid");
  }

  for (const target of value.source_targets) {
    if (!isRecord(target)) throw new Error("source target must be an object");
    for (const field of ["id", "source_kind", "scout_family", "target", "purpose", "allowed_access", "freshness"]) {
      requireString(target, field, "source target");
    }
    requireArray(target.primitives, `source target ${String(target.id)} primitives`);
  }

  for (const query of value.queries_prompts) {
    if (!isRecord(query)) throw new Error("source query prompt must be an object");
    for (const field of ["id", "scout_family", "query_or_prompt", "agent_instruction"]) {
      requireString(query, field, "source query prompt");
    }
    requireArray(query.construction_rules, `source query ${String(query.id)} construction_rules`);
    requireArray(query.negative_filters, `source query ${String(query.id)} negative_filters`);
    requireArray(query.expected_signals, `source query ${String(query.id)} expected_signals`);
  }
}

function toFilePath(pathOrUrl: string | URL): string {
  return pathOrUrl instanceof URL ? fileURLToPath(pathOrUrl) : pathOrUrl;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireArray(value: unknown, label: string): asserts value is unknown[] {
  if (!Array.isArray(value)) throw new Error(`source strategy ${label} must be an array`);
}

function requireString(value: Record<string, unknown>, field: string, label: string): void {
  if (typeof value[field] !== "string" || String(value[field]).trim().length === 0) {
    throw new Error(`source strategy ${label}.${field} is required`);
  }
}
