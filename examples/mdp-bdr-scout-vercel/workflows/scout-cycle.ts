import { FatalError, getWritable } from "workflow";
import { loadScoutConfig, type ScoutConfig } from "../src/config.ts";
import { getDefaultFixture, readFixture } from "../src/providers/fixtures.ts";
import { discoverWithExa } from "../src/providers/exa.ts";
import { createMdpRunner } from "../src/mdp/runner.ts";
import { scoreCandidate } from "../src/scoring/score-candidate.ts";
import { appendLedgerRows } from "../src/storage/ledger.ts";
import type { CandidateWithEvidence } from "../src/schemas/candidate.ts";
import type { LedgerRow } from "../src/schemas/ledger.ts";

export type ScoutWorkflowInput = {
  packId?: string;
  scheduleId?: string;
  query?: string;
  fixturePath?: string;
  outputDir?: string;
  dryRun?: boolean;
  persist?: boolean;
};

export type ScoutWorkflowEvent =
  | { type: "step_start"; name: string; at: string }
  | { type: "step_done"; name: string; at: string; count?: number }
  | { type: "done"; at: string; runId: string; qualified: number; ledgerPath: string | null };

export type ScoutWorkflowResult = {
  runId: string;
  qualified: number;
  ledgerPath: string | null;
};

export async function scoutCycleWorkflow(input: ScoutWorkflowInput = {}): Promise<ScoutWorkflowResult> {
  "use workflow";

  const config = await loadConfigStep(input);
  const discovered = await discoverCandidatesStep({ input, config });
  const rows = await mdpScoreRowsStep({ input, config, discovered });
  const persisted = await persistRowsStep({ input, rows, runId: config.runId });
  await emitDone({ runId: persisted.runId, qualified: rows.length, ledgerPath: persisted.ledgerPath });

  return { runId: persisted.runId, qualified: rows.length, ledgerPath: persisted.ledgerPath };
}

async function loadConfigStep(input: ScoutWorkflowInput): Promise<ScoutConfig & { runId: string }> {
  "use step";
  await emit({ type: "step_start", name: "load_config", at: new Date().toISOString() });
  const config = loadScoutConfig({ packId: input.packId, scheduleId: input.scheduleId });
  const runId = createRunId(config.scheduleId);
  await emit({ type: "step_done", name: "load_config", at: new Date().toISOString() });
  return { ...config, runId };
}

async function discoverCandidatesStep(args: { input: ScoutWorkflowInput; config: ScoutConfig }): Promise<CandidateWithEvidence[]> {
  "use step";
  await emit({ type: "step_start", name: "discover_candidates", at: new Date().toISOString() });
  const dryRun = args.input.dryRun ?? !process.env.EXA_API_KEY;
  const fixture = args.input.fixturePath ? await readFixture(args.input.fixturePath) : dryRun ? getDefaultFixture() : undefined;
  const discovered = await discoverWithExa({
    query: args.input.query ?? "GTM engineering teams adopting AI agents",
    limit: args.config.maxCandidates,
    dryRun,
    fixture
  });
  await emit({ type: "step_done", name: "discover_candidates", at: new Date().toISOString(), count: discovered.length });
  return discovered;
}

async function mdpScoreRowsStep(args: { input: ScoutWorkflowInput; config: ScoutConfig & { runId?: string }; discovered: CandidateWithEvidence[] }): Promise<LedgerRow[]> {
  "use step";
  await emit({ type: "step_start", name: "mdp_score_rows", at: new Date().toISOString() });
  const runId = args.config.runId ?? createRunId(args.config.scheduleId);
  const runner = createMdpRunner({ mode: args.input.dryRun === false ? process.env.MDP_RUNNER_MODE : "simulated" });
  const rows: LedgerRow[] = [];

  for (const item of args.discovered.slice(0, args.config.maxCandidates)) {
    const mdp = await runner.runFitAndBrief(item);
    const score = scoreCandidate({ mdp, evidence: item.evidence });
    if (score.overall < args.config.minScore) continue;

    rows.push({
      contract_version: "mdp_scout_candidate/v0",
      run_id: runId,
      pack_id: args.config.packId,
      candidate: item.candidate,
      evidence: item.evidence,
      mdp,
      score,
      actions: {
        outreach_sent: false,
        crm_sync_status: args.config.crmSyncEnabled ? "pending" : "not_enabled"
      }
    });
  }

  await emit({ type: "step_done", name: "mdp_score_rows", at: new Date().toISOString(), count: rows.length });
  return rows;
}

async function persistRowsStep(args: { input: ScoutWorkflowInput; rows: LedgerRow[]; runId: string }): Promise<{ runId: string; ledgerPath: string | null }> {
  "use step";
  await emit({ type: "step_start", name: "persist_rows", at: new Date().toISOString() });
  if (args.rows.length === 0) {
    await emit({ type: "step_done", name: "persist_rows", at: new Date().toISOString(), count: 0 });
    return { runId: args.runId, ledgerPath: null };
  }

  const runId = args.rows[0]?.run_id;
  if (!runId) throw new FatalError("Cannot persist rows without run_id");

  let ledgerPath: string | null = null;
  if (args.input.persist !== false) {
    const written = await appendLedgerRows(args.rows, { outputDir: args.input.outputDir ?? "/tmp/mdp-bdr-scout" });
    ledgerPath = written.ledgerPath;
  }
  await emit({ type: "step_done", name: "persist_rows", at: new Date().toISOString(), count: args.rows.length });
  return { runId, ledgerPath };
}

async function emitDone(event: Omit<Extract<ScoutWorkflowEvent, { type: "done" }>, "type" | "at">): Promise<void> {
  "use step";
  await emit({ type: "done", at: new Date().toISOString(), ...event });
}

async function emit(event: ScoutWorkflowEvent): Promise<void> {
  "use step";
  console.log(`[mdp-bdr-scout] ${event.type}${"name" in event ? `:${event.name}` : ""} at=${event.at}`);
  const writer = getWritable<ScoutWorkflowEvent>().getWriter();
  try {
    await writer.write(event);
  } finally {
    writer.releaseLock();
  }
}

function createRunId(scheduleId: string): string {
  const iso = new Date().toISOString();
  const slug = iso.replace(/[-:.TZ]/g, "").slice(0, 14);
  return `scout_${scheduleId}_${slug}`;
}
