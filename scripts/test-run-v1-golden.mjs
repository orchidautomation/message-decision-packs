#!/usr/bin/env node
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const fixturePath = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'cli',
  'tests',
  'fixtures',
  'run-v1',
  'canonical-json-vectors.json',
)
const vectors = JSON.parse(readFileSync(fixturePath, 'utf8'))
const MAX_SAFE_INTEGER = 9_007_199_254_740_991n

class AuthorityJsonError extends Error {
  constructor(category, message) {
    super(message)
    this.category = category
  }
}

class AuthorityJsonParser {
  constructor(raw, limits) {
    this.raw = raw
    this.limits = limits
    this.index = 0
  }

  parse() {
    if (Buffer.byteLength(this.raw, 'utf8') > this.limits.max_bytes) {
      throw new AuthorityJsonError('byte_limit', 'authority JSON exceeds byte limit')
    }
    this.skipWhitespace()
    const value = this.parseValue(0)
    this.skipWhitespace()
    if (this.index !== this.raw.length) {
      throw new AuthorityJsonError('invalid_json', 'authority JSON has trailing data')
    }
    return value
  }

  parseValue(depth) {
    if (depth > this.limits.max_depth) {
      throw new AuthorityJsonError('depth_limit', 'authority JSON exceeds nesting-depth limit')
    }
    const char = this.raw[this.index]
    if (char === '{') return this.parseObject(depth)
    if (char === '[') return this.parseArray(depth)
    if (char === '"') return this.parseString()
    if (char === '-' || (char >= '0' && char <= '9')) return this.parseNumber()
    for (const [token, value] of [['true', true], ['false', false], ['null', null]]) {
      if (this.raw.startsWith(token, this.index)) {
        this.index += token.length
        return value
      }
    }
    throw new AuthorityJsonError('invalid_json', `unexpected token at character ${this.index}`)
  }

  parseObject(depth) {
    this.index += 1
    this.skipWhitespace()
    const entries = []
    const keys = new Set()
    if (this.raw[this.index] === '}') {
      this.index += 1
      return { type: 'object', entries }
    }
    while (true) {
      if (entries.length >= this.limits.max_object_members) {
        throw new AuthorityJsonError('object_limit', 'authority JSON object exceeds member limit')
      }
      if (this.raw[this.index] !== '"') {
        throw new AuthorityJsonError('invalid_json', 'object key must be a JSON string')
      }
      const key = this.parseString()
      if (keys.has(key)) {
        throw new AuthorityJsonError('duplicate_member', `duplicate object member: ${key}`)
      }
      keys.add(key)
      this.skipWhitespace()
      this.expect(':')
      this.skipWhitespace()
      entries.push([key, this.parseValue(depth + 1)])
      this.skipWhitespace()
      if (this.raw[this.index] === '}') {
        this.index += 1
        return { type: 'object', entries }
      }
      this.expect(',')
      this.skipWhitespace()
    }
  }

  parseArray(depth) {
    this.index += 1
    this.skipWhitespace()
    const values = []
    if (this.raw[this.index] === ']') {
      this.index += 1
      return values
    }
    while (true) {
      const value = this.parseValue(depth + 1)
      if (values.length >= this.limits.max_array_length) {
        throw new AuthorityJsonError('array_limit', 'authority JSON array exceeds length limit')
      }
      values.push(value)
      this.skipWhitespace()
      if (this.raw[this.index] === ']') {
        this.index += 1
        return values
      }
      this.expect(',')
      this.skipWhitespace()
    }
  }

  parseString() {
    const start = this.index
    this.index += 1
    let escaped = false
    while (this.index < this.raw.length) {
      const code = this.raw.charCodeAt(this.index)
      const char = this.raw[this.index]
      if (!escaped && char === '"') {
        this.index += 1
        let value
        try {
          value = JSON.parse(this.raw.slice(start, this.index))
        } catch {
          throw new AuthorityJsonError('invalid_json', 'invalid JSON string')
        }
        if (containsUnpairedSurrogate(value)) {
          throw new AuthorityJsonError('invalid_unicode', 'authority JSON contains unpaired surrogate')
        }
        if (Buffer.byteLength(value, 'utf8') > this.limits.max_string_bytes) {
          throw new AuthorityJsonError('string_limit', 'authority JSON string exceeds byte limit')
        }
        return value
      }
      if (!escaped && code < 0x20) {
        throw new AuthorityJsonError('invalid_json', 'unescaped control character in string')
      }
      if (!escaped && char === '\\') {
        escaped = true
      } else {
        escaped = false
      }
      this.index += 1
    }
    throw new AuthorityJsonError('invalid_json', 'unterminated JSON string')
  }

