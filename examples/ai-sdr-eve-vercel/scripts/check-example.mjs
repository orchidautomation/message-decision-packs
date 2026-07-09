import { existsSync, readFileSync } from "node:fs";

const required = [
  ".mdp/manifest.yaml",
  ".mdp/source-strategy.json",
  "agent/agent.ts",
  "agent/instructions.md",
  "agent/schedules/weekday-scout.md",
  "agent/sandbox/workspace/.mdp/manifest.yaml",
  "agent/sandbox/workspace/.mdp/source-strategy.json",
  "agent/skills/mdp-lfg/SKILL.md",
  "agent/skills/mdp-source-strategy/SKILL.md",
  "agent/skills/mdp-prospect-brief/SKILL.md",
  "agent/lib/provider-tools.ts",
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

const providerTools = readFileSync("agent/lib/provider-tools.ts", "utf8");
if (!providerTools.includes("x-exa-integration") || !providerTools.includes("tool({")) {
  console.error("provider tools must expose local AI SDK tool wrappers and Exa integration metadata");
  process.exit(1);
}

console.log("ok ai-sdr-eve-vercel scaffold check passed");

function assertSourceStrategy(strategy, label) {
  if (strategy.format !== "mdp.source-strategy.v0") throw new Error(`${label} has unexpected source strategy format`);
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
}
