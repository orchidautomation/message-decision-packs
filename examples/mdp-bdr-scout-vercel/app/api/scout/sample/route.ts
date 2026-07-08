import { runScoutCycle } from "../../../../src/scout/run-scout-cycle.ts";

export async function GET(): Promise<Response> {
  const result = await runScoutCycle({ dryRun: true, persist: false });
  return Response.json({ ok: true, ...result });
}
