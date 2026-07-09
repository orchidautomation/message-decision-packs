import { appendFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { outputDir } from "./paths.ts";
import type { LedgerRow } from "./schemas.ts";

export async function appendLedgerRows(rows: LedgerRow[]): Promise<{ ledgerPath: string; rowsWritten: number }> {
  await mkdir(outputDir(), { recursive: true });
  const ledgerPath = join(outputDir(), "scout-ledger.jsonl");
  const body = rows.map((row) => JSON.stringify(assertLedgerRow(row))).join("\n") + "\n";
  await appendFile(ledgerPath, body, "utf8");
  return { ledgerPath, rowsWritten: rows.length };
}

export function assertLedgerRow(row: LedgerRow): LedgerRow {
  if (row.contract_version !== "mdp_scout_candidate/v0") throw new Error("unexpected ledger contract version");
  if (!row.run_id) throw new Error("run_id is required");
  if (!row.pack_id) throw new Error("pack_id is required");
  if (!row.source_strategy.query_id) throw new Error("source_strategy.query_id is required");
  if (row.score.overall < 0 || row.score.overall > 100) throw new Error("score.overall must be 0-100");
  if (row.actions.outreach_sent !== false) throw new Error("outreach must stay disabled");
  return row;
}

export function createRunId(prefix = "eve_scout"): string {
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
  return `${prefix}_${stamp}`;
}
