import { discoverCandidates } from "./discovery.ts";
import { appendLedgerRows, createRunId } from "./ledger.ts";
import { runMdpBrief } from "./mdp-runner.ts";
import { scoreCandidate } from "./scoring.ts";
import { loadSourceStrategy, selectScoutQuery } from "./source-strategy.ts";
import type { LedgerRow } from "./schemas.ts";

export async function runFixtureScoutCycle(): Promise<{ runId: string; query: string; qualified: number; ledgerPath: string | null; rows: LedgerRow[] }> {
  const strategy = await loadSourceStrategy();
  const selected = selectScoutQuery(strategy);
  const discovered = await discoverCandidates({ query: selected.query, limit: Number(process.env.SCOUT_MAX_CANDIDATES ?? 5), dryRun: true });
  const runId = createRunId();
  const minScore = Number(process.env.SCOUT_MIN_SCORE ?? 65);
  const rows: LedgerRow[] = [];

  for (const item of discovered) {
    const mdp = await runMdpBrief(item, "linkedin");
    const score = scoreCandidate({ mdp, evidence: item.evidence });
    if (score.overall < minScore) continue;
    rows.push({
      contract_version: "mdp_scout_candidate/v0",
      run_id: runId,
      pack_id: process.env.MDP_PACK_ID ?? "profound-gtm-vetting-example",
      source_strategy: selected.trace,
      candidate: item.candidate,
      evidence: item.evidence,
      mdp,
      score,
      actions: { outreach_sent: false, crm_sync_status: process.env.CRM_SYNC_ENABLED === "true" ? "pending" : "not_enabled" }
    });
  }

  const written = rows.length ? await appendLedgerRows(rows) : null;
  return { runId, query: selected.query, qualified: rows.length, ledgerPath: written?.ledgerPath ?? null, rows };
}
