import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import bundledFixture from "../../samples/profound-public-source-fixture.json";
import { fixturePath } from "./paths.ts";
import { capabilityFor, providerCapabilities, runExaSearch, type ExaSearchResult, type ProviderCapability } from "./provider-tools.ts";
import { candidateWithEvidenceSchema, type Candidate, type CandidateWithEvidence, type EvidenceSource } from "./schemas.ts";

export type DiscoveryResult = {
  provider: "exa" | "fixture";
  mode: "live" | "fixture";
  providerCapabilities: ProviderCapability[];
  fallbackReason: string | null;
  candidates: CandidateWithEvidence[];
};

type PersonResolution = {
  name: string;
  title: string;
  url: string;
  evidence: EvidenceSource;
  persona: string;
  segment: string;
};

const PERSON_ROLE_TERMS = [
  "AEO",
  "answer engine optimization",
  "AI search",
  "AI visibility",
  "SEO",
  "organic growth",
  "content strategy",
  "content marketing",
  "brand",
  "communications",
  "PR",
  "public relations",
  "growth marketing",
  "demand generation",
  "marketing"
];

const TITLE_RE = /\b(?:Chief|CMO|CRO|VP|V\.P\.|Vice President|SVP|EVP|Head|Director|Senior Director|Manager|Lead|Principal|Strategist|Specialist|Owner|Founder|Partner|Consultant)\b[^\n|•]{0,120}/i;
const NAME_RE = /\b([A-Z][A-Za-z'.-]+(?:\s+[A-Z][A-Za-z'.-]+){1,3})\b/;
const BLOCKED_PERSON_HOSTS = /(apollo|rocketreach|signalhire|lusha|hunter\.io|zoominfo|contactout|email-format|leadIQ|seamless)\./i;
const BLOCKED_PERSON_WORDS = /\b(email|phone|mobile|contact database|directory|salary|jobs?|hiring|careers|login|required|private)\b/i;

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
  const accountCandidates = (payload.results ?? []).slice(0, input.limit).map(resultToCandidateWithEvidence);
  const requirePeople = process.env.SCOUT_REQUIRE_PERSON !== "false";
  const personSearchLimit = clamp(Number(process.env.SCOUT_PERSON_SEARCH_LIMIT ?? 6), 1, 10);
  const resolved: CandidateWithEvidence[] = [];

  for (const accountCandidate of accountCandidates) {
    const person = await resolvePersonForAccount(accountCandidate, personSearchLimit);
    if (person) {
      resolved.push(applyPersonResolution(accountCandidate, person));
    } else if (!requirePeople) {
      resolved.push(accountCandidate);
    }
  }

  return {
    provider: "exa",
    mode: "live",
    providerCapabilities: capabilities,
    fallbackReason: resolved.length ? null : "Exa found account trigger evidence, but no public person-level owner matched the MDP source policy.",
    candidates: resolved
  };
}

export async function readFixture(): Promise<CandidateWithEvidence> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(await readFile(fixturePath(), "utf8"));
  } catch (error) {
    if (!isMissingFile(error)) throw error;
    parsed = bundledFixture as unknown;
  }
  return candidateWithEvidenceSchema.parse(parsed);
}

async function resolvePersonForAccount(account: CandidateWithEvidence, limit: number): Promise<PersonResolution | null> {
  if (account.candidate.name && account.candidate.title) {
    const evidence = account.evidence[0];
    return {
      name: account.candidate.name,
      title: account.candidate.title,
      url: account.candidate.linkedin_url ?? evidence.url,
      evidence,
      persona: account.candidate.persona ?? inferPersona(account.candidate.title),
      segment: account.candidate.segment ?? inferSegment(account.candidate.title)
    };
  }

  const query = buildPersonResolutionQuery(account.candidate);
  const payload = await runExaSearch({ query, limit });
  for (const result of payload.results ?? []) {
    const parsed = parsePersonResult(result, account.candidate);
    if (parsed) return parsed;
  }
  return null;
}

function applyPersonResolution(account: CandidateWithEvidence, person: PersonResolution): CandidateWithEvidence {
  const candidate: Candidate = {
    ...account.candidate,
    name: person.name,
    title: person.title,
    linkedin_url: person.url.includes("linkedin.com/in/") ? person.url : account.candidate.linkedin_url,
    persona: person.persona,
    segment: person.segment,
    signals: [
      ...(account.candidate.signals ?? []),
      "Public person-level role evidence",
      person.title
    ]
  };
  return candidateWithEvidenceSchema.parse({
    candidate,
    evidence: [...account.evidence, person.evidence]
  });
}

