import { discoverCandidates } from "./discovery.ts";
import { appendLedgerRows, createRunId } from "./ledger.ts";
import { runMdpBrief } from "./mdp-runner.ts";
import { normalizeScoreThreshold, validateQualifiedCandidate } from "./qualification.ts";
import { scoreCandidate } from "./scoring.ts";
import { loadSourceStrategy, normalizeRunPolicy, selectPersonResolutionQuery, selectScoutQueries } from "./source-strategy.ts";
import type { CandidateWithEvidence, LedgerRow, SourceStrategyTrace } from "./schemas.ts";

export type ScoutCycleInput = {
  dryRun?: boolean;
  limit?: number;
  query?: string | null;
};

export type ScoutCycleResult = {
  runId: string;
  query: string;
  queries: string[];
  targetQualified: number;
  discoveryPasses: number;
  exhausted: boolean;
  qualified: number;
  ledgerPath: string | null;
  rows: LedgerRow[];
  provider: string;
  fallbackReason: string | null;
};

export async function runFixtureScoutCycle(): Promise<ScoutCycleResult> {
  return runScoutCycle({ dryRun: true });
}

export async function runScoutCycle(input: ScoutCycleInput = {}): Promise<ScoutCycleResult> {
  const strategy = await loadSourceStrategy();
  const selectedQueries = selectScoutQueries(strategy, input.query);
  const personResolutionQuery = selectPersonResolutionQuery(strategy);
  const policy = normalizeRunPolicy(strategy);
  const dryRun = input.dryRun === true;
  const targetQualified = dryRun ? 1 : policy.minimumQualifiedPeoplePerRun;
  const maxDiscoveryPasses = dryRun ? 1 : policy.maxDiscoveryPassesPerRun;
  const discoveryLimit = input.limit ?? parseIntegerSetting(process.env.SCOUT_MAX_CANDIDATES, policy.discoveryBatchSize, 1, 20);
  const runId = createRunId();
  const minScore = normalizeScoreThreshold(Number(process.env.SCOUT_MIN_SCORE ?? 65));
  const rows: LedgerRow[] = [];
  const queried: string[] = [];
  const fallbackReasons: string[] = [];
  const seen = new Set<string>();
  let provider = "exa";
  let discoveryPasses = 0;

  const discoveryQueue = buildDiscoveryQueue(selectedQueries, maxDiscoveryPasses, policy.stopWhenStrategyQueriesExhausted);

  for (const selected of discoveryQueue) {
    if (rows.length >= targetQualified) break;
    discoveryPasses += 1;
    queried.push(selected.query);

    const discovery = await discoverCandidates({
      query: selected.query,
      limit: discoveryLimit,
      dryRun,
      personResolutionQueryTemplate: personResolutionQuery?.query_template ?? null
    });
    provider = discovery.provider;
    if (discovery.fallbackReason) pushUnique(fallbackReasons, discovery.fallbackReason);

    const trace: SourceStrategyTrace = {
      ...selected.trace,
      provider_mode: discovery.mode,
      provider_available: discovery.mode === "live",
      provider_fallback: discovery.fallbackReason
    };

    for (const item of discovery.candidates) {
      if (rows.length >= targetQualified) break;

      const key = candidateDedupeKey(item);
      if (seen.has(key)) continue;

      const mdp = await runMdpBrief(item, "linkedin");
      const score = scoreCandidate({ mdp, evidence: item.evidence });
      const qualification = validateQualifiedCandidate({ candidate: item.candidate, evidence: item.evidence, mdp, score, minScore });
      if (!qualification.ok) continue;
      seen.add(key);
      rows.push({
        contract_version: "mdp_scout_candidate/v0",
        run_id: runId,
        pack_id: process.env.MDP_PACK_ID ?? "profound-gtm-vetting-example",
        source_strategy: {
          ...trace,
          person_resolution_status: qualification.personResolutionStatus,
          person_resolution_evidence_ids: qualification.personEvidenceIds,
          qualified_signal_reasons: qualification.signalReasons,
          qualified_signal_evidence_ids: qualification.signalEvidenceIds
        },
        candidate: item.candidate,
        evidence: item.evidence,
        mdp,
        score,
        actions: { outreach_sent: false, crm_sync_status: process.env.CRM_SYNC_ENABLED === "true" ? "pending" : "not_enabled" }
      });
    }

    if (dryRun || discovery.mode === "unavailable" || !policy.continueUntilMinimumQualified) break;
  }

  const written = rows.length ? await appendLedgerRows(rows) : null;
  const exhausted = !dryRun && policy.continueUntilMinimumQualified && rows.length < targetQualified;
  if (exhausted) {
    pushUnique(fallbackReasons, `Qualified ${rows.length} of ${targetQualified} target people before exhausting ${discoveryPasses} bounded source-strategy discovery pass(es).`);
  }
  return {
    runId,
    query: queried[0] ?? selectedQueries[0]?.query ?? "",
    queries: queried,
    targetQualified,
    discoveryPasses,
    exhausted,
    qualified: rows.length,
    ledgerPath: written?.ledgerPath ?? null,
    rows,
    provider,
    fallbackReason: fallbackReasons.length ? fallbackReasons.join(" ") : null
  };
}

function buildDiscoveryQueue<T>(queries: T[], maxPasses: number, stopWhenQueriesExhausted: boolean): T[] {
  if (!queries.length || maxPasses <= 0) return [];
  if (stopWhenQueriesExhausted) return queries.slice(0, maxPasses);
  return Array.from({ length: maxPasses }, (_, index) => queries[index % queries.length]);
}

function parseIntegerSetting(value: string | undefined, fallback: number, min: number, max: number): number {
  const parsed = Number(value ?? fallback);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(min, Math.min(max, Math.trunc(parsed)));
}

function candidateDedupeKey(item: CandidateWithEvidence): string {
  const company = normalizeIdentityPart(item.candidate.company_domain ?? item.candidate.company);
  const name = normalizeIdentityPart(item.candidate.name);
  const title = normalizeIdentityPart(item.candidate.title);
  if (name && title) return ["person", company, name, title].join("|");
  return ["account", company].join("|");
}

function pushUnique(items: string[], value: string): void {
  if (!items.includes(value)) items.push(value);
}

function normalizeIdentityPart(value: string | null | undefined): string {
  return (value ?? "")
    .replace(/^https?:\/\//i, "")
    .replace(/^www\./i, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim()
    .replace(/\s+/g, " ");
}
