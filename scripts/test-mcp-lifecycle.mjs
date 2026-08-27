import assert from 'node:assert/strict'
import test from 'node:test'
import {
  MCP_MAX_CONCURRENT_TOOL_CALLS,
  MCP_MAX_QUEUED_TOOL_CALLS,
  createBoundedToolScheduler,
  validateProtocolVersion,
} from './lib/mcp-lifecycle.mjs'

test('protocol negotiation accepts only the declared version', () => {
  assert.equal(validateProtocolVersion({ protocolVersion: '2025-06-18' }), null)
  const refused = validateProtocolVersion({ protocolVersion: '/private/customer/version' })
  assert.equal(refused.code, -32602)
  assert.deepEqual(refused.data.supported_protocol_versions, ['2025-06-18'])
  assert.equal(JSON.stringify(refused).includes('/private/customer'), false)
})

test('tool scheduler enforces explicit active and queued limits', async () => {
  let running = 0
  let observed = 0
  const releases = []
  const scheduler = createBoundedToolScheduler({
    busyResponse: (id) => ({ id, busy: true }),
  })
  const operations = Array.from(
    { length: MCP_MAX_CONCURRENT_TOOL_CALLS + MCP_MAX_QUEUED_TOOL_CALLS },
    (_, id) => scheduler.schedule(id, async () => {
      running += 1
      observed = Math.max(observed, running)
      await new Promise((resolve) => releases.push(resolve))
      running -= 1
      return { id, busy: false }
    }),
  )
  const refused = await scheduler.schedule('overflow', async () => ({ busy: false }))
  assert.deepEqual(refused, { id: 'overflow', busy: true })
  while (releases.length || running) {
    releases.splice(0).forEach((release) => release())
    await new Promise((resolve) => setImmediate(resolve))
  }
  assert.equal(observed, MCP_MAX_CONCURRENT_TOOL_CALLS)
  assert((await Promise.all(operations)).every((result) => result.busy === false))
})
