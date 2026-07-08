import type { Candidate, EvidenceSource } from "./candidate.ts";

export type MdpDecision = {
  fit_status: "fit" | "no_fit" | "insufficient_context";
  persona: string | null;
  route: string | null;
  brief_json_url: string | null;
  brief_md_url: string | null;
  gaps: string[];
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
  candidate: Candidate;
  evidence: EvidenceSource[];
  mdp: MdpDecision;
  score: ScoreBreakdown;
  actions: {
    outreach_sent: false;
    crm_sync_status: "not_enabled" | "pending" | "synced" | "failed";
  };
};

export function assertLedgerRow(row: LedgerRow): void {
  if (row.contract_version !== "mdp_scout_candidate/v0") throw new Error("unexpected ledger contract version");
  if (!row.run_id) throw new Error("run_id is required");
  if (!row.pack_id) throw new Error("pack_id is required");
  if (row.score.overall < 0 || row.score.overall > 100) throw new Error("score.overall must be 0-100");
  if (row.actions.outreach_sent !== false) throw new Error("outreach must stay disabled in the scout ledger");
}
