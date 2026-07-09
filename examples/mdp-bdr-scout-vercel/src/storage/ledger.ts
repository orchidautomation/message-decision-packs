import { mkdir, appendFile } from "node:fs/promises";
import { join } from "node:path";
import type { LedgerRow } from "../schemas/ledger.ts";
import { assertLedgerRow } from "../schemas/ledger.ts";

export async function appendLedgerRows(rows: LedgerRow[], options: { outputDir: string }): Promise<{ ledgerPath: string; rowsWritten: number }> {
  await mkdir(options.outputDir, { recursive: true });
  const ledgerPath = join(options.outputDir, "scout-ledger.jsonl");
  const body = rows.map((row) => {
    assertLedgerRow(row);
    return JSON.stringify(row);
  }).join("\n") + "\n";
  await appendFile(ledgerPath, body, "utf8");
  return { ledgerPath, rowsWritten: rows.length };
}
