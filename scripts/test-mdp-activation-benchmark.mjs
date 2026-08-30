#!/usr/bin/env node
// MDP activation benchmark: measure warm-unchanged compact-call overhead.
//
// Usage: test-mdp-activation-benchmark.mjs --root <path> \\
//        --workspace <abs-path> --cache <abs-path> --out <abs-path> \\
//        --iterations <int>
//
// Output: writes a JSON file with iterations, p10/p25/p50/p75/p95 in
// milliseconds, plus environment details.

import { spawnSync } from 'node:child_process'
import { writeFileSync } from 'node:fs'
import { cpus, hostname } from 'node:os'

const params = new Map()
const argv = process.argv.slice(2)
for (let i = 0; i < argv.length; i++) {
  const arg = argv[i]
  if (!arg.startsWith('--')) continue
  const eq = arg.indexOf('=')
  if (eq > 0) {
    params.set(arg.slice(2, eq), arg.slice(eq + 1))
    continue
  }
  const next = argv[i + 1]
  if (next !== undefined && !next.startsWith('--')) {
    params.set(arg.slice(2), next)
    i += 1
    continue
  }
  params.set(arg.slice(2), 'true')
}

const ROOT = params.get('root') || ''
const WORKSPACE = params.get('workspace') || ''
const CACHE = params.get('cache') || ''
const OUT = params.get('out') || ''
const ITERATIONS = parseInt(params.get('iterations') || '50', 10)

if (!ROOT || !WORKSPACE || !CACHE || !OUT) {
  process.stderr.write('missing required flags (--root --workspace --cache --out)\n')
  process.exit(2)
}

const samples = []
const stdoutLengths = []

for (let i = 0; i < ITERATIONS; i++) {
  const t0 = process.hrtime.bigint()
  const result = spawnSync('bash', [
    `${ROOT}/scripts/mdp-activate.sh`,
    '--mode=compact',
    `--workspace=${WORKSPACE}`,
    `--plugin-root=${ROOT}`,
    '--session-id=benchmark-session',
  ], {
    env: { ...process.env, MDP_ACTIVATION_CACHE_ROOT: CACHE },
    encoding: 'utf8',
  })
  const t1 = process.hrtime.bigint()
  samples.push(Number(t1 - t0) / 1e6)
  stdoutLengths.push(result.stdout.length)
}

samples.sort((a, b) => a - b)

const at = (fraction) => samples[Math.min(samples.length - 1, Math.floor(samples.length * fraction))]

const payload = {
  iterations: samples.length,
  p10_ms: Number(at(0.10).toFixed(3)),
  p25_ms: Number(at(0.25).toFixed(3)),
  p50_ms: Number(at(0.50).toFixed(3)),
  p75_ms: Number(at(0.75).toFixed(3)),
  p95_ms: Number(at(0.95).toFixed(3)),
  mean_ms: Number((samples.reduce((s, v) => s + v, 0) / samples.length).toFixed(3)),
  min_ms: Number(samples[0].toFixed(3)),
  max_ms: Number(samples[samples.length - 1].toFixed(3)),
  stdout_avg_bytes: Number((stdoutLengths.reduce((s, v) => s + v, 0) / stdoutLengths.length).toFixed(3)),
  shell: process.env.SHELL_BANNER || '',
  node_version: process.version,
  os: process.platform,
  arch: process.arch,
  cpus: cpus().length,
  hostname: hostname(),
  argv0: process.argv0,
}
writeFileSync(OUT, `${JSON.stringify(payload, null, 2)}\n`)

process.stdout.write(`MDP-281 benchmark p50=${payload.p50_ms}ms p95=${payload.p95_ms}ms (n=${payload.iterations})\n`)