import { existsSync, readFileSync } from "node:fs";

const required = [
  ".mdp/manifest.yaml",
  ".mdp/source-strategy.json",
  "agent/agent.ts",
  "agent/channels/scout.ts",
  "agent/instructions.md",
  "agent/schedules/weekday-scout.md",
  "agent/sandbox/workspace/.mdp/manifest.yaml",
  "agent/sandbox/workspace/.mdp/source-strategy.json",
  "agent/skills/mdp-lfg/SKILL.md",
  "agent/skills/mdp-source-strategy/SKILL.md",
  "agent/skills/mdp-prospect-brief/SKILL.md",
  "agent/lib/provider-tools.ts",
  "agent/lib/qualification.ts",
  "agent/tools/mdp_validate.ts",
  "agent/tools/discover_candidates.ts",
  "agent/tools/extract_evidence.ts",
  "agent/tools/mdp_fit.ts",
  "agent/tools/mdp_create_brief.ts",
  "agent/tools/append_ledger.ts",
  "samples/profound-public-source-fixture.json"
];

const missing = required.filter((file) => !existsSync(file));
if (missing.length) {
  console.error(`missing required files:\n${missing.join("\n")}`);
  process.exit(1);
}

const instructions = readFileSync("agent/instructions.md", "utf8");
if (/send outreach|update CRM/i.test(instructions.replace(/Do not send outreach|Do not update CRM records/g, ""))) {
  console.error("instructions contain unsafe outreach/CRM language");
  process.exit(1);
}

const sourceStrategy = JSON.parse(readFileSync(".mdp/source-strategy.json", "utf8"));
assertSourceStrategy(sourceStrategy, ".mdp/source-strategy.json");
const sandboxStrategy = JSON.parse(readFileSync("agent/sandbox/workspace/.mdp/source-strategy.json", "utf8"));
assertSourceStrategy(sandboxStrategy, "agent/sandbox/workspace/.mdp/source-strategy.json");

const sourceStrategyLib = readFileSync("agent/lib/source-strategy.ts", "utf8");
if (!sourceStrategyLib.includes("selectPersonResolutionQuery") || !sourceStrategyLib.includes("query_template")) {
  console.error("source strategy loader must expose the person-resolution query template for Eve runtime use");
  process.exit(1);
}
if (!sourceStrategyLib.includes("bundledSourceStrategy")) {
  console.error("source strategy loader must include a bundled fallback for Vercel serverless deployments");
  process.exit(1);
}
if (!sourceStrategyLib.includes("selectScoutQueries") || !sourceStrategyLib.includes("normalizeRunPolicy")) {
  console.error("source strategy loader must expose multi-query scout selection and run-policy normalization");
  process.exit(1);
}

const discoveryLib = readFileSync("agent/lib/discovery.ts", "utf8");
if (!discoveryLib.includes("bundledFixture")) {
  console.error("discovery fixture loader must include a bundled fallback for Vercel serverless deployments");
  process.exit(1);
}
if (!discoveryLib.includes("personResolutionQueryTemplate") || !discoveryLib.includes("renderPersonQueryTemplate")) {
  console.error("discovery must render person-resolution queries from the MDP source strategy template");
  process.exit(1);
}
if (!discoveryLib.includes("resolvePersonForAccount") || !discoveryLib.includes("SCOUT_REQUIRE_PERSON")) {
  console.error("discovery must resolve public person-level owners and require people by default");
  process.exit(1);
}
if (!discoveryLib.includes("input.dryRun === true") || !discoveryLib.includes('mode: "unavailable"')) {
  console.error("discovery must only use fixtures for explicit dry-runs and fail closed when Exa is unavailable");
  process.exit(1);
}
if (!discoveryLib.includes("extractPersonTitleEvidence") || !discoveryLib.includes("boundedWindow")) {
  console.error("person parsing must require bounded name/title co-location");
  process.exit(1);
}

const qualificationLib = readFileSync("agent/lib/qualification.ts", "utf8");
if (!qualificationLib.includes("validateQualifiedCandidate") || !qualificationLib.includes("findPersonResolutionEvidence")) {
  console.error("qualification must share person-evidence validation across scout and Eve tools");
  process.exit(1);
}
if (!qualificationLib.includes("findQualificationSignals") || !qualificationLib.includes("signalEvidenceIds") || !qualificationLib.includes("hasNowSignal")) {
  console.error("qualification must require source-backed fit and why-now signals before ledger append");
  process.exit(1);
}

const mdpRunnerLib = readFileSync("agent/lib/mdp-runner.ts", "utf8");
if (!mdpRunnerLib.includes('process.env.MDP_RUNNER_MODE ?? "native"') || !mdpRunnerLib.includes("Native MDP fit was not run")) {
  console.error("MDP runner must default to native mdp fit and fail closed in simulated mode");
  process.exit(1);
}
if (mdpRunnerLib.includes("Unknown Contact") || mdpRunnerLib.includes("Unknown Role")) {
  console.error("MDP runner must not emit plausible placeholder people that can satisfy person-resolution gates");
  process.exit(1);
}

const providerTools = readFileSync("agent/lib/provider-tools.ts", "utf8");
if (!providerTools.includes("x-exa-integration") || !providerTools.includes("tool({")) {
  console.error("provider tools must expose local AI SDK tool wrappers and Exa integration metadata");
  process.exit(1);
}

