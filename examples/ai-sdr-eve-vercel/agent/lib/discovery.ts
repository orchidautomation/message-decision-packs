import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { fixturePath } from "./paths.ts";
import { capabilityFor, providerCapabilities, runExaSearch, type ExaSearchResult, type ProviderCapability } from "./provider-tools.ts";
import { candidateWithEvidenceSchema, type CandidateWithEvidence } from "./schemas.ts";

export type DiscoveryResult = {
  provider: "exa" | "fixture";
  mode: "live" | "fixture";
  providerCapabilities: ProviderCapability[];
  fallbackReason: string | null;
  candidates: CandidateWithEvidence[];
};

export async function discoverCandidates(input: { query: string; limit: number; dryRun?: boolean }): Promise<DiscoveryResult> {
  const capabilities = providerCapabilities();
  const exa = capabilityFor("exa");
  const dryRun = input.dryRun ?? !exa.enabled;

  if (dryRun) {
    const reason = input.dryRun
      ? "Dry-run requested; using public-safe fixture candidate."
      : exa.reason;
    return {
      provider: "fixture",
      mode: "fixture",
      providerCapabilities: capabilities,
      fallbackReason: reason,
      candidates: [await readFixture()]
    };
  }

  const payload = await runExaSearch({ query: input.query, limit: input.limit });
  return {
    provider: "exa",
    mode: "live",
    providerCapabilities: capabilities,
    fallbackReason: null,
    candidates: (payload.results ?? []).slice(0, input.limit).map(resultToCandidateWithEvidence)
  };
}

export async function readFixture(): Promise<CandidateWithEvidence> {
  const parsed = JSON.parse(await readFile(fixturePath(), "utf8"));
  return candidateWithEvidenceSchema.parse(parsed);
}

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
      signals: ["Exa AI SDK public discovery", title]
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