function resultToCandidateWithEvidence(result: ExaSearchResult): CandidateWithEvidence {
  const url = result.url ?? "https://unknown.example";
  const host = safeHost(url);
  const title = result.title ?? host;
  const snippet = firstText(result.highlights?.[0], result.summary, result.text, title);
  const company = inferCompanyName(result, host);
  const id = `exa_${createHash("sha256").update(url + title).digest("hex").slice(0, 10)}`;

  return {
    candidate: {
      name: null,
      title: null,
      company,
      company_domain: host === "unknown.example" ? null : host,
      linkedin_url: null,
      source_kind: "public_web",
      trigger: snippet,
      persona: null,
      segment: null,
      signals: ["Exa AI SDK public account discovery", title]
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

function buildPersonResolutionQuery(candidate: Candidate): string {
  const company = quote(candidate.company);
  const domain = candidate.company_domain ? quote(candidate.company_domain) : "";
  const site = candidate.company_domain ? `OR site:${candidate.company_domain}` : "";
  return `(${company}${domain ? ` OR ${domain}` : ""}) (${PERSON_ROLE_TERMS.map(quote).join(" OR ")}) ("Director" OR "Head" OR "VP" OR "Vice President" OR "Manager" OR "Lead" OR "Principal" OR "Strategist" OR "Founder" OR "Partner") (site:linkedin.com/in ${site}) -jobs -hiring -careers -email -phone -"contact database" -apollo -rocketreach -zoominfo -lusha`;
}

function parsePersonResult(result: ExaSearchResult, account: Candidate): PersonResolution | null {
  const url = result.url ?? "";
  if (!url || !isAllowedPersonUrl(url, account)) return null;
  const haystack = compact([result.title, result.author, result.summary, ...(result.highlights ?? []), result.text]).join("\n");
  if (!mentionsCompany(haystack, account) || !mentionsRelevantRole(haystack) || BLOCKED_PERSON_WORDS.test(haystack)) return null;

  const name = extractPersonName(result, account);
  if (!name) return null;
  const title = extractPersonTitle(result, name);
  if (!title || !mentionsRelevantRole(title)) return null;
  const snippet = firstText(result.highlights?.[0], result.summary, result.text, result.title, `${name} — ${title}`);
  const evidence: EvidenceSource = {
    id: `exa_person_${createHash("sha256").update(url + name + title).digest("hex").slice(0, 10)}`,
    url,
    title: result.title ?? `${name} — ${title}`,
    observed_at: result.publishedDate ?? new Date().toISOString(),
    snippet,
    content_hash: `sha256:${createHash("sha256").update(snippet).digest("hex")}`,
    confidence: url.includes("linkedin.com/in/") ? 0.78 : 0.7,
    provider: "exa"
  };

  return {
    name,
    title: cleanTitle(title),
    url,
    evidence,
    persona: inferPersona(title),
    segment: inferSegment(title)
  };
}

function extractPersonName(result: ExaSearchResult, account: Candidate): string | null {
  const candidates: string[] = [];
  if (result.url?.includes("linkedin.com/in/") && result.title) {
    candidates.push(result.title.split(/\s[-|–—]\s/)[0] ?? "");
  }
  if (result.author) candidates.push(result.author);
  const titleMatch = result.title?.match(NAME_RE)?.[1];
  if (titleMatch) candidates.push(titleMatch);
  const text = compact([result.summary, ...(result.highlights ?? []), result.text]).join("\n");
  const textMatch = text.match(NAME_RE)?.[1];
  if (textMatch) candidates.push(textMatch);

  for (const raw of candidates) {
    const name = cleanName(raw);
    if (isLikelyPersonName(name, account)) return name;
  }
  return null;
}

function extractPersonTitle(result: ExaSearchResult, name: string): string | null {
  const title = result.title ?? "";
  const parts = title.split(/\s[-–—|]\s/).map((part) => part.trim()).filter(Boolean);
  const titlePart = parts.find((part) => part !== name && mentionsRelevantRole(part));
  if (titlePart) return titlePart;

  const text = compact([result.summary, ...(result.highlights ?? []), result.text, title]).join("\n");
  const afterName = text.slice(Math.max(0, text.indexOf(name)));
  const matched = (afterName.match(TITLE_RE) ?? text.match(TITLE_RE))?.[0];
  return matched ? cleanTitle(matched) : null;
}

function isAllowedPersonUrl(url: string, account: Candidate): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  const host = parsed.hostname.replace(/^www\./, "").toLowerCase();
  const hostAndPath = `${host}${parsed.pathname}`.toLowerCase();
  if (BLOCKED_PERSON_HOSTS.test(host)) return false;
  if (/\b(instagram|facebook|threads|twitter|x|tiktok|youtube|pinterest)\.com$/i.test(host)) return false;
  if (hostAndPath.includes("linkedin.com/company") || hostAndPath.includes("linkedin.com/jobs") || hostAndPath.includes("/jobs/")) return false;
  if (hostAndPath.includes("linkedin.com/in/")) return true;
  const accountDomain = account.company_domain?.replace(/^www\./, "").toLowerCase();
  return Boolean(accountDomain && (host === accountDomain || host.endsWith(`.${accountDomain}`)));
}

function mentionsCompany(text: string, account: Candidate): boolean {
  const normalized = normalize(text);
  const company = normalize(account.company);
  const domain = normalize(account.company_domain ?? "");
  return Boolean(company && normalized.includes(company)) || Boolean(domain && normalized.includes(domain.replace(/\./g, "")));
}

function mentionsRelevantRole(text: string): boolean {
  return /\b(AEO|answer engine|AI search|AI visibility|LLM visibility|SEO|organic growth|content strategy|content marketing|brand|communications|public relations|\bPR\b|growth marketing|demand generation|marketing|search)\b/i.test(text);
}

function isLikelyPersonName(name: string | null, account: Candidate): name is string {
  if (!name) return false;
  const words = name.split(/\s+/).filter(Boolean);
  if (words.length < 2 || words.length > 4) return false;
  if (normalize(name).includes(normalize(account.company))) return false;
  if (/\b(Answer|Engine|Optimization|Marketing|Search|Guide|Playbook|Agency|Company|Solutions|Digital|Media|Studio|Group|Team|LinkedIn|Profile|Director|Manager|Lead|Founder|Partner|SEO|AEO|AI)\b/i.test(name)) return false;
  return words.every((word) => /^[A-Z][A-Za-z'.-]+$/.test(word));
}

function inferCompanyName(result: ExaSearchResult, host: string): string {
  const title = result.title ?? "";
  const separators = title.split(/\s[|–—]\s/).map((part) => part.trim()).filter(Boolean);
  const tail = separators.at(-1);
  if (tail && isLikelyCompanyLabel(tail)) return tail;
  return prettifyHost(host);
}

function inferPersona(title: string): string {
  if (/agency|partner|consultant|client service/i.test(title)) return "Agency Partner";
  if (/\bPR\b|public relations|communications|brand|product marketing|analyst relations/i.test(title)) return "PR & Brand Lead";
  if (/content/i.test(title)) return "Content Lead";
  return "AEO Lead";
}

function inferSegment(title: string): string {
  if (/agency|partner|consultant|client service/i.test(title)) return "Agency partner or services team";
  if (/\bPR\b|public relations|communications|brand|product marketing|analyst relations/i.test(title)) return "Brand, PR, or communications team";
  if (/content/i.test(title)) return "Content or demand team";
  return "AEO, SEO, or organic growth team";
}

function isLikelyCompanyLabel(value: string): boolean {
  return value.length >= 2 && value.length <= 60 && !/answer engine|SEO|AEO|guide|playbook|blog|article|linkedin/i.test(value);
}

function prettifyHost(host: string): string {
  const base = host.replace(/^www\./, "").split(".")[0] ?? host;
  return base.split(/[-_]/).filter(Boolean).map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" ") || host;
}

function cleanName(value: string | null | undefined): string | null {
  const cleaned = value?.replace(/\|\s*LinkedIn.*$/i, "").replace(/\s+-\s+LinkedIn.*$/i, "").replace(/[^A-Za-z'.\-\s]/g, " ").replace(/\s+/g, " ").trim();
  return cleaned || null;
}

function cleanTitle(value: string): string {
  return value
    .replace(/\|\s*LinkedIn.*$/i, "")
    .replace(/\s+at\s+[A-Z].*$/i, "")
    .replace(/\s+based\s+in\s+.*$/i, "")
    .replace(/\s+since\s+.*$/i, "")
    .replace(/\.\s+.*$/i, "")
    .replace(/\s+/g, " ")
    .replace(/^(at|is|as)\s+/i, "")
    .trim()
    .slice(0, 120);
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

function compact(values: Array<string | undefined>): string[] {
  return values.filter((value): value is string => Boolean(value && value.trim()));
}

function quote(value: string): string {
  return `"${value.replace(/"/g, "")}"`;
}

function normalize(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

function clamp(value: number, min: number, max: number): number {
  return Number.isFinite(value) ? Math.max(min, Math.min(max, Math.trunc(value))) : min;
}

function isMissingFile(error: unknown): boolean {
  return typeof error === "object" && error !== null && "code" in error && (error as { code?: unknown }).code === "ENOENT";
}
