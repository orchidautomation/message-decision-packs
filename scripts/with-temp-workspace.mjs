#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { cleanupOwnedTempWorkspace, cleanupStaleOwnedTempWorkspaces, createOwnedTempWorkspace } from './lib/temp-workspace.mjs'

const separator = process.argv.indexOf('--')
if (separator < 0 || separator === process.argv.length - 1) {
  console.error('usage: with-temp-workspace.mjs --purpose PURPOSE -- COMMAND [ARG...]')
  process.exit(2)
}
const options = process.argv.slice(2, separator)
const purposeIndex = options.indexOf('--purpose')
if (purposeIndex < 0 || !options[purposeIndex + 1]) {
  console.error('--purpose is required')
  process.exit(2)
}
const purpose = options[purposeIndex + 1]
cleanupStaleOwnedTempWorkspaces({ purpose })
const root = createOwnedTempWorkspace({ purpose })
const [command, ...args] = process.argv.slice(separator + 1)
const child = spawn(command, args, {
  stdio: 'inherit',
  env: { ...process.env, MDP_TEMP_ROOT: root, MDP_TEMP_WORKSPACE_ACTIVE: '1' },
})

let forwardedSignal = null
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(signal, () => {
    forwardedSignal = signal
    if (!child.killed) child.kill(signal)
  })
}
child.once('error', (error) => {
  cleanupOwnedTempWorkspace(root, { purpose })
  console.error(`unable to start validation command: ${error.message}`)
  process.exit(127)
})
child.once('exit', (code, signal) => {
  cleanupOwnedTempWorkspace(root, { purpose })
  if (forwardedSignal || signal) {
    const terminalSignal = forwardedSignal || signal
    process.removeAllListeners(terminalSignal)
    process.kill(process.pid, terminalSignal)
    return
  }
  process.exit(code ?? 1)
})
