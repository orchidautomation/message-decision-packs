import { createHash } from "node:crypto";
import type { EvidenceSource } from "../schemas/candidate.ts";

type FirecrawlScrapeResponse = {
  success?: boolean;
  data?: {
    markdown?: string;
    html?: string;
    metadata?: { title?: string; sourceURL?: string; url?: string };
  };
  error?: string;
};

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

  const response = await fetch("https://api.firecrawl.dev/v2/scrape", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${process.env.FIRECRAWL_API_KEY}`,
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      url: input.url,
      formats: ["markdown"],
      onlyMainContent: true,
      timeout: 60_000,
      removeBase64Images: true,
      blockAds: true
    })
  });

  if (!response.ok) {
    throw new Error(`Firecrawl scrape failed: ${response.status} ${await response.text()}`);
  }

  const payload = await response.json() as FirecrawlScrapeResponse;
  if (payload.success === false) throw new Error(payload.error ?? "Firecrawl scrape returned success=false");

  const data = payload.data ?? {};
  const title = data.metadata?.title ?? input.url;
  const snippet = (data.markdown ?? data.html ?? "").replace(/\s+/g, " ").trim().slice(0, 1000) || "Firecrawl returned no textual content.";

  return {
    id: `firecrawl_${createHash("sha256").update(input.url + snippet).digest("hex").slice(0, 10)}`,
    url: data.metadata?.sourceURL ?? data.metadata?.url ?? input.url,
    title,
    observed_at: new Date().toISOString(),
    snippet,
    content_hash: `sha256:${createHash("sha256").update(snippet).digest("hex")}`,
    confidence: snippet.length > 120 ? 0.78 : 0.55,
    provider: "firecrawl"
  };
}
