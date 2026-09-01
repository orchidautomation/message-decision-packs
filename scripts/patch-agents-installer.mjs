#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const installerPath = resolve(process.argv[2] ?? '')
if (!process.argv[2]) {
  console.error('Usage: patch-agents-installer.mjs PATH_TO_INSTALL_AGENTS_SH')
  process.exit(2)
}

const source = readFileSync(installerPath, 'utf8')
const broken = ".split(/\n/).filter(Boolean).map((line) =>"
const repaired = ".split(/\\r?\\n/u).filter(Boolean).map((line) =>"
const brokenMatches = source.split(broken).length - 1
const repairedMatches = source.split(repaired).length - 1

if (brokenMatches === 0 && repairedMatches === 1) process.exit(0)
if (brokenMatches !== 1 || repairedMatches !== 0) {
  console.error(
    `Expected exactly one known Pluxx install-results splitter; found malformed=${brokenMatches}, repaired=${repairedMatches}: ${installerPath}`,
  )
  process.exit(1)
}

writeFileSync(installerPath, source.replace(broken, repaired), { mode: 0o755 })
