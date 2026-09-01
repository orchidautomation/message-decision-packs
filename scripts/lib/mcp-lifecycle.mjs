export const MCP_PROTOCOL_VERSION = '2025-06-18'
export const MCP_MAX_CONCURRENT_TOOL_CALLS = 2
export const MCP_MAX_QUEUED_TOOL_CALLS = 16

export const mcpDiagnostic = ({
  code,
  phase,
  retryable = false,
  nextAction,
}) => ({
  contract: 'mdp.mcp-diagnostic.v1',
  code,
  phase,
  retryable,
  next_action: nextAction,
})

export const protocolVersionError = (requested) => ({
  code: -32602,
  message: 'Unsupported MCP protocol version',
  data: {
    ...mcpDiagnostic({
      code: 'mcp-protocol-version-unsupported',
      phase: 'initialize',
      nextAction: `Reconnect using MCP protocol ${MCP_PROTOCOL_VERSION}.`,
    }),
    supported_protocol_versions: [MCP_PROTOCOL_VERSION],
    requested_protocol_version: typeof requested === 'string' ? '<unsupported>' : null,
  },
})

export const validateProtocolVersion = (params) => {
  const requested = params?.protocolVersion ?? MCP_PROTOCOL_VERSION
  return requested === MCP_PROTOCOL_VERSION ? null : protocolVersionError(requested)
}

export const toolErrorDiagnostic = (code) => {
  const phase = code === 'cli-cancelled' ? 'cancellation'
    : code === 'cli-timeout' ? 'execution'
      : code === 'cli-output-limit' ? 'transport'
        : code === 'mcp-cleanup-incomplete' ? 'cleanup'
          : code?.includes('verify') ? 'verify'
            : 'tool-call'
  const retryable = ['cli-timeout', 'cli-unavailable'].includes(code)
  const nextAction = code === 'cli-cancelled'
    ? 'Start a new request only if the operation is still needed.'
    : code === 'mcp-cleanup-incomplete'
      ? 'Inspect the named approved output locations before retrying.'
      : retryable
        ? 'Retry once after checking the local CLI and configured deadline.'
        : 'Correct the request or inspect the local CLI result before retrying.'
  return mcpDiagnostic({ code: code || 'mcp-tool-error', phase, retryable, nextAction })
}

export const safeMcpMessage = (value, fallback = 'MCP request failed') => {
  const bounded = String(value || fallback).slice(0, 512)
  return bounded
    .replace(/(^|[\s("'=])\/(?:[^\s,;:)"']+\/?)+/gu, '$1<local-path>')
    .replace(/\b[A-Za-z]:\\(?:[^\s,;:)"']+\\?)+/gu, '<local-path>')
}

export const mcpRequestKey = (id) => `${id === null ? 'null' : typeof id}:${JSON.stringify(id)}`

export const createBoundedToolScheduler = ({
  maxConcurrent = MCP_MAX_CONCURRENT_TOOL_CALLS,
  maxQueued = MCP_MAX_QUEUED_TOOL_CALLS,
  busyResponse,
  cancelledResponse = () => undefined,
}) => {
  let running = 0
  const queued = []
  const queuedIds = new Set()

  const drain = () => {
    while (running < maxConcurrent && queued.length > 0) {
      const item = queued.shift()
      if (!queued.some((candidate) => candidate.key === item.key)) queuedIds.delete(item.key)
      running += 1
      let result
      try {
        result = item.operation()
      } catch (error) {
        result = Promise.reject(error)
      }
      Promise.resolve(result)
        .then(item.resolve, item.reject)
        .finally(() => {
          running -= 1
          drain()
        })
    }
  }

  const schedule = (id, operation) => {
    if (running >= maxConcurrent && queued.length >= maxQueued) {
      return Promise.resolve(busyResponse(id))
    }
    return new Promise((resolve, reject) => {
      const key = mcpRequestKey(id)
      queued.push({ id, key, operation, resolve, reject })
      queuedIds.add(key)
      drain()
    })
  }

  const cancelQueued = (id) => {
    const key = mcpRequestKey(id)
    const index = queued.findIndex((item) => item.key === key)
    if (index < 0) return false
    const [item] = queued.splice(index, 1)
    if (!queued.some((candidate) => candidate.key === key)) queuedIds.delete(key)
    item.resolve(cancelledResponse(item.id))
    return true
  }

  return {
    schedule,
    cancelQueued,
    isQueued: (id) => queuedIds.has(mcpRequestKey(id)),
    limits: Object.freeze({ maxConcurrent, maxQueued }),
  }
}
