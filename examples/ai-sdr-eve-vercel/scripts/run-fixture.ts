import { runFixtureScoutCycle } from "../agent/lib/scout-cycle.ts";

const result = await runFixtureScoutCycle();
const first = result.rows[0];
if (result.qualified !== result.targetQualified || !first || first.mdp.fit_status !== "fit") {
  throw new Error(`Fixture scout must qualify ${result.targetQualified} synthetic row(s) through native MDP fit; received ${result.qualified}.`);
}
console.log(JSON.stringify({
  ok: true,
  runId: result.runId,
  qualified: result.qualified,
  targetQualified: result.targetQualified,
  discoveryPasses: result.discoveryPasses,
  exhausted: result.exhausted,
  ledgerPath: result.ledgerPath,
  query: result.query,
  queries: result.queries,
  provider: result.provider,
  fallbackReason: result.fallbackReason,
  firstCandidate: first?.candidate.company ?? null,
  firstPerson: first?.candidate.name ?? null,
  firstTitle: first?.candidate.title ?? null,
  firstScore: first?.score.overall ?? null,
  firstMdpStatus: first?.mdp.fit_status ?? null,
  firstMdpRoute: first?.mdp.route ?? null
}, null, 2));
