import type { LedgerRow } from "../schemas/ledger.ts";

export async function persistLedgerRowsToNeon(_rows: LedgerRow[]): Promise<void> {
  throw new Error("Neon persistence is not wired in the offline scaffold. Use appendLedgerRows for dry runs.");
}
