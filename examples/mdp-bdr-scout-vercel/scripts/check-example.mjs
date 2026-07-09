import { access, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { assertCandidateWithEvidence } from '../src/schemas/candidate.ts';
import { runScoutCycle } from '../src/scout/run-scout-cycle.ts';
import { assertSourceStrategy, loadSourceStrategy, summarizeSourceStrategy } from '../src/scout/source-strategy.ts';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const required = [
  'README.md',
  'package.json',
  'vercel.json',
  'app/api/cron/scout/route.ts',
  'app/api/readable/[runId]/route.ts',
  'app/api/runs/[runId]/route.ts',
  'workflows/scout-cycle.ts',
  'agent/agent.ts',
  'agent/instructions.md',
  'src/mdp/runner.ts',
  'src/mdp/native-runner.ts',
  'src/providers/exa.ts',
  'src/providers/firecrawl.ts',
  'src/providers/apify.ts',
  'src/schemas/ledger.ts',
  'samples/public-source-fixture.json',
  'samples/candidate-ledger-row.json',
  'samples/source-strategy.json'
];

for (const file of required) {
  await access(join(root, file));
}

const fixture = JSON.parse(await readFile(join(root, 'samples/public-source-fixture.json'), 'utf8'));
assertCandidateWithEvidence(fixture);

const sourceStrategyJson = JSON.parse(await readFile(join(root, 'samples/source-strategy.json'), 'utf8'));
assertSourceStrategy(sourceStrategyJson);
const sourceStrategy = await loadSourceStrategy(join(root, 'samples/source-strategy.json'));
const strategySummary = summarizeSourceStrategy(sourceStrategy);
if (!strategySummary.queries.includes('exa-ai-gtm-agent-triggers')) throw new Error('source strategy must include the Exa trigger query');
if (!sourceStrategy.agent_operating_plan?.downstream_handoff_prompt.includes('mdp check-claims')) {
  throw new Error('source strategy must include downstream mdp check-claims handoff language');
}
for (const query of sourceStrategy.queries_prompts) {
  if (!query.agent_instruction || query.construction_rules.length === 0) {
    throw new Error(`source strategy query ${query.id} must include agent instructions and construction rules`);
  }
}

const result = await runScoutCycle({
  fixturePath: join(root, 'samples/public-source-fixture.json'),
  dryRun: true,
  persist: false
});

if (result.qualified.length !== 1) throw new Error(`expected 1 qualified fixture row, got ${result.qualified.length}`);
if (result.qualified[0].score.overall < 70) throw new Error('sample score should pass default threshold');
if (result.sourceStrategy.query_id !== 'exa-ai-gtm-agent-triggers') throw new Error('sample should use the default MDP source strategy query');
if (result.qualified[0].source_strategy.query_id !== 'exa-ai-gtm-agent-triggers') throw new Error('ledger row should preserve source strategy trace');

console.log('ok mdp-bdr-scout-vercel structural check passed');
