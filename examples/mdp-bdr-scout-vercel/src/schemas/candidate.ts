export type Candidate = {
  name: string | null;
  title: string | null;
  company: string;
  company_domain: string | null;
  linkedin_url: string | null;
  source_kind: "public_web" | "news" | "github" | "community" | "dataset";
  trigger: string;
  persona?: string | null;
  segment?: string | null;
  signals?: string[];
};

export type EvidenceSource = {
  id: string;
  url: string;
  title: string;
  observed_at: string;
  snippet: string;
  content_hash: string;
  confidence: number;
  provider: "fixture" | "exa" | "firecrawl" | "apify" | "manual";
};

export type CandidateWithEvidence = {
  candidate: Candidate;
  evidence: EvidenceSource[];
};

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringOrNull(value: unknown): value is string | null {
  return typeof value === "string" || value === null;
}

export function assertCandidate(value: unknown): asserts value is Candidate {
  if (!isObject(value)) throw new Error("candidate must be an object");
  if (!isStringOrNull(value.name)) throw new Error("candidate.name must be string or null");
  if (!isStringOrNull(value.title)) throw new Error("candidate.title must be string or null");
  if (typeof value.company !== "string" || value.company.length === 0) throw new Error("candidate.company is required");
  if (!isStringOrNull(value.company_domain)) throw new Error("candidate.company_domain must be string or null");
  if (!isStringOrNull(value.linkedin_url)) throw new Error("candidate.linkedin_url must be string or null");
  if (typeof value.source_kind !== "string") throw new Error("candidate.source_kind is required");
  if (typeof value.trigger !== "string" || value.trigger.length === 0) throw new Error("candidate.trigger is required");
}

export function assertEvidenceSource(value: unknown): asserts value is EvidenceSource {
  if (!isObject(value)) throw new Error("evidence must be an object");
  for (const field of ["id", "url", "title", "observed_at", "snippet", "content_hash", "provider"]) {
    if (typeof value[field] !== "string" || String(value[field]).length === 0) throw new Error(`evidence.${field} is required`);
  }
  if (typeof value.confidence !== "number" || value.confidence < 0 || value.confidence > 1) {
    throw new Error("evidence.confidence must be between 0 and 1");
  }
}

export function assertCandidateWithEvidence(value: unknown): asserts value is CandidateWithEvidence {
  if (!isObject(value)) throw new Error("candidate fixture must be an object");
  assertCandidate(value.candidate);
  if (!Array.isArray(value.evidence) || value.evidence.length === 0) throw new Error("evidence must be a non-empty array");
  value.evidence.forEach(assertEvidenceSource);
}
