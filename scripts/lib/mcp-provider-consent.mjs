import { createHash } from 'node:crypto'
import { existsSync, lstatSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { boundedDenial } from './mcp-path-policy.mjs'

export const CONSENT_CONTRACT = 'mdp.mcp-provider-consent.v1'
const consumed = new Set()
const digest = (value) => createHash('sha256').update(JSON.stringify(value)).digest('hex')

export const consentBinding = ({ provider, purpose, requestSha256, sourceSha256s = [], outputRoot, expiresAt, nonce }) => digest({ contract: CONSENT_CONTRACT, provider, purpose, request_sha256: requestSha256, source_sha256s: sourceSha256s, output_root: outputRoot, expires_at: expiresAt, nonce })

export const consumeProviderConsent = ({ policy, consentId, provider, purpose, requestSha256, sourceSha256s = [], outputRoot, now = Date.now() }) => {
  if (typeof consentId !== 'string' || !/^[A-Za-z0-9._-]{1,128}$/.test(consentId)) throw Object.assign(new Error('consent identifier is invalid'), { code: 'mcp-consent-denied' })
  if (consumed.has(consentId)) throw Object.assign(new Error('consent has already been consumed'), { code: 'mcp-consent-replayed' })
  const selected = policy.existing('consent', join(policy.roots.consent[0], `${consentId}.json`), 'file')
  if (!existsSync(selected.path) || lstatSync(selected.path).isSymbolicLink()) throw Object.assign(new Error('consent record is unavailable'), { code: 'mcp-consent-denied' })
  let record
  try { record = JSON.parse(readFileSync(selected.path, 'utf8')) } catch { throw Object.assign(new Error('consent record is invalid'), { code: 'mcp-consent-denied' }) }
  const required = ['contract', 'provider', 'purpose', 'request_sha256', 'source_sha256s', 'output_root', 'expires_at', 'nonce', 'binding_sha256']
  if (Object.keys(record).some((key) => !required.includes(key)) || required.some((key) => !(key in record)) || record.contract !== CONSENT_CONTRACT || record.provider !== provider || record.purpose !== purpose || record.request_sha256 !== requestSha256 || JSON.stringify(record.source_sha256s) !== JSON.stringify(sourceSha256s) || record.output_root !== outputRoot || typeof record.nonce !== 'string' || typeof record.binding_sha256 !== 'string') throw Object.assign(new Error('consent binding does not match request'), { code: 'mcp-consent-mismatch' })
  const expiry = Date.parse(record.expires_at)
  if (!Number.isFinite(expiry) || expiry <= now) throw Object.assign(new Error('consent has expired'), { code: 'mcp-consent-expired' })
  if (consentBinding({ provider, purpose, requestSha256, sourceSha256s, outputRoot, expiresAt: record.expires_at, nonce: record.nonce }) !== record.binding_sha256 && record.binding_sha256 !== undefined) throw Object.assign(new Error('consent binding is invalid'), { code: 'mcp-consent-mismatch' })
  consumed.add(consentId)
  return { consent_id: consentId, nonce: record.nonce, binding_sha256: consentBinding({ provider, purpose, requestSha256, sourceSha256s, outputRoot, expiresAt: record.expires_at, nonce: record.nonce }), output_root: outputRoot }
}

export { boundedDenial }
