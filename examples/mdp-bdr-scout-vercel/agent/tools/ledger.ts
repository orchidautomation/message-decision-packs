import { appendLedgerRows } from "../../src/storage/ledger.ts";
import type { LedgerRow } from "../../src/schemas/ledger.ts";

export async function writeLedgerRows(input: { rows: LedgerRow[]; outputDir?: string }) {
  return appendLedgerRows(input.rows, { outputDir: input.outputDir ?? "artifacts" });
}
