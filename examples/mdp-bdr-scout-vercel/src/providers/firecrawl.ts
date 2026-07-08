import type { EvidenceSource } from "../schemas/candidate.ts";

export async function extractWithFirecrawl(input: { url: string; dryRun?: boolean }): Promise<EvidenceSource> {
  if (input.dryRun || !process.env.FIRECRAWL_API_KEY) {
    return {
      id: "firecrawl-dry-run",
      url: input.url,
      title: "Dry-run Firecrawl extraction",
      observed_at: new Date().toISOString(),
      snippet: "Firecrawl would extract markdown, structured JSON, screenshots, or JS-rendered page content here.",
      content_hash: "sha256:dry-run-firecrawl",
      confidence: 0.5,
      provider: "firecrawl"
    };
  }

  throw new Error("Live Firecrawl extraction is intentionally not enabled in this scaffold. Add the Firecrawl SDK/API call after credential policy is set.");
}
