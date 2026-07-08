export type ScoutConfig = {
  packId: string;
  scheduleId: string;
  minScore: number;
  maxCandidates: number;
  crmSyncEnabled: boolean;
};

export function loadScoutConfig(overrides: Partial<ScoutConfig> = {}): ScoutConfig {
  return {
    packId: overrides.packId ?? process.env.MDP_PACK_ID ?? "mdp-for-mdp",
    scheduleId: overrides.scheduleId ?? process.env.SCOUT_SCHEDULE_ID ?? "weekday-default",
    minScore: overrides.minScore ?? Number(process.env.SCOUT_MIN_SCORE ?? 70),
    maxCandidates: overrides.maxCandidates ?? Number(process.env.SCOUT_MAX_CANDIDATES ?? 25),
    crmSyncEnabled: overrides.crmSyncEnabled ?? process.env.CRM_SYNC_ENABLED === "true"
  };
}
