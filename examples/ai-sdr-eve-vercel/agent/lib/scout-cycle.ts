import { discoverCandidates } from "./discovery.ts";
import { appendLedgerRows, createRunId } from "./ledger.ts";
import { runMdpBrief } from "./mdp-runner.ts";
import { scoreCandidate } from "./scoring.ts";
import { loadSourceStrategy, selectScoutQuery } from "./source-strategy.ts";
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
  const discovery = await discoverCandidates({
    query: selected.query,
    limit: input.limit ?? Number(process.env.SCOUT_MAX_CANDIDATES ?? 5),
    dryRun: input.dryRun
  });
  const runId = createRunId();
  const minScore = Number(process.env.SCOUT_MIN_SCORE ?? 65);
  const rows: LedgerRow[] = [];
  const trace: SourceStrategyTrace = {
    ...selected.trace,
    provider_mode: discovery.mode,
    provider_available: discovery.provider === "exa",
    provider_fallback: discovery.fallbackReason
  };

  for (const item of discovery.candidates) {
    const mdp = await runMdpBrief(item, "linkedin");
    const score = scoreCandidate({ mdp, evidence: item.evidence });
    if (score.overall < minScore) continue;
    const personEvidenceIds = item.evidence.filter((evidence) => evidence.id.startsWith("exa_person_")).map((evidence) => evidence.id);
    rows.push({
      contract_version: "mdp_scout_candidate/v0",
      run_id: runId,
      pack_id: process.env.MDP_PACK_ID ?? "profound-gtm-vetting-example",
      source_strategy: {
        ...trace,
        person_resolution_status: item.candidate.name && item.candidate.title ? "resolved" : "not_found",
        person_resolution_evidence_ids: personEvidenceIds
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
