import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { runScoutCycle } from '../src/scout/run-scout-cycle.ts';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const result = await runScoutCycle({
  fixturePath: join(root, 'samples/public-source-fixture.json'),
  outputDir: join(root, 'artifacts'),
  dryRun: true,
  persist: true
});

console.log(JSON.stringify({
  ok: true,
  runId: result.runId,
  qualified: result.qualified.length,
  ledgerPath: result.ledgerPath,
  firstCandidate: result.qualified[0]?.candidate.company ?? null,
  firstScore: result.qualified[0]?.score.overall ?? null
}, null, 2));
