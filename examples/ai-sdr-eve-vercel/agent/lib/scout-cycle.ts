import { discoverCandidates } from "./discovery.ts";
import { appendLedgerRows, createRunId } from "./ledger.ts";
import { runMdpBrief } from "./mdp-runner.ts";
import { normalizeScoreThreshold, validateQualifiedCandidate } from "./qualification.ts";
import { scoreCandidate } from "./scoring.ts";
import { loadSourceStrategy, selectPersonResolutionQuery, selectScoutQuery } from "./source-strategy.ts";
import type { LedgerRow, SourceStrategyTrace } from "./schemas.ts";

export type ScoutCycleInput = {
  dryRun?: boolean;
  limit?: number;
  query?: string | null;
};

export type ScoutCycleResult = {
  runId: string;
  query: string;
  qualified: number;
  ledgerPath: string | null;
  rows: LedgerRow[];
  provider: string;
  fallbackReason: string | null;
};

export async function runFixtureScoutCycle(): Promise<ScoutCycleResult> {
  return runScoutCycle({ dryRun: true });
}

export async function runScoutCycle(input: ScoutCycleInput = {}): Promise<ScoutCycleResult> {
  const strategy = await loadSourceStrategy();
  const selected = selectScoutQuery(strategy, input.query);
  const personResolutionQuery = selectPersonResolutionQuery(strategy);
  const discovery = await discoverCandidates({
    query: selected.query,
    limit: input.limit ?? parseIntegerSetting(process.env.SCOUT_MAX_CANDIDATES, 5, 1, 20),
    dryRun: input.dryRun,
    personResolutionQueryTemplate: personResolutionQuery?.query_template ?? null
  });
  const runId = createRunId();
  const minScore = normalizeScoreThreshold(Number(process.env.SCOUT_MIN_SCORE ?? 65));
  const rows: LedgerRow[] = [];
  const trace: SourceStrategyTrace = {
    ...selected.trace,
    provider_mode: discovery.mode,
    provider_available: discovery.mode === "live",
    provider_fallback: discovery.fallbackReason
  };

  for (const item of discovery.candidates) {
    const mdp = await runMdpBrief(item, "linkedin");
    const score = scoreCandidate({ mdp, evidence: item.evidence });
    const qualification = validateQualifiedCandidate({ candidate: item.candidate, evidence: item.evidence, mdp, score, minScore });
    if (!qualification.ok) continue;
    rows.push({
      contract_version: "mdp_scout_candidate/v0",
      run_id: runId,
      pack_id: process.env.MDP_PACK_ID ?? "profound-gtm-vetting-example",
      source_strategy: {
        ...trace,
        person_resolution_status: qualification.personResolutionStatus,
        person_resolution_evidence_ids: qualification.personEvidenceIds
      },
      candidate: item.candidate,
      evidence: item.evidence,
      mdp,
      score,
      actions: { outreach_sent: false, crm_sync_status: process.env.CRM_SYNC_ENABLED === "true" ? "pending" : "not_enabled" }
    });
  }

  const written = rows.length ? await appendLedgerRows(rows) : null;
  return { runId, query: selected.query, qualified: rows.length, ledgerPath: written?.ledgerPath ?? null, rows, provider: discovery.provider, fallbackReason: discovery.fallbackReason };
}

function parseIntegerSetting(value: string | undefined, fallback: number, min: number, max: number): number {
  const parsed = Number(value ?? fallback);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(min, Math.min(max, Math.trunc(parsed)));
}
