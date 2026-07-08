import { discoverWithExa } from "../../src/providers/exa.ts";

export async function discoverCandidates(input: { query: string; limit?: number }) {
  return discoverWithExa({ query: input.query, limit: input.limit ?? 10, dryRun: !process.env.EXA_API_KEY });
}
