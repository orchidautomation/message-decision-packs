import { defineTool } from "eve/tools";
import { z } from "zod";
import { appendLedgerRows, createRunId } from "../lib/ledger.ts";
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
    const row = {
      contract_version: "mdp_scout_candidate/v0" as const,
      run_id: input.runId ?? createRunId(),
      pack_id: input.packId ?? process.env.MDP_PACK_ID ?? "profound-gtm-vetting-example",
      source_strategy: selected.trace,
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