  parseNumber() {
    const rest = this.raw.slice(this.index)
    const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(rest)
    if (!match) throw new AuthorityJsonError('invalid_json', 'invalid JSON number')
    const token = match[0]
    this.index += token.length
    if (token === '-0') throw new AuthorityJsonError('negative_zero', 'negative zero is forbidden')
    if (token.includes('.') || token.includes('e') || token.includes('E')) {
      throw new AuthorityJsonError('floating_point', 'floating-point numbers are forbidden')
    }
    const value = BigInt(token)
    if (value > MAX_SAFE_INTEGER || value < -MAX_SAFE_INTEGER) {
      throw new AuthorityJsonError('unsafe_integer', 'integer is outside the safe range')
    }
    return value
  }

  skipWhitespace() {
    while (' \t\r\n'.includes(this.raw[this.index] ?? '\0')) this.index += 1
  }

  expect(char) {
    if (this.raw[this.index] !== char) {
      throw new AuthorityJsonError('invalid_json', `expected ${char} at character ${this.index}`)
    }
    this.index += 1
  }
}

function containsUnpairedSurrogate(value) {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      if (!(next >= 0xdc00 && next <= 0xdfff)) return true
      index += 1
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return true
    }
  }
  return false
}

function compareUnicodeScalars(left, right) {
  const leftPoints = Array.from(left, (char) => char.codePointAt(0))
  const rightPoints = Array.from(right, (char) => char.codePointAt(0))
  const length = Math.min(leftPoints.length, rightPoints.length)
  for (let index = 0; index < length; index += 1) {
    if (leftPoints[index] !== rightPoints[index]) return leftPoints[index] - rightPoints[index]
  }
  return leftPoints.length - rightPoints.length
}

function canonicalJson(value) {
  if (typeof value === 'bigint') return value.toString()
  if (value === null || typeof value === 'boolean' || typeof value === 'string') {
    return JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (value?.type === 'object') {
    return `{${value.entries
      .slice()
      .sort(([left], [right]) => compareUnicodeScalars(left, right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonicalJson(item)}`)
      .join(',')}}`
  }
  throw new Error(`unsupported canonical JSON value: ${typeof value}`)
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

assert.equal(vectors.contract, 'mdp.authority-json-golden-vectors.v1')

for (const vector of vectors.accepted) {
  const limits = { ...vectors.default_limits, ...(vector.limits ?? {}) }
  const parsed = new AuthorityJsonParser(vector.raw_json, limits).parse()
  const canonical = canonicalJson(parsed)
  const rawBytes = Buffer.from(vector.raw_json, 'utf8')
  const canonicalBytes = Buffer.from(canonical, 'utf8')
  assert.equal(rawBytes.length, vector.raw_byte_count, `${vector.id}: raw byte count`)
  assert.equal(sha256(rawBytes), vector.raw_utf8_sha256, `${vector.id}: raw hash`)
  assert.equal(canonical, vector.canonical_json, `${vector.id}: canonical JSON`)
  assert.equal(sha256(canonicalBytes), vector.canonical_utf8_sha256, `${vector.id}: canonical hash`)
  assert.equal(
    sha256(Buffer.concat([Buffer.from(vector.domain, 'ascii'), Buffer.from([0]), canonicalBytes])),
    vector.domain_sha256,
    `${vector.id}: domain hash`,
  )
}

for (const vector of vectors.rejected) {
  const limits = { ...vectors.default_limits, ...(vector.limits ?? {}) }
  assert.throws(
    () => new AuthorityJsonParser(vector.raw_json, limits).parse(),
    (error) => error instanceof AuthorityJsonError && error.category === vector.error_category,
    `${vector.id}: expected ${vector.error_category}`,
  )
}

const keyOrderVectors = vectors.accepted.filter((vector) => vector.id.startsWith('object-key-order-'))
assert.equal(keyOrderVectors.length, 2)
assert.equal(keyOrderVectors[0].domain_sha256, keyOrderVectors[1].domain_sha256)
assert.notEqual(keyOrderVectors[0].raw_utf8_sha256, keyOrderVectors[1].raw_utf8_sha256)

const arrayOrderVectors = vectors.accepted.filter((vector) => vector.id.startsWith('array-order-'))
assert.equal(arrayOrderVectors.length, 2)
assert.notEqual(arrayOrderVectors[0].domain_sha256, arrayOrderVectors[1].domain_sha256)

console.log(`run-v1 golden vectors passed: ${vectors.accepted.length} accepted, ${vectors.rejected.length} rejected`)
