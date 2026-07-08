import { runScoutCycle } from "../src/scout/run-scout-cycle.ts";

export async function scoutCycleWorkflow(input: { packId?: string; scheduleId?: string } = {}) {
  "use workflow";

  return runScoutCycle({
    packId: input.packId ?? "mdp-for-mdp",
    scheduleId: input.scheduleId ?? "weekday-default",
    dryRun: !process.env.EXA_API_KEY,
    outputDir: process.env.SCOUT_OUTPUT_DIR ?? "/tmp/mdp-bdr-scout"
  });
}
