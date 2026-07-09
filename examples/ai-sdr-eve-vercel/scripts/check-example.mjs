import { existsSync, readFileSync } from "node:fs";

const required = [
  ".mdp/manifest.yaml",
  ".mdp/source-strategy.json",
  "agent/agent.ts",
  "agent/instructions.md",
  "agent/schedules/weekday-scout.md",
  "agent/sandbox/workspace/.mdp/manifest.yaml",
  "agent/skills/mdp-lfg/SKILL.md",
  "agent/skills/mdp-source-strategy/SKILL.md",
  "agent/skills/mdp-prospect-brief/SKILL.md",
  "agent/tools/mdp_validate.ts",
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

console.log("ok ai-sdr-eve-vercel scaffold check passed");
