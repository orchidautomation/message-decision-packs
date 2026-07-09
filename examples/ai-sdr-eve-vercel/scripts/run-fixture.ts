import { runFixtureScoutCycle } from "../agent/lib/scout-cycle.ts";

const result = await runFixtureScoutCycle();
const first = result.rows[0];
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
  firstSignalReasons: first?.source_strategy.qualified_signal_reasons ?? [],
  firstMdpStatus: first?.mdp.fit_status ?? null,
  firstMdpRoute: first?.mdp.route ?? null
}, null, 2));
