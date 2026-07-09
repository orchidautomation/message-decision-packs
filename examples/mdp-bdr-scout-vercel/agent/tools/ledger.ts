import { defineTool } from "eve/tools";
import { z } from "zod";
import { appendLedgerRows } from "../../src/storage/ledger.ts";
import type { LedgerRow } from "../../src/schemas/ledger.ts";

export async function writeLedgerRows(input: { rows: LedgerRow[]; outputDir?: string }) {
  return appendLedgerRows(input.rows, { outputDir: input.outputDir ?? "artifacts" });
}

export default defineTool({
  description: "Append normalized scout ledger rows. Outreach stays disabled in this contract.",
  inputSchema: z.object({
    rows: z.array(z.any()).min(1),
    outputDir: z.string().optional()
  }),
  async execute(input) {
    return writeLedgerRows({ rows: input.rows as LedgerRow[], outputDir: input.outputDir });
  }
});
