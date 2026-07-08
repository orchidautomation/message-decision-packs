import { runScoutCycle } from "../../../../src/scout/run-scout-cycle.ts";

export async function GET(request: Request): Promise<Response> {
  const secret = process.env.CRON_SECRET;
  if (secret) {
    const expected = `Bearer ${secret}`;
    if (request.headers.get("authorization") !== expected) {
      return Response.json({ ok: false, error: "unauthorized" }, { status: 401 });
    }
  }

  const result = await runScoutCycle({
    dryRun: process.env.NODE_ENV !== "production",
    outputDir: "/tmp/mdp-bdr-scout",
    persist: true
  });

  return Response.json({ ok: true, runId: result.runId, qualified: result.qualified.length, ledgerPath: result.ledgerPath });
}
