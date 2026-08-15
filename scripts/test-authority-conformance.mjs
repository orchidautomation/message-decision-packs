#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'

const root = resolve(import.meta.dirname, '..')
const sourceCanonicalPath = resolve(root, 'plugin/assets/authority-conformance/corpus.json')
const packagedCanonicalPath = resolve(root, 'assets/authority-conformance/corpus.json')
const canonicalPath = existsSync(sourceCanonicalPath) ? sourceCanonicalPath : packagedCanonicalPath
const canonicalBytes = readFileSync(canonicalPath)
if (existsSync(sourceCanonicalPath) && existsSync(packagedCanonicalPath)) {
  const mirrorBytes = readFileSync(packagedCanonicalPath)
  if (!canonicalBytes.equals(mirrorBytes)) throw new Error('authority corpus mirror drift')
}

const corpus = JSON.parse(canonicalBytes)
if (corpus.contract !== 'mdp.authority-conformance-corpus.v1') throw new Error('unexpected corpus contract')
if (corpus.oracle !== 'hand-authored') throw new Error('authority corpus must be an independent hand-authored oracle')
if (!Array.isArray(corpus.cases) || corpus.cases.length < 10) throw new Error('authority corpus is incomplete')

const required = new Set([
  'run-success-allow',
  'lifecycle-success-blocked-decision',
  'preflight-refused-block',
  'runner-failed-unavailable',
  'output-invalid-block',
  'decision-invalid-block',
  'audit-incomplete-unavailable',
  'policy-blocked',
  'human-brief-blocked-projection',
  'mcp-canonical-denial-is-data',
  'proposal-completed-remains-blocked',
  'mdp-205-detached-governed-input-unavailable',
  'mdp-208-raw-prompt-output-no-trace-authority',
  'mdp-209-profile-activation-block',
])
const ids = new Set()
for (const entry of corpus.cases) {
  if (!entry || typeof entry !== 'object' || Array.isArray(entry)) throw new Error('corpus cases must be objects')
  if (typeof entry.id !== 'string' || !entry.id) throw new Error('corpus case id is required')
  if (ids.has(entry.id)) throw new Error(`duplicate authority corpus id: ${entry.id}`)
  ids.add(entry.id)
  if (!entry.source || typeof entry.source !== 'object') throw new Error(`${entry.id}: source is required`)
  if (!entry.expected || typeof entry.expected !== 'object') throw new Error(`${entry.id}: expected oracle is required`)
  if (entry.expected.source_disposition === 'block' && entry.expected.governed_generation !== false) {
    throw new Error(`${entry.id}: blocked projection cannot grant governed generation`)
  }
  if (entry.expected.disposition === 'block' && entry.expected.governed_generation !== 'absent') {
    throw new Error(`${entry.id}: blocked source cannot grant governed generation`)
  }
}
for (const id of required) if (!ids.has(id)) throw new Error(`missing authority corpus case: ${id}`)

const sha256 = createHash('sha256').update(canonicalBytes).digest('hex')
process.stdout.write(`${JSON.stringify({ ok: true, contract: corpus.contract, cases: corpus.cases.length, sha256 })}\n`)
