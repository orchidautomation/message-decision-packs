import type { EvidenceSource } from "../schemas/candidate.ts";

export async function extractWithApify(input: { url: string; actorId?: string; dryRun?: boolean }): Promise<EvidenceSource> {
  if (input.dryRun || !process.env.APIFY_TOKEN) {
    return {
      id: "apify-dry-run",
      url: input.url,
      title: "Dry-run Apify actor result",
      observed_at: new Date().toISOString(),
      snippet: "Apify would run an MCP tool or Actor and return dataset-backed evidence for hard-site workflows here.",
      content_hash: "sha256:dry-run-apify",
      confidence: 0.5,
      provider: "apify"
    };
  }

  throw new Error("Live Apify execution is intentionally not enabled in this scaffold. Add MCP/Actor calls after actor allowlists and cost controls are set.");
}
