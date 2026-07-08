import { defineTool } from "eve/tools";
import { z } from "zod";
import { extractWithFirecrawl } from "../../src/providers/firecrawl.ts";
import { extractWithApify } from "../../src/providers/apify.ts";

export async function extractEvidence(input: { url: string; mode?: "firecrawl" | "apify"; actorId?: string }) {
  if (input.mode === "apify") return extractWithApify({ url: input.url, actorId: input.actorId, dryRun: !process.env.APIFY_TOKEN });
  return extractWithFirecrawl({ url: input.url, dryRun: !process.env.FIRECRAWL_API_KEY });
}

export default defineTool({
  description: "Extract source-backed evidence from a URL using Firecrawl by default or Apify for hard-site actor workflows.",
  inputSchema: z.object({
    url: z.string().url(),
    mode: z.enum(["firecrawl", "apify"]).optional(),
    actorId: z.string().optional()
  }),
  async execute(input) {
    return extractEvidence(input);
  }
});
