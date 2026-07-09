import { start } from "workflow/api";
import { scoutCycleWorkflow } from "../../../../workflows/scout-cycle.ts";

export async function GET(request: Request): Promise<Response> {
  const secret = process.env.CRON_SECRET;
  if (!secret) {
    return Response.json({ ok: false, error: "cron_secret_required" }, { status: 401 });
  }

  const expected = `Bearer ${secret}`;
  if (request.headers.get("authorization") !== expected) {
    return Response.json({ ok: false, error: "unauthorized" }, { status: 401 });
  }

  const url = new URL(request.url);
  const dryRunParam = url.searchParams.get("dryRun");
  const dryRun = dryRunParam === "true" || (dryRunParam !== "false" && process.env.EXA_API_KEY == null);
  const run = await start(scoutCycleWorkflow, [{
    packId: process.env.MDP_PACK_ID ?? "mdp-for-mdp",
    scheduleId: process.env.SCOUT_SCHEDULE_ID ?? "weekday-default",
    query: process.env.SCOUT_QUERY,
    sourceStrategyPath: process.env.SCOUT_SOURCE_STRATEGY_PATH,
    fixturePath: process.env.SCOUT_FIXTURE_PATH,
    outputDir: process.env.SCOUT_OUTPUT_DIR ?? "/tmp/mdp-bdr-scout",
    dryRun,
    persist: true
  }]);

  return Response.json({ ok: true, runId: run.runId, statusUrl: `/api/runs/${run.runId}`, streamUrl: `/api/readable/${run.runId}` }, { status: 202 });
}
