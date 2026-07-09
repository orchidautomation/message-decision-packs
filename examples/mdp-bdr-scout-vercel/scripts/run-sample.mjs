import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
import { runScoutCycle } from '../src/scout/run-scout-cycle.ts';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = resolve(root, '../..');
const native = process.argv.includes('--native');

if (native) {
  process.env.MDP_RUNNER_MODE = process.env.MDP_RUNNER_MODE ?? 'native';
  process.env.MDP_PACK_DIR = process.env.MDP_PACK_DIR ?? repoRoot;
  process.env.MDP_BIN = process.env.MDP_BIN ?? 'mdp';
}

const result = await runScoutCycle({
  fixturePath: join(root, 'samples/public-source-fixture.json'),
  outputDir: join(root, 'artifacts'),
  dryRun: !native,
  persist: true
});

console.log(JSON.stringify({
  ok: true,
  mode: native ? 'native' : 'simulated',
  runId: result.runId,
  qualified: result.qualified.length,
  ledgerPath: result.ledgerPath,
  sourceStrategy: result.sourceStrategy,
  query: result.query,
  firstCandidate: result.qualified[0]?.candidate.company ?? null,
  firstScore: result.qualified[0]?.score.overall ?? null,
  firstMdpStatus: result.qualified[0]?.mdp.fit_status ?? null,
  firstMdpRoute: result.qualified[0]?.mdp.route ?? null
}, null, 2));
