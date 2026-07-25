import { existsSync } from 'node:fs'

import { sha256File } from './proposal-runner-runtime.mjs'

export const PROPOSAL_READINESS_REPORT_CONTRACT = 'mdp.proposal-readiness-report.v0'

const anchor = (id, kind, path) => {
  if (!path || !existsSync(path)) return null
  return {
    id,
    kind,
    path,
    sha256: sha256File(path),
  }
}

const findingCategory = (code) => {
  if (code.startsWith('source_') || code.includes('source_ref')) return 'evidence'
  if (code.startsWith('runner_') || code === 'non_native_evidence') return 'runner-boundary'
  if (code.startsWith('review_')) return 'review-readiness'
  return 'validation'
}

const issueFinding = (issue, anchorId, index) => ({
  id: `finding-${String(index + 1).padStart(3, '0')}`,
  code: issue.code || 'unknown_issue',
  category: findingCategory(issue.code || ''),
  severity: issue.severity === 'warning' ? 'warning' : 'blocker',
  status: 'open',
  summary: issue.message || issue.code || 'Unknown issue',
  source_path: issue.path || null,
  confidence: {
    level: anchorId ? 'high' : 'medium',
    basis: anchorId
      ? 'The finding is copied from a hash-bound deterministic artifact.'
      : 'The finding is derived from the runner result without a persisted artifact anchor.',
    anchor_ids: anchorId ? [anchorId] : [],
  },
})

export const buildProposalReadinessReport = ({
  result,
  validation,
  receipt,
  runnerAudit,
  sourceIntake,
  paths = {},
}) => {
  const anchors = [
    anchor('validation', 'prompt-output-validation', paths.validation),
    anchor('receipt', 'run-receipt', paths.receipt),
    anchor('runner-audit', 'runner-audit', paths.runnerAudit),
    anchor('source-intake', 'source-intake', paths.sourceIntake),
  ].filter(Boolean)
  const anchorIds = new Set(anchors.map((entry) => entry.id))
  const findings = []

  for (const issue of validation?.issues || []) {
    findings.push(issueFinding(issue, anchorIds.has('validation') ? 'validation' : null, findings.length))
  }
  for (const issue of receipt?.issues || []) {
    findings.push(issueFinding(issue, anchorIds.has('receipt') ? 'receipt' : null, findings.length))
  }

  const intakeApproved =
    Array.isArray(sourceIntake?.entries) &&
    sourceIntake.entries.length > 0 &&
    sourceIntake.entries.every(
      (entry) => entry?.state === 'approved' && entry?.approval_class === 'operator-approved',
    )
  if (!intakeApproved) {
    findings.push(
      issueFinding(
        {
          code: 'source_intake_not_approved',
          severity: 'error',
          message: 'Source intake is missing or contains entries without explicit operator approval.',
          path: 'source_intake.entries',
        },
        anchorIds.has('source-intake') ? 'source-intake' : null,
        findings.length,
      ),
    )
  }

  const skippedReview = (result?.steps || []).find(
    (step) => step?.name === 'mdp_review_proposal' && step?.status === 'skipped',
  )
  if (skippedReview) {
    findings.push(
      issueFinding(
        {
          code: 'review_probe_skipped',
          severity: 'warning',
          message: skippedReview.reason || 'Proposal review probes were skipped.',
          path: 'result.steps',
        },
        null,
        findings.length,
      ),
    )
  }

  if (result?.mode !== 'native') {
    findings.push(
      issueFinding(
        {
          code: 'non_native_evidence',
          severity: 'error',
          message: `${result?.mode || 'unknown'} mode is not real native provider evidence.`,
          path: 'result.mode',
        },
        null,
        findings.length,
      ),
    )
  }

  const auditGrade =
    result?.mode === 'native' &&
    result?.decision === 'audit-grade' &&
    result?.audit_grade_eligible === true &&
    validation?.valid === true &&
    receipt?.valid === true &&
    receipt?.decision === 'audit-grade' &&
    intakeApproved &&
    runnerAudit != null
  const blockers = findings.filter((finding) => finding.severity === 'blocker').length
  const warnings = findings.filter((finding) => finding.severity === 'warning').length
  const confidenceLevel =
    auditGrade && anchors.length === 4
      ? 'high'
      : blockers === 0 && anchors.length >= 2
        ? 'medium'
        : 'low'

  return {
    contract: PROPOSAL_READINESS_REPORT_CONTRACT,
    readiness: {
      status: auditGrade ? 'ready' : blockers > 0 ? 'blocked' : 'advisory',
      audit_grade: auditGrade,
      decision: result?.decision || 'blocked',
      runner_assurance: result?.runner_assurance || receipt?.runner?.assurance || 'unknown',
    },
    summary: {
      blocker_count: blockers,
      warning_count: warnings,
      finding_count: findings.length,
      anchor_count: anchors.length,
    },
    confidence: {
      level: confidenceLevel,
      basis:
        confidenceLevel === 'high'
          ? 'All required deterministic artifacts are hash-bound and the current native receipt is audit-grade.'
          : 'Confidence is bounded by missing, blocked, mock, or incompletely anchored evidence.',
      anchor_ids: anchors.map((entry) => entry.id),
    },
    anchors,
    findings,
    caveats: [
      'Readiness summarizes machine-observed artifact state; it does not certify semantic truth, compliance, legal approval, or proposal submission readiness.',
      'Confidence describes evidence anchoring, not the probability that a proposal claim is true.',
    ],
  }
}
