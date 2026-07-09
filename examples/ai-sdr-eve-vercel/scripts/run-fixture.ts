import { runFixtureScoutCycle } from "../agent/lib/scout-cycle.ts";

const result = await runFixtureScoutCycle();
const first = result.rows[0];
console.log(JSON.stringify({
  ok: true,
  runId: result.runId,
  qualified: result.qualified,
  ledgerPath: result.ledgerPath,
  query: result.query,
  provider: result.provider,
  fallbackReason: result.fallbackReason,
  firstCandidate: first?.candidate.company ?? null,
  firstScore: first?.score.overall ?? null,
  firstMdpStatus: first?.mdp.fit_status ?? null,
  firstMdpRoute: first?.mdp.route ?? null
}, null, 2));
