import type { EvidenceSource } from "../schemas/candidate.ts";
import type { MdpDecision, ScoreBreakdown } from "../schemas/ledger.ts";

export function scoreCandidate(input: { mdp: MdpDecision; evidence: EvidenceSource[] }): ScoreBreakdown {
  const evidenceConfidence = input.evidence.reduce((sum, item) => sum + item.confidence, 0) / Math.max(input.evidence.length, 1);
  const mdpFit = input.mdp.fit_status === "fit" ? 30 : 0;
  const evidenceQuality = Math.round(Math.min(25, input.evidence.length * 8 + evidenceConfidence * 10));
  const triggerRelevance = input.evidence.some((item) => /launch|hiring|expansion|workflow|agent|AI/i.test(item.snippet)) ? 18 : 8;
  const personaMatch = input.mdp.persona ? 10 : 4;
  const recency = input.evidence.some((item) => Date.parse(item.observed_at) > Date.now() - 1000 * 60 * 60 * 24 * 45) ? 7 : 3;
  const operationalConfidence = input.mdp.gaps.length === 0 ? 10 : Math.max(2, 10 - input.mdp.gaps.length * 3);

  const overall = Math.min(100, mdpFit + evidenceQuality + triggerRelevance + personaMatch + recency + operationalConfidence);

  return {
    overall,
    components: {
      mdp_fit: mdpFit,
      evidence_quality: evidenceQuality,
      trigger_relevance: triggerRelevance,
      persona_match: personaMatch,
      recency,
      operational_confidence: operationalConfidence
    },
    rationale: [
      `MDP fit status: ${input.mdp.fit_status}.`,
      `Evidence quality from ${input.evidence.length} source(s) at average confidence ${evidenceConfidence.toFixed(2)}.`,
      input.mdp.gaps.length ? `Open gaps: ${input.mdp.gaps.join("; ")}.` : "No blocking MDP gaps in the dry-run decision."
    ],
    evidence_ids: input.evidence.map((item) => item.id)
  };
}
