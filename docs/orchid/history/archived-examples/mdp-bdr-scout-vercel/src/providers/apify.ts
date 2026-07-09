import { createHash } from "node:crypto";
import type { EvidenceSource } from "../schemas/candidate.ts";

export async function extractWithApify(input: { url: string; actorId?: string; dryRun?: boolean }): Promise<EvidenceSource> {
  if (input.dryRun || !process.env.APIFY_TOKEN || !input.actorId) {
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

  const actor = encodeURIComponent(input.actorId);
  const response = await fetch(`https://api.apify.com/v2/acts/${actor}/run-sync-get-dataset-items`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${process.env.APIFY_TOKEN}`,
      "Content-Type": "application/json"
    },
    body: JSON.stringify({ startUrls: [{ url: input.url }], url: input.url })
  });

  if (!response.ok) {
    throw new Error(`Apify actor failed: ${response.status} ${await response.text()}`);
  }

  const items = await response.json() as unknown[];
  const first = items[0] as Record<string, unknown> | undefined;
  const text = JSON.stringify(first ?? {}).slice(0, 1000);
  const title = typeof first?.title === "string" ? first.title : "Apify actor dataset item";

  return {
    id: `apify_${createHash("sha256").update(input.url + text).digest("hex").slice(0, 10)}`,
    url: input.url,
    title,
    observed_at: new Date().toISOString(),
    snippet: text || "Apify actor returned an empty dataset item.",
    content_hash: `sha256:${createHash("sha256").update(text).digest("hex")}`,
    confidence: first ? 0.72 : 0.4,
    provider: "apify"
  };
}
