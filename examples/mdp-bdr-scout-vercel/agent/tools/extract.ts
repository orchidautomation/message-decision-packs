import { extractWithFirecrawl } from "../../src/providers/firecrawl.ts";
import { extractWithApify } from "../../src/providers/apify.ts";

export async function extractEvidence(input: { url: string; mode?: "firecrawl" | "apify" }) {
  if (input.mode === "apify") return extractWithApify({ url: input.url, dryRun: !process.env.APIFY_TOKEN });
  return extractWithFirecrawl({ url: input.url, dryRun: !process.env.FIRECRAWL_API_KEY });
}
