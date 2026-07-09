import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';

const input = await readFile(new URL('../sample-webhook.json', import.meta.url), 'utf8');
const runner = process.platform === 'win32' ? 'npx.cmd' : 'npx';
const child = spawn(runner, ['flue', 'run', 'workflow:draft-response', '--input', input], {
  stdio: 'inherit',
});

child.on('exit', (code) => {
  process.exit(code ?? 1);
});
