import assert from 'node:assert/strict'
import test from 'node:test'
import {
  MCP_MAX_CONCURRENT_TOOL_CALLS,
  MCP_MAX_QUEUED_TOOL_CALLS,
  createBoundedToolScheduler,
  mcpRequestKey,
  validateProtocolVersion,
} from './lib/mcp-lifecycle.mjs'

test('protocol negotiation accepts only the declared version', () => {
  assert.equal(validateProtocolVersion({ protocolVersion: '2025-06-18' }), null)
  const refused = validateProtocolVersion({ protocolVersion: '/private/customer/version' })
  assert.equal(refused.code, -32602)
  assert.deepEqual(refused.data.supported_protocol_versions, ['2025-06-18'])
  assert.equal(JSON.stringify(refused).includes('/private/customer'), false)
})

test('request keys preserve JSON-RPC scalar types', () => {
  assert.notEqual(mcpRequestKey(1), mcpRequestKey('1'))
  assert.notEqual(mcpRequestKey(null), mcpRequestKey('null'))
  assert.equal(mcpRequestKey(1), mcpRequestKey(1))
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

test('tool scheduler preserves JSON-RPC ID types when cancelling queued calls', async () => {
  let releaseActive
  const invoked = []
  const scheduler = createBoundedToolScheduler({
    maxConcurrent: 1,
    maxQueued: 2,
    busyResponse: (id) => ({ id, busy: true }),
    cancelledResponse: (id) => ({ id, cancelled: true }),
  })
  const active = scheduler.schedule('active', async () => {
    await new Promise((resolve) => { releaseActive = resolve })
    return { id: 'active' }
  })
  await new Promise((resolve) => setImmediate(resolve))
  const numeric = scheduler.schedule(1, async () => {
    invoked.push(1)
    return { id: 1 }
  })
  const string = scheduler.schedule('1', async () => {
    invoked.push('1')
    return { id: '1' }
  })

  assert.equal(scheduler.isQueued(1), true)
  assert.equal(scheduler.isQueued('1'), true)
  assert.equal(scheduler.cancelQueued('1'), true)
  assert.equal(scheduler.isQueued(1), true)
  assert.equal(scheduler.isQueued('1'), false)
  assert.deepEqual(await string, { id: '1', cancelled: true })

  releaseActive()
  assert.deepEqual(await Promise.all([active, numeric]), [{ id: 'active' }, { id: 1 }])
  assert.deepEqual(invoked, [1])
})

test('queued cancellation resolves immediately and releases scheduler capacity', async () => {
  let releaseActive
  let cancelledInvocations = 0
  const scheduler = createBoundedToolScheduler({
    maxConcurrent: 1,
    maxQueued: 1,
    busyResponse: (id) => ({ id, busy: true }),
    cancelledResponse: (id) => ({ id, cancelled: true }),
  })
  const active = scheduler.schedule('active', async () => {
    await new Promise((resolve) => { releaseActive = resolve })
    return { id: 'active' }
  })
  await new Promise((resolve) => setImmediate(resolve))
  const cancelled = scheduler.schedule('cancel-me', async () => {
    cancelledInvocations += 1
    return { id: 'cancel-me' }
  })

  assert.equal(scheduler.cancelQueued('cancel-me'), true)
  assert.equal(scheduler.cancelQueued('cancel-me'), false)
  assert.deepEqual(await cancelled, { id: 'cancel-me', cancelled: true })
  assert.equal(cancelledInvocations, 0)

  const replacement = scheduler.schedule('replacement', async () => ({ id: 'replacement' }))
  assert.equal(scheduler.isQueued('replacement'), true)
  releaseActive()
  assert.deepEqual(await Promise.all([active, replacement]), [{ id: 'active' }, { id: 'replacement' }])
})
