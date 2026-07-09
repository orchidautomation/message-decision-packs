import type { EvidenceSource, MdpDecision, ScoreBreakdown } from "./schemas.ts";

export function scoreCandidate(input: { mdp: MdpDecision; evidence: EvidenceSource[] }): ScoreBreakdown {
  const mdpFit = input.mdp.fit_status === "fit" ? 30 : 0;
  const evidenceQuality = Math.min(22, Math.round(input.evidence.reduce((sum, item) => sum + item.confidence, 0) * 12));
  const triggerRelevance = input.evidence.some((item) => /AI|answer|search|visibility|prompt|content|growth|workflow|agent/i.test(item.snippet)) ? 18 : 8;
  const personaMatch = input.mdp.persona ? 10 : 4;
  const recency = input.evidence.some((item) => Date.parse(item.observed_at) > Date.now() - 1000 * 60 * 60 * 24 * 120) ? 10 : 5;
  const operationalConfidence = input.mdp.gaps.length === 0 ? 10 : Math.max(2, 10 - input.mdp.gaps.length * 3);
  const overall = Math.min(100, mdpFit + evidenceQuality + triggerRelevance + personaMatch + recency + operationalConfidence);
  return {
    overall,
    components: { mdp_fit: mdpFit, evidence_quality: evidenceQuality, trigger_relevance: triggerRelevance, persona_match: personaMatch, recency, operational_confidence: operationalConfidence },
    rationale: [
      `MDP fit status: ${input.mdp.fit_status}.`,
      `Evidence count: ${input.evidence.length}.`,
      input.mdp.gaps.length ? `Open gaps: ${input.mdp.gaps.join("; ")}.` : "No blocking MDP gaps in this decision."
    ],
    evidence_ids: input.evidence.map((item) => item.id)
  };
}
