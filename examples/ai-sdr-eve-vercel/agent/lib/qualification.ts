import type { Candidate, EvidenceSource, MdpDecision, ScoreBreakdown } from "./schemas.ts";

export type QualificationValidation = {
  ok: boolean;
  reasons: string[];
  personEvidenceIds: string[];
  personResolutionStatus: "resolved" | "not_found";
  signalReasons: string[];
  signalEvidenceIds: string[];
};

export type QualificationSignalCoverage = {
  reasons: string[];
  evidenceIds: string[];
  hasFitSignal: boolean;
  hasNowSignal: boolean;
};

export type QualificationInput = {
  candidate: Candidate;
  evidence: EvidenceSource[];
  mdp: MdpDecision;
  score: ScoreBreakdown;
  minScore: number;
  requirePerson?: boolean;
  minSignals?: number;
};

const BLOCKED_EVIDENCE_HOSTS = /(apollo|rocketreach|signalhire|lusha|hunter\.io|zoominfo|contactout|email-format|leadiq|seamless)\./i;
const BLOCKED_PERSON_SOURCE_WORDS = /\b(email|phone|mobile|contact database|directory|salary|jobs?|hiring|careers|login|required|private)\b/i;
const BLOCKED_SOCIAL_HOSTS = /\b(instagram|facebook|threads|twitter|x|tiktok|youtube|pinterest)\.com$/i;
const FIT_SIGNAL_RE = /\b(AEO|answer engine|AI search|AI visibility|LLM visibility|zero[- ]click|prompt|citation|crawler|SEO|organic growth|content strategy|content marketing|content gap|comparison page|resource center|category discovery|PR|public relations|brand|communications|agency|client services|analytics|reporting|ChatGPT|Perplexity)\b/i;
const NOW_SIGNAL_RE = /\b(hiring|launched?|published|posted|active|current|recent|roadmap|strategy|studying|researching|building|expanding|new|webinar|event|case study|service|offering|rollout|migration|refresh)\b/i;
const GENERIC_SIGNAL_RE = /\b(exa ai sdk public account discovery|public person-role resolution signal|public person level role evidence)\b/i;

export function validateQualifiedCandidate(input: QualificationInput): QualificationValidation {
  const reasons: string[] = [];
  const minScore = normalizeScoreThreshold(input.minScore);
  const minSignals = normalizeSignalMinimum(input.minSignals ?? Number(process.env.SCOUT_MIN_SIGNALS ?? 1));
  const personEvidence = findPersonResolutionEvidence(input.evidence, input.candidate);
  const signalCoverage = findQualificationSignals(input.candidate, input.evidence);

  if (input.mdp.fit_status !== "fit") reasons.push(`MDP fit_status must be fit; received ${input.mdp.fit_status}.`);
  if (input.mdp.gaps.length > 0) reasons.push(`MDP gaps must be resolved before qualification: ${input.mdp.gaps.join("; ")}.`);
  if (input.score.overall < minScore) reasons.push(`Score ${input.score.overall} is below qualification threshold ${minScore}.`);
  if (!input.evidence.length) reasons.push("At least one evidence receipt is required.");
  if (input.requirePerson !== false && personEvidence.length === 0) {
    reasons.push("Public person-level evidence with name, title, and allowed source URL is required before qualification.");
  }
  if (signalCoverage.reasons.length < minSignals || signalCoverage.evidenceIds.length === 0) {
    reasons.push(`At least ${minSignals} source-backed fit/why-now signal is required before qualification.`);
  }
  if (!signalCoverage.hasFitSignal) {
    reasons.push("At least one source-backed signal must explain why the account/person is a good Profound fit.");
  }
  if (!signalCoverage.hasNowSignal) {
    reasons.push("At least one source-backed signal must explain why now is a reasonable time to reach out.");
  }

  const blocked = input.evidence.find(isBlockedEvidenceSource);
  if (blocked) reasons.push(`Blocked evidence source is not allowed before ledger append: ${blocked.url}.`);

  return {
    ok: reasons.length === 0,
    reasons,
    personEvidenceIds: personEvidence.map((item) => item.id),
    personResolutionStatus: personEvidence.length > 0 ? "resolved" : "not_found",
    signalReasons: signalCoverage.reasons,
    signalEvidenceIds: signalCoverage.evidenceIds
  };
}

export function assertQualifiedCandidate(input: QualificationInput): QualificationValidation {
  const validation = validateQualifiedCandidate(input);
  if (!validation.ok) throw new Error(`Candidate is not qualified for ledger append: ${validation.reasons.join(" ")}`);
  return validation;
}

export function findQualificationSignals(candidate: Candidate, evidence: EvidenceSource[], maxSignals = 3): QualificationSignalCoverage {
  const reasons: string[] = [];
  const evidenceIds: string[] = [];
  let hasFitSignal = false;
  let hasNowSignal = false;

  for (const signal of candidate.signals ?? []) {
    const cleaned = cleanSignal(signal);
    if (!cleaned || GENERIC_SIGNAL_RE.test(cleaned)) continue;
    if (FIT_SIGNAL_RE.test(cleaned) || NOW_SIGNAL_RE.test(cleaned)) pushUnique(reasons, cleaned);
  }

  const trigger = cleanSignal(candidate.trigger);
  if (trigger && FIT_SIGNAL_RE.test(trigger)) {
    hasFitSignal = true;
    pushUnique(reasons, `Fit signal from trigger: ${trigger}`);
  }
  if (trigger && NOW_SIGNAL_RE.test(trigger)) {
    hasNowSignal = true;
    pushUnique(reasons, `Why-now signal from trigger: ${trigger}`);
  }

  for (const item of evidence) {
    const text = `${item.title}
${item.snippet}`;
    const title = cleanSignal(item.title) || item.url;
    const fit = FIT_SIGNAL_RE.test(text);
    const now = NOW_SIGNAL_RE.test(text);
    if (fit) {
      hasFitSignal = true;
      pushUnique(evidenceIds, item.id);
      pushUnique(reasons, `Fit signal: ${title}`);
    }
    if (now) {
      hasNowSignal = true;
      pushUnique(evidenceIds, item.id);
      pushUnique(reasons, `Why-now signal: ${title}`);
    }
  }

  return {
    reasons: reasons.slice(0, Math.max(1, maxSignals)),
    evidenceIds,
    hasFitSignal,
    hasNowSignal
  };
}

export function findPersonResolutionEvidence(evidence: EvidenceSource[], candidate: Candidate): EvidenceSource[] {
  return evidence.filter((item) => isPersonResolutionEvidence(item, candidate));
}

export function normalizeScoreThreshold(value: number, fallback = 65): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.max(0, Math.min(100, Math.trunc(value)));
}

function normalizeSignalMinimum(value: number, fallback = 1): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.max(1, Math.min(3, Math.trunc(value)));
}

function cleanSignal(value: string | null | undefined): string | null {
  const cleaned = value?.replace(/\s+/g, " ").trim();
  return cleaned || null;
}

function pushUnique(items: string[], value: string): void {
  if (!items.includes(value)) items.push(value);
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
