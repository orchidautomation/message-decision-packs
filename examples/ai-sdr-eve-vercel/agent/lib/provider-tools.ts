import { createHash } from "node:crypto";
import { tool } from "ai";
import { z } from "zod";
import type { EvidenceSource } from "./schemas.ts";

export type ProviderName = "exa" | "firecrawl" | "apify" | "fixture";
export type ProviderMode = "live" | "fixture" | "optional" | "unavailable";

export type ProviderCapability = {
  provider: ProviderName;
  configured: boolean;
  enabled: boolean;
  requiredEnv: string | null;
  mode: ProviderMode;
  reason: string;
};

export type ExaSearchInput = {
  query: string;
  limit: number;
};

export type ExaSearchResult = {
  title?: string;
  url?: string;
  id?: string;
  publishedDate?: string;
  author?: string;
  image?: string;
  favicon?: string;
  highlights?: string[];
  text?: string;
  summary?: string;
};

export type ExaSearchPayload = {
  results?: ExaSearchResult[];
};

export function providerCapabilities(): ProviderCapability[] {
  const exaConfigured = Boolean(process.env.EXA_API_KEY);
  const firecrawlConfigured = Boolean(process.env.FIRECRAWL_API_KEY);
  const apifyConfigured = Boolean(process.env.APIFY_TOKEN);

  return [
    {
      provider: "exa",
      configured: exaConfigured,
      enabled: exaConfigured,
      requiredEnv: "EXA_API_KEY",
      mode: exaConfigured ? "live" : "fixture",
      reason: exaConfigured
        ? "Exa live discovery is enabled through a local AI SDK tool wrapper."
        : "EXA_API_KEY is absent; live discovery must use the public-safe fixture fallback."
    },
    {
      provider: "firecrawl",
      configured: firecrawlConfigured,
      enabled: firecrawlConfigured,
      requiredEnv: "FIRECRAWL_API_KEY",
      mode: firecrawlConfigured ? "live" : "optional",
      reason: firecrawlConfigured
        ? "Firecrawl accepted-URL extraction is available through a local AI SDK tool wrapper."
        : "FIRECRAWL_API_KEY is absent; accepted URL cleanup is skipped unless a key is added."
    },
    {
      provider: "apify",
      configured: apifyConfigured,
      enabled: false,
      requiredEnv: "APIFY_TOKEN",
      mode: "optional",
      reason: apifyConfigured
        ? "APIFY_TOKEN is present, but Apify MCP/Actor execution is intentionally deferred from this first Eve slice."
        : "APIFY_TOKEN is absent; Apify MCP/Actor execution remains an optional follow-up."
    },
    {
      provider: "fixture",
      configured: true,
      enabled: true,
      requiredEnv: null,
      mode: "fixture",
      reason: "Public-safe fixture fallback is always available for local checks and template demos."
    }
  ];
}

export function capabilityFor(provider: ProviderName): ProviderCapability {
  return providerCapabilities().find((item) => item.provider === provider) ?? {
    provider,
    configured: false,
    enabled: false,
    requiredEnv: null,
    mode: "unavailable",
    reason: `${provider} is not available in this workspace.`
  };
}

async function executeExaSearch(input: ExaSearchInput): Promise<ExaSearchPayload> {
  if (!process.env.EXA_API_KEY) {
    throw new Error("EXA_API_KEY is required for live Exa discovery; use fixture fallback when it is absent.");
  }

  const response = await fetch("https://api.exa.ai/search", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": process.env.EXA_API_KEY,
      "x-exa-integration": "vercel-ai-sdk",
      "User-Agent": "mdp-eve-ai-sdr vercel-ai-sdk-tool"
    },
    body: JSON.stringify({
      query: input.query,
      type: "auto",
      numResults: input.limit,
      contents: {
        highlights: true,
        text: { maxCharacters: 1200 },
        summary: true,
        livecrawl: "fallback"
      }
    })
  });

  if (!response.ok) {
    throw new Error(`Exa search failed: ${response.status} ${await response.text()}`);
  }

  return await response.json() as ExaSearchPayload;
}

export const exaSearchTool = tool({
  description: "Search public web sources with Exa for source-backed MDP scout candidates. Requires EXA_API_KEY; fixture fallback is handled by the caller.",
  inputSchema: z.object({
    query: z.string().min(3).max(500),
    limit: z.number().int().min(1).max(25).default(5)
  }),
  execute: executeExaSearch
});

export async function runExaSearch(input: ExaSearchInput): Promise<ExaSearchPayload> {
  return executeExaSearch(input);
}

async function executeFirecrawlScrape(input: { url: string }): Promise<EvidenceSource> {
  if (!process.env.FIRECRAWL_API_KEY) {
    throw new Error("FIRECRAWL_API_KEY is required for live Firecrawl extraction; skip cleanup when it is absent.");
  }

  const response = await fetch("https://api.firecrawl.dev/v2/scrape", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${process.env.FIRECRAWL_API_KEY}`,
      "Content-Type": "application/json",
      "User-Agent": "mdp-eve-ai-sdr vercel-ai-sdk-tool"
    },
    body: JSON.stringify({
      url: input.url,
      formats: ["markdown"],
      onlyMainContent: true,
      removeBase64Images: true,
      blockAds: true,
      timeout: 60_000
    })
  });

  if (!response.ok) {
    throw new Error(`Firecrawl scrape failed: ${response.status} ${await response.text()}`);
  }

  const payload = await response.json() as {
    success?: boolean;
    data?: { markdown?: string; html?: string; metadata?: { title?: string; sourceURL?: string; url?: string } };
    error?: string;
  };
  if (payload.success === false) throw new Error(payload.error ?? "Firecrawl scrape returned success=false");

  const data = payload.data ?? {};
  const snippet = (data.markdown ?? data.html ?? "").replace(/\s+/g, " ").trim().slice(0, 1000) || "Firecrawl returned no textual content.";
  const sourceUrl = data.metadata?.sourceURL ?? data.metadata?.url ?? input.url;
  const title = data.metadata?.title ?? sourceUrl;
  const id = `firecrawl_${createHash("sha256").update(sourceUrl + snippet).digest("hex").slice(0, 10)}`;

  return {
    id,
    url: sourceUrl,
    title,
    observed_at: new Date().toISOString(),
    snippet,
    content_hash: `sha256:${createHash("sha256").update(snippet).digest("hex")}`,
    confidence: snippet.length > 120 ? 0.78 : 0.55,
    provider: "firecrawl"
  };
}

export const firecrawlScrapeTool = tool({
  description: "Extract clean evidence from an already accepted public URL with Firecrawl. Requires FIRECRAWL_API_KEY and must not be used for broad discovery.",
  inputSchema: z.object({ url: z.string().url() }),
  execute: executeFirecrawlScrape
});

export async function runFirecrawlScrape(input: { url: string }): Promise<EvidenceSource> {
  return executeFirecrawlScrape(input);
}
