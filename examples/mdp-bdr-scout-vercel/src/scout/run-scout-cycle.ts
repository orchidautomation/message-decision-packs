import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { createHash } from "node:crypto";
import { loadScoutConfig } from "../config.ts";
import { readFixture } from "../providers/fixtures.ts";
import { discoverWithExa } from "../providers/exa.ts";
import { createMdpRunner } from "../mdp/runner.ts";
import { scoreCandidate } from "../scoring/score-candidate.ts";
import { appendLedgerRows } from "../storage/ledger.ts";
import type { LedgerRow } from "../schemas/ledger.ts";

const here = dirname(fileURLToPath(import.meta.url));
const defaultFixture = join(here, "../../samples/public-source-fixture.json");

export type ScoutCycleResult = {
  runId: string;
  qualified: LedgerRow[];
  ledgerPath: string | null;
};

export async function runScoutCycle(options: {
  packId?: string;
  scheduleId?: string;
  fixturePath?: string | URL;
  outputDir?: string;
  dryRun?: boolean;
  persist?: boolean;
} = {}): Promise<ScoutCycleResult> {
  const config = loadScoutConfig({ packId: options.packId, scheduleId: options.scheduleId });
  const runId = createRunId(config.scheduleId);
  const fixture = await readFixture(options.fixturePath ?? defaultFixture);

  const discovered = await discoverWithExa({
    query: "GTM engineering teams adopting AI agents",
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

  return { runId, qualified: rows, ledgerPath };
}

function createRunId(scheduleId: string): string {
  const iso = new Date().toISOString();
  const hash = createHash("sha256").update(`${scheduleId}:${iso}`).digest("hex").slice(0, 10);
  return `scout_${iso.replace(/[-:.TZ]/g, "").slice(0, 14)}_${hash}`;
}