const scoutCycleLib = readFileSync("agent/lib/scout-cycle.ts", "utf8");
if (!scoutCycleLib.includes("selectPersonResolutionQuery") || !scoutCycleLib.includes("personResolutionQueryTemplate")) {
  console.error("scout cycle must pass the MDP person-resolution query template into discovery");
  process.exit(1);
}
if (!scoutCycleLib.includes("validateQualifiedCandidate") || !scoutCycleLib.includes("normalizeScoreThreshold")) {
  console.error("scout cycle must validate qualification before ledger append and clamp score thresholds");
  process.exit(1);
}
if (!scoutCycleLib.includes("targetQualified") || !scoutCycleLib.includes("buildDiscoveryQueue") || !scoutCycleLib.includes("continueUntilMinimumQualified")) {
  console.error("scout cycle must enforce the MDP run policy until the target qualified count or bounded exhaustion");
  process.exit(1);
}

const discoverCandidatesTool = readFileSync("agent/tools/discover_candidates.ts", "utf8");
if (!discoverCandidatesTool.includes("selectPersonResolutionQuery") || !discoverCandidatesTool.includes("personResolutionQueryTemplate")) {
  console.error("discover_candidates tool must pass the MDP person-resolution query template into discovery");
  process.exit(1);
}
if (!discoverCandidatesTool.includes("person_resolution_query")) {
  console.error("discover_candidates tool must return the selected person-resolution query trace");
  process.exit(1);
}

const appendLedgerTool = readFileSync("agent/tools/append_ledger.ts", "utf8");
if (!appendLedgerTool.includes("assertQualifiedCandidate") || !appendLedgerTool.includes("person_resolution_evidence_ids")) {
  console.error("append_ledger tool must enforce the shared qualification contract");
  process.exit(1);
}

const scoutChannel = readFileSync("agent/channels/scout.ts", "utf8");
if (!scoutChannel.includes('POST("/scout/run"') || !scoutChannel.includes("runScoutCycle")) {
  console.error("scout channel must expose deterministic POST /scout/run endpoint");
  process.exit(1);
}
if (!scoutChannel.includes("x-mdp-scout-secret") || !scoutChannel.includes("input.dryRun === true")) {
  console.error("scout channel must support protected live runs and public-safe fixture dry-runs");
  process.exit(1);
}
if (!scoutChannel.includes("target_qualified") || !scoutChannel.includes("discovery_passes") || !scoutChannel.includes("exhausted")) {
  console.error("scout channel must report the run-policy target and bounded exhaustion state");
  process.exit(1);
}
if (!scoutChannel.includes("signal_reasons") || !scoutChannel.includes("collectSignalReasons")) {
  console.error("scout channel must expose qualified fit/why-now signal reasons in run responses");
  process.exit(1);
}

const vercelConfig = JSON.parse(readFileSync("vercel.json", "utf8"));
if (!Array.isArray(vercelConfig.crons) || !vercelConfig.crons.some((cron) => cron.path === "/scout/run")) {
  console.error("vercel.json must schedule the deterministic /scout/run endpoint");
  process.exit(1);
}

console.log("ok ai-sdr-eve-vercel scaffold check passed");

function assertSourceStrategy(strategy, label) {
  if (strategy.format !== "mdp.source-strategy.v0") throw new Error(`${label} has unexpected source strategy format`);
  if (strategy.run_policy?.minimum_qualified_people_per_run !== 3) throw new Error(`${label} must target 3 qualified people per live run`);
  if (strategy.run_policy?.continue_until_minimum_qualified !== true) throw new Error(`${label} must continue until the run target is met or bounded discovery is exhausted`);
  if (!Number.isInteger(strategy.run_policy?.max_discovery_passes_per_run) || strategy.run_policy.max_discovery_passes_per_run < 1) throw new Error(`${label} must bound discovery passes`);
  if (!strategy.agent_operating_plan?.downstream_handoff_prompt?.includes("mdp --json fit")) {
    throw new Error(`${label} must include supported MDP CLI fit handoff language`);
  }
  if (!strategy.agent_operating_plan?.downstream_handoff_prompt?.includes("mdp --json check-claims")) {
    throw new Error(`${label} must include supported MDP check-claims handoff language`);
  }
  for (const query of strategy.queries_prompts ?? []) {
    if (!query.agent_instruction || !Array.isArray(query.construction_rules) || query.construction_rules.length === 0) {
      throw new Error(`${label} query ${query.id} must include agent instructions and construction rules`);
    }
    if (!Array.isArray(query.required_receipts) || query.required_receipts.length === 0) {
      throw new Error(`${label} query ${query.id} must include required receipts`);
    }
  }
  const personQuery = strategy.queries_prompts?.find((query) => query.id === "exa-person-role-resolution");
  if (!personQuery) {
    throw new Error(`${label} must include an Exa person-role resolution query`);
  }
  if (!personQuery.query_template?.includes("<company>") || !personQuery.query_template?.includes("<company-domain>")) {
    throw new Error(`${label} person-role resolution query must include a programmatic query_template with company/domain tokens`);
  }
  for (const receipt of ["person_name", "person_title", "person_source_url", "company_match"]) {
    if (!personQuery.required_receipts?.includes(receipt)) throw new Error(`${label} person-role resolution query must require ${receipt}`);
  }
  if (strategy.evidence_requirements?.person_resolution_required !== true) {
    throw new Error(`${label} must require person-level resolution before qualification`);
  }
  if (strategy.evidence_requirements?.minimum_qualified_signals_per_candidate !== 1 || strategy.evidence_requirements?.maximum_qualified_signals_per_candidate !== 3) {
    throw new Error(`${label} must require 1-3 source-backed fit/why-now signals before qualification`);
  }
  if (!String(strategy.evidence_requirements?.signal_gate ?? "").includes("why now")) {
    throw new Error(`${label} must describe the fit and why-now signal gate`);
  }
}
