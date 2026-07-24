#!/usr/bin/env node
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { buildProposalReadinessReport } from './lib/proposal-readiness-report.mjs'

const fixtureFile = (root, name, value) => {
  const path = join(root, name)
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
  return path
}

test('audit-grade native evidence produces a ready report with anchored confidence', () => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-readiness-ready-'))
  const validation = { valid: true, issues: [] }
  const receipt = {
    decision: 'audit-grade',
    valid: true,
    issues: [],
    runner: { assurance: 'stateless-api-verified' },
  }
  const runnerAudit = { runner: 'native-api', tool_invocations_observed: 0 }
  const sourceIntake = {
    entries: [{ state: 'approved', approval_class: 'operator-approved' }],
  }
  const paths = {
    validation: fixtureFile(root, 'validation.json', validation),
    receipt: fixtureFile(root, 'receipt.json', receipt),
    runnerAudit: fixtureFile(root, 'runner-audit.json', runnerAudit),
    sourceIntake: fixtureFile(root, 'source-intake.json', sourceIntake),
  }
  const report = buildProposalReadinessReport({
    result: {
      mode: 'native',
      decision: 'audit-grade',
      audit_grade_eligible: true,
      runner_assurance: 'stateless-api-verified',
      steps: [],
    },
    validation,
    receipt,
    runnerAudit,
    sourceIntake,
    paths,
  })

  assert.equal(report.contract, 'mdp.proposal-readiness-report.v0')
  assert.equal(report.readiness.status, 'ready')
  assert.equal(report.readiness.audit_grade, true)
  assert.equal(report.findings.length, 0)
  assert.equal(report.confidence.level, 'high')
  assert.equal(report.confidence.anchor_ids.length, 4)
  assert.ok(report.anchors.every((anchor) => /^[a-f0-9]{64}$/.test(anchor.sha256)))
})

test('blocked evidence produces deterministic structured findings', () => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-readiness-blocked-'))
  const validation = {
    valid: false,
    issues: [
      {
        code: 'prompt_output_source_ref_missing',
        severity: 'error',
        message: 'missing source ref',
        path: 'output.json#/signals/0/source',
      },
    ],
  }
  const receipt = {
    decision: 'blocked',
    valid: false,
    issues: [
      {
        code: 'prompt_output_validation_failed',
        severity: 'error',
        message: 'validation failed',
        path: 'validation.valid',
      },
    ],
    runner: { assurance: 'invalid' },
  }
  const paths = {
    validation: fixtureFile(root, 'validation.json', validation),
    receipt: fixtureFile(root, 'receipt.json', receipt),
  }
  const report = buildProposalReadinessReport({
    result: {
      mode: 'mock',
      decision: 'blocked',
      audit_grade_eligible: false,
      runner_assurance: 'invalid',
      steps: [
        {
          name: 'mdp_review_proposal',
          status: 'skipped',
          reason: 'run receipt decision blocked is not acceptable for review probes',
        },
      ],
    },
    validation,
    receipt,
    runnerAudit: null,
    sourceIntake: { entries: [{ state: 'candidate', approval_class: 'candidate' }] },
    paths,
  })

  assert.equal(report.readiness.status, 'blocked')
  assert.equal(report.readiness.audit_grade, false)
  assert.equal(report.confidence.level, 'low')
  assert.deepEqual(
    report.findings.map((finding) => finding.code),
    [
      'prompt_output_source_ref_missing',
      'prompt_output_validation_failed',
      'source_intake_not_approved',
      'review_probe_skipped',
      'non_native_evidence',
    ],
  )
  assert.ok(
    report.findings.every(
      (finding) =>
        finding.confidence.level &&
        Array.isArray(finding.confidence.anchor_ids) &&
        finding.confidence.basis,
    ),
  )
})
