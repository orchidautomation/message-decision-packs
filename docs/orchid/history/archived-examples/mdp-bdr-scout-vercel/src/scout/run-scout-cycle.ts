import { createHash } from "node:crypto";
import { loadScoutConfig } from "../config.ts";
import { getDefaultFixture, readFixture } from "../providers/fixtures.ts";
import { discoverWithExa } from "../providers/exa.ts";
import { createMdpRunner } from "../mdp/runner.ts";
import { scoreCandidate } from "../scoring/score-candidate.ts";
import { appendLedgerRows } from "../storage/ledger.ts";
import type { LedgerRow, SourceStrategyTrace } from "../schemas/ledger.ts";
import { loadSourceStrategy, selectScoutQuery, toSourceStrategyTrace } from "./source-strategy.ts";

export type ScoutCycleResult = {
  runId: string;
  qualified: LedgerRow[];
  ledgerPath: string | null;
  query: string;
  sourceStrategy: SourceStrategyTrace;
};

export async function runScoutCycle(options: {
  packId?: string;
  scheduleId?: string;
  fixturePath?: string | URL;
  outputDir?: string;
  query?: string;
  sourceStrategyPath?: string | URL;
  dryRun?: boolean;
  persist?: boolean;
} = {}): Promise<ScoutCycleResult> {
  const config = loadScoutConfig({ packId: options.packId, scheduleId: options.scheduleId });
  const runId = createRunId(config.scheduleId);
  const sourceStrategy = await loadSourceStrategy(options.sourceStrategyPath ?? process.env.SCOUT_SOURCE_STRATEGY_PATH);
  const selectedQuery = selectScoutQuery(sourceStrategy, options.query);
  const sourceStrategyTrace = toSourceStrategyTrace(selectedQuery);
  const fixture = options.fixturePath ? await readFixture(options.fixturePath) : getDefaultFixture();

  const discovered = await discoverWithExa({
    query: selectedQuery.query,
    limit: config.maxCandidates,
    dryRun: options.dryRun ?? true,
    fixture
  });

  const runner = createMdpRunner({ mode: options.dryRun === false ? process.env.MDP_RUNNER_MODE : "simulated" });
  const rows: LedgerRow[] = [];

  for (const item of discovered.slice(0, config.maxCandidates)) {
    const mdp = await runner.runFitAndBrief(item);
    const score = scoreCandidate({ mdp, evidence: item.evidence });
    if (score.overall < config.minScore) continue;

    rows.push({
      contract_version: "mdp_scout_candidate/v0",
      run_id: runId,
      pack_id: config.packId,
      source_strategy: sourceStrategyTrace,
      candidate: item.candidate,
      evidence: item.evidence,
      mdp,
      score,
      actions: {
        outreach_sent: false,
        crm_sync_status: config.crmSyncEnabled ? "pending" : "not_enabled"
      }
    });
  }

  let ledgerPath: string | null = null;
  if (options.persist !== false && rows.length > 0) {
    const written = await appendLedgerRows(rows, { outputDir: options.outputDir ?? "artifacts" });
    ledgerPath = written.ledgerPath;
  }

  return { runId, qualified: rows, ledgerPath, query: selectedQuery.query, sourceStrategy: sourceStrategyTrace };
}

function createRunId(scheduleId: string): string {
  const iso = new Date().toISOString();
  const hash = createHash("sha256").update(`${scheduleId}:${iso}`).digest("hex").slice(0, 10);
  return `scout_${iso.replace(/[-:.TZ]/g, "").slice(0, 14)}_${hash}`;
}
