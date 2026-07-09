import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fixturePath } from "./paths.ts";
import { candidateWithEvidenceSchema, type CandidateWithEvidence } from "./schemas.ts";

export async function discoverCandidates(input: { query: string; limit: number; dryRun?: boolean }): Promise<CandidateWithEvidence[]> {
  const dryRun = input.dryRun ?? !process.env.EXA_API_KEY;
  if (dryRun) return [await readFixture()];

  const response = await fetch("https://api.exa.ai/search", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": process.env.EXA_API_KEY ?? ""
    },
    body: JSON.stringify({
      query: input.query,
      type: "auto",
      numResults: input.limit,
      contents: { highlights: true, text: { maxCharacters: 1200 }, summary: true }
    })
  });

  if (!response.ok) throw new Error(`Exa search failed: ${response.status} ${await response.text()}`);
  const payload = await response.json() as { results?: ExaSearchResult[] };
  return (payload.results ?? []).slice(0, input.limit).map(resultToCandidateWithEvidence);
}

export async function readFixture(): Promise<CandidateWithEvidence> {
  const parsed = JSON.parse(await readFile(fixturePath(), "utf8"));
  return candidateWithEvidenceSchema.parse(parsed);
}

type ExaSearchResult = {
  title?: string;
  url?: string;
  publishedDate?: string;
  highlights?: string[];
  text?: string;
  summary?: string;
};

function resultToCandidateWithEvidence(result: ExaSearchResult): CandidateWithEvidence {
  const url = result.url ?? "https://unknown.example";
  const host = safeHost(url);
  const title = result.title ?? host;
  const snippet = firstText(result.highlights?.[0], result.summary, result.text, title);
  const company = host.split(".").slice(0, -1).join(".") || host;
  const id = `exa_${createHash("sha256").update(url + title).digest("hex").slice(0, 10)}`;

  return {
    candidate: {
      name: null,
      title: "GTM or Growth Leader",
      company,
      company_domain: host,
      linkedin_url: null,
      source_kind: "public_web",
      trigger: snippet,
      persona: "AEO Lead",
      segment: null,
      signals: ["Exa public discovery", title]
    },
    evidence: [{
      id,
      url,
      title,
      observed_at: result.publishedDate ?? new Date().toISOString(),
      snippet,
      content_hash: `sha256:${createHash("sha256").update(snippet).digest("hex")}`,
      confidence: 0.68,
      provider: "exa"
    }]
  };
}

function safeHost(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "unknown.example";
  }
}

function firstText(...values: Array<string | undefined>): string {
  return values.find((value) => value && value.trim().length > 0)?.trim().slice(0, 1000) ?? "Public source matched the scout query.";
}
