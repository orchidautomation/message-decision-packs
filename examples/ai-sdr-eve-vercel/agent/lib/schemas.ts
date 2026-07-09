import { z } from "zod";

export const candidateSchema = z.object({
  name: z.string().nullable(),
  title: z.string().nullable(),
  company: z.string().min(1),
  company_domain: z.string().nullable(),
  linkedin_url: z.string().nullable(),
  source_kind: z.enum(["public_web", "news", "github", "community", "dataset"]),
  trigger: z.string().min(1),
  persona: z.string().nullable().optional(),
  segment: z.string().nullable().optional(),
  signals: z.array(z.string()).optional()
});

export const evidenceSchema = z.object({
  id: z.string().min(1),
  url: z.string().url(),
  title: z.string().min(1),
  observed_at: z.string().min(1),
  snippet: z.string().min(1),
  content_hash: z.string().min(1),
  confidence: z.number().min(0).max(1),
  provider: z.enum(["fixture", "exa", "firecrawl", "apify", "manual"])
});

export const candidateWithEvidenceSchema = z.object({
  candidate: candidateSchema,
  evidence: z.array(evidenceSchema).min(1)
});

export const mdpDecisionSchema = z.object({
  fit_status: z.enum(["fit", "no_fit", "insufficient_context"]),
  persona: z.string().nullable(),
  route: z.string().nullable(),
  brief_json_url: z.string().nullable(),
  brief_md_url: z.string().nullable(),
  gaps: z.array(z.string())
});

export type Candidate = z.infer<typeof candidateSchema>;
export type EvidenceSource = z.infer<typeof evidenceSchema>;
export type CandidateWithEvidence = z.infer<typeof candidateWithEvidenceSchema>;
export type MdpDecision = z.infer<typeof mdpDecisionSchema>;

export type SourceStrategyTrace = {
  strategy_id: string;
  profile_id: string;
  review_status: "draft" | "needs-human-review" | "accepted" | "blocked" | string;
  query_id: string;
  scout_family: string;
  source_target_ids: string[];
};

export type ScoreBreakdown = {
  overall: number;
  components: {
    mdp_fit: number;
    evidence_quality: number;
    trigger_relevance: number;
    persona_match: number;
    recency: number;
    operational_confidence: number;
  };
  rationale: string[];
  evidence_ids: string[];
};

export type LedgerRow = {
  contract_version: "mdp_scout_candidate/v0";
  run_id: string;
  pack_id: string;
  source_strategy: SourceStrategyTrace;
  candidate: Candidate;
  evidence: EvidenceSource[];
  mdp: MdpDecision;
  score: ScoreBreakdown;
  actions: {
    outreach_sent: false;
    crm_sync_status: "not_enabled" | "pending" | "synced" | "failed";
  };
};
