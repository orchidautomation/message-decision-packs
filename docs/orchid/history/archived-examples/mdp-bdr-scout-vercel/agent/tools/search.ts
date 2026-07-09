import { defineTool } from "eve/tools";
import { z } from "zod";
import { discoverWithExa } from "../../src/providers/exa.ts";
import { getDefaultFixture } from "../../src/providers/fixtures.ts";

export async function discoverCandidates(input: { query: string; limit?: number }) {
  const dryRun = !process.env.EXA_API_KEY;
  const fixture = dryRun ? getDefaultFixture() : undefined;
  return discoverWithExa({ query: input.query, limit: input.limit ?? 10, dryRun, fixture });
}

export default defineTool({
  description: "Discover public candidate companies or people using Exa. Dry-runs return fixture data when EXA_API_KEY is absent.",
  inputSchema: z.object({
    query: z.string().min(3),
    limit: z.number().int().min(1).max(25).optional()
  }),
  async execute(input) {
    return discoverCandidates(input);
  }
});
