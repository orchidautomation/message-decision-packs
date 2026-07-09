import type { Candidate, EvidenceSource, MdpDecision, ScoreBreakdown } from "./schemas.ts";

export type QualificationValidation = {
  ok: boolean;
  reasons: string[];
  personEvidenceIds: string[];
  personResolutionStatus: "resolved" | "not_found";
};

export type QualificationInput = {
  candidate: Candidate;
  evidence: EvidenceSource[];
  mdp: MdpDecision;
  score: ScoreBreakdown;
  minScore: number;
};

const BLOCKED_EVIDENCE_HOSTS = /(apollo|rocketreach|signalhire|lusha|hunter\.io|zoominfo|contactout|email-format|leadiq|seamless)\./i;
const BLOCKED_PERSON_SOURCE_WORDS = /\b(email|phone|mobile|contact database|directory|salary|jobs?|hiring|careers|login|required|private)\b/i;
const BLOCKED_SOCIAL_HOSTS = /\b(instagram|facebook|threads|twitter|x|tiktok|youtube|pinterest)\.com$/i;

export function validateQualifiedCandidate(input: QualificationInput): QualificationValidation {
  const reasons: string[] = [];
  const minScore = normalizeScoreThreshold(input.minScore);
  const personEvidence = findPersonResolutionEvidence(input.evidence, input.candidate);

  if (input.mdp.fit_status !== "fit") reasons.push(`MDP fit_status must be fit; received ${input.mdp.fit_status}.`);
  if (input.mdp.gaps.length > 0) reasons.push(`MDP gaps must be resolved before qualification: ${input.mdp.gaps.join("; ")}.`);
  if (input.score.overall < minScore) reasons.push(`Score ${input.score.overall} is below qualification threshold ${minScore}.`);
  if (!input.evidence.length) reasons.push("At least one evidence receipt is required.");

  const blocked = input.evidence.find(isBlockedEvidenceSource);
  if (blocked) reasons.push(`Blocked evidence source is not allowed before ledger append: ${blocked.url}.`);

  return {
    ok: reasons.length === 0,
    reasons,
    personEvidenceIds: personEvidence.map((item) => item.id),
    personResolutionStatus: personEvidence.length > 0 ? "resolved" : "not_found"
  };
}

export function assertQualifiedCandidate(input: QualificationInput): QualificationValidation {
  const validation = validateQualifiedCandidate(input);
  if (!validation.ok) throw new Error(`Candidate is not qualified for ledger append: ${validation.reasons.join(" ")}`);
  return validation;
}

export function findPersonResolutionEvidence(evidence: EvidenceSource[], candidate: Candidate): EvidenceSource[] {
  return evidence.filter((item) => isPersonResolutionEvidence(item, candidate));
}

export function normalizeScoreThreshold(value: number, fallback = 65): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.max(0, Math.min(100, Math.trunc(value)));
}

function isPersonResolutionEvidence(item: EvidenceSource, candidate: Candidate): boolean {
  if (!candidate.name?.trim() || !candidate.title?.trim()) return false;
  if (!isAllowedPersonEvidenceUrl(item.url, candidate)) return false;
  if (BLOCKED_PERSON_SOURCE_WORDS.test(`${item.url}\n${item.title}\n${item.snippet}`)) return false;
  const idLooksPersonScoped = /(^|_)(person|profile|contact)(_|$)/i.test(item.id) || item.url.includes("linkedin.com/in/");
  return idLooksPersonScoped && mentionsCandidateIdentity(item, candidate);
}

function mentionsCandidateIdentity(item: EvidenceSource, candidate: Candidate): boolean {
  const text = normalize(`${item.title}\n${item.snippet}`);
  const name = normalize(candidate.name ?? "");
  const title = normalize(candidate.title ?? "");
  return Boolean(name && title && text.includes(name) && text.includes(title));
}

function isAllowedPersonEvidenceUrl(url: string, candidate: Candidate): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  const host = parsed.hostname.replace(/^www\./, "").toLowerCase();
  const hostAndPath = `${host}${parsed.pathname}`.toLowerCase();
  if (BLOCKED_EVIDENCE_HOSTS.test(host) || BLOCKED_SOCIAL_HOSTS.test(host)) return false;
  if (hostAndPath.includes("linkedin.com/company") || hostAndPath.includes("linkedin.com/jobs") || hostAndPath.includes("/jobs/")) return false;
  if (hostAndPath.includes("linkedin.com/in/")) return true;

  const accountDomain = candidate.company_domain?.replace(/^www\./, "").toLowerCase();
  if (!accountDomain) return false;
  if (host !== accountDomain && !host.endsWith(`.${accountDomain}`)) return false;
  return !/\/(careers?|jobs?|login)(\/|$)/i.test(parsed.pathname);
}

function isBlockedEvidenceSource(item: EvidenceSource): boolean {
  let host = "";
  try {
    host = new URL(item.url).hostname.replace(/^www\./, "").toLowerCase();
  } catch {
    return true;
  }
  return BLOCKED_EVIDENCE_HOSTS.test(host);
}

function normalize(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "");
}
