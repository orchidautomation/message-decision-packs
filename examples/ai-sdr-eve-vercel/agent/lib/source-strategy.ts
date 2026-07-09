import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { packDir } from "./paths.ts";
import { providerCapabilities } from "./provider-tools.ts";
import type { SourceStrategyTrace } from "./schemas.ts";

export type AgentOperatingPlan = {
  role: string;
  operating_instructions: string[];
  stop_conditions: string[];
  insufficient_evidence_action: string;
  downstream_handoff_prompt: string;
};

export type SourceTarget = {
  id: string;
  scout_family: string;
  source_kind: string;
  target?: string;
  purpose?: string;
  allowed_access?: string;
  freshness?: string;
  primitives?: string[];
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

export type SourceStrategy = {
  format: "mdp.source-strategy.v0";
  profile: { id: string; label: string };
  objective: Record<string, unknown> & { decision_needed: string };
  agent_operating_plan: AgentOperatingPlan;
  primitive_mappings: Array<{ primitive: string; known: string[]; evidence_needed: string[]; gaps: string[] }>;
  source_targets: SourceTarget[];
  queries_prompts: SourceQueryPrompt[];
  exclusions: string[];
  evidence_requirements: Record<string, unknown>;
  routing_jobs: Array<{ id: string; next_skill: string; review_job?: string; handoff: string; cli_handoff?: string }>;
  gaps: string[];
  eval_checks: Array<{ id: string; scenario: string; expected: string }>;
  review_status: "draft" | "needs-human-review" | "accepted" | "blocked";
};

export type SelectedScoutQuery = {
  queryId: string;
  scoutFamily: string;
  query: string;
  targetSourceIds: string[];
  agentInstruction: string;
  constructionRules: string[];
  negativeFilters: string[];
  expectedSignals: string[];
  requiredReceipts: string[];
  reviewRequired: boolean;
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
    agentInstruction: query.agent_instruction,
    constructionRules: query.construction_rules,
    negativeFilters: query.negative_filters,
    expectedSignals: query.expected_signals,
    requiredReceipts: query.required_receipts ?? [],
    reviewRequired: query.review_required ?? true,
    trace: {
      strategy_id: strategy.format,
      profile_id: strategy.profile.id,
      review_status: strategy.review_status,
      query_id: query.id,
      scout_family: query.scout_family,
      source_target_ids: query.target_source_ids ?? [],
      agent_instruction: query.agent_instruction,
      required_receipts: query.required_receipts ?? []
    }
  };
}

export function summarizeSourceStrategy(strategy: SourceStrategy) {
  return {
    format: strategy.format,
    profile: strategy.profile,
    review_status: strategy.review_status,
    objective: strategy.objective.decision_needed,
    agent_operating_plan: strategy.agent_operating_plan,
    provider_capabilities: providerCapabilities(),
    queries: strategy.queries_prompts.map((item) => ({
      id: item.id,
      scout_family: item.scout_family,
      max_results: item.max_results ?? null,
      agent_instruction: item.agent_instruction,
      construction_rules: item.construction_rules,
      required_receipts: item.required_receipts ?? [],
      review_required: item.review_required ?? true
    })),
    source_targets: strategy.source_targets.map((item) => ({
      id: item.id,
      scout_family: item.scout_family,
      source_kind: item.source_kind,
      allowed_access: item.allowed_access ?? null
    }))
  };
}

function assertSourceStrategy(value: unknown): asserts value is SourceStrategy {
  if (!isRecord(value)) throw new Error("source strategy must be an object");
  if (value.format !== "mdp.source-strategy.v0") throw new Error("unexpected source strategy format");
  if (!isRecord(value.profile) || typeof value.profile.id !== "string" || typeof value.profile.label !== "string") throw new Error("source strategy profile id/label are required");
  if (!isRecord(value.objective) || typeof value.objective.decision_needed !== "string") throw new Error("source strategy objective.decision_needed is required");
  assertAgentOperatingPlan(value.agent_operating_plan);
  assertArray(value.primitive_mappings, "primitive_mappings");
  assertArray(value.source_targets, "source_targets");
  assertArray(value.queries_prompts, "queries_prompts");
  assertArray(value.exclusions, "exclusions");
  if (!isRecord(value.evidence_requirements)) throw new Error("source strategy evidence_requirements must be an object");
  assertArray(value.routing_jobs, "routing_jobs");
  assertArray(value.gaps, "gaps");
  assertArray(value.eval_checks, "eval_checks");
  if (!["draft", "needs-human-review", "accepted", "blocked"].includes(String(value.review_status))) throw new Error("source strategy review_status is invalid");

  for (const target of value.source_targets) {
    if (!isRecord(target)) throw new Error("source target must be an object");
    for (const field of ["id", "source_kind", "scout_family"]) requireString(target, field, "source target");
  }

  for (const query of value.queries_prompts) {
    if (!isRecord(query)) throw new Error("source query prompt must be an object");
    for (const field of ["id", "scout_family", "query_or_prompt", "agent_instruction"]) requireString(query, field, "source query prompt");
    assertArray(query.construction_rules, `source query ${String(query.id)} construction_rules`);
    assertArray(query.negative_filters, `source query ${String(query.id)} negative_filters`);
    assertArray(query.expected_signals, `source query ${String(query.id)} expected_signals`);
  }
}

function assertAgentOperatingPlan(value: unknown): asserts value is AgentOperatingPlan {
  if (!isRecord(value)) throw new Error("source strategy agent_operating_plan must be an object");
  for (const field of ["role", "insufficient_evidence_action", "downstream_handoff_prompt"]) requireString(value, field, "agent_operating_plan");
  assertArray(value.operating_instructions, "agent_operating_plan.operating_instructions");
  assertArray(value.stop_conditions, "agent_operating_plan.stop_conditions");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function assertArray(value: unknown, label: string): asserts value is unknown[] {
  if (!Array.isArray(value)) throw new Error(`source strategy ${label} must be an array`);
}

function requireString(value: Record<string, unknown>, field: string, label: string): void {
  if (typeof value[field] !== "string" || String(value[field]).trim().length === 0) {
    throw new Error(`source strategy ${label}.${field} is required`);
  }
}
