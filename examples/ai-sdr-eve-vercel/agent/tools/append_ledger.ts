import { defineTool } from "eve/tools";
import { z } from "zod";
import { appendLedgerRows, createRunId } from "../lib/ledger.ts";
import { assertQualifiedCandidate, normalizeScoreThreshold } from "../lib/qualification.ts";
import { scoreCandidate } from "../lib/scoring.ts";
import { loadSourceStrategy, selectScoutQuery } from "../lib/source-strategy.ts";
import { candidateSchema, evidenceSchema, mdpDecisionSchema } from "../lib/schemas.ts";

export default defineTool({
  description: "Append a qualified, normalized MDP scout row to the JSONL ledger. Outreach remains false and CRM sync is disabled by default.",
  inputSchema: z.object({
    candidate: candidateSchema,
    evidence: z.array(evidenceSchema).min(1),
    mdp: mdpDecisionSchema,
    runId: z.string().optional(),
    packId: z.string().optional()
  }),
  async execute(input) {
    const strategy = await loadSourceStrategy();
    const selected = selectScoutQuery(strategy);
    const score = scoreCandidate({ mdp: input.mdp, evidence: input.evidence });
    const minScore = normalizeScoreThreshold(Number(process.env.SCOUT_MIN_SCORE ?? 65));
    const qualification = assertQualifiedCandidate({ candidate: input.candidate, evidence: input.evidence, mdp: input.mdp, score, minScore });
    const provider = input.evidence.some((item) => item.provider === "exa") ? "live" : input.evidence.some((item) => item.provider === "fixture") ? "fixture" : "optional";
    const sourceStrategy = {
      ...selected.trace,
      provider_mode: provider,
      provider_available: provider === "live",
      provider_fallback: provider === "fixture" ? "Ledger append received fixture evidence rather than live provider output." : null,
      person_resolution_status: qualification.personResolutionStatus,
      person_resolution_evidence_ids: qualification.personEvidenceIds
    };
    const row = {
      contract_version: "mdp_scout_candidate/v0" as const,
      run_id: input.runId ?? createRunId(),
      pack_id: input.packId ?? process.env.MDP_PACK_ID ?? "synthetic-vendor-gtm-vetting-example",
      source_strategy: sourceStrategy,
      candidate: input.candidate,
      evidence: input.evidence,
      mdp: input.mdp,
      score,
      actions: { outreach_sent: false as const, crm_sync_status: process.env.CRM_SYNC_ENABLED === "true" ? "pending" as const : "not_enabled" as const }
    };
    const written = await appendLedgerRows([row]);
    return { ...written, row };
  }
});
