import { discoverCandidates } from "./tools/search.ts";
import { extractEvidence } from "./tools/extract.ts";
import { runMdp } from "./tools/mdp.ts";
import { writeLedgerRows } from "./tools/ledger.ts";

export const bdrScoutAgent = {
  name: "mdp-bdr-scout",
  description: "Scheduled Vercel-first BDR Scout powered by Message Decision Packs.",
  tools: {
    discoverCandidates,
    extractEvidence,
    runMdp,
    writeLedgerRows
  }
};
