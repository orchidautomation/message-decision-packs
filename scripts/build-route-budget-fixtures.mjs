#!/usr/bin/env node
// Regenerates the synthetic route-budget example packs under
// examples/route-budget/{overflow,ready}. The packs are copied from the
// starter GTM template and patched so a single declared persona ("Buyer")
// either overflows the declared outbound-copy-brief context budget
// (overflow pack) or fits it through narrower structured applicability
// (ready pack).
//
// The packs are intentionally synthetic and public-safe: no customer data,
// no real proof, no audited Sanity.io content. Run after editing the template
// or this script to keep the committed fixtures in parity:
//
//   node scripts/build-route-budget-fixtures.mjs
//
// This script writes the committed fixture directories in place. It does not
// mutate the audited starter template or any customer pack.

import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const templateDir = join(root, "plugin", "assets", "templates", "basic");
const exampleRoot = join(root, "examples", "route-budget");

// Ninety-nine synthetic case-study entries, all stamped applies_to: Buyer.
// Each is deliberately generic so the pack never claims real proof or past
// performance. The bodies are padded so the pack also overflows the byte
// budget, mirroring the 99-persona-match plus guardrail failure shape.
const overflowCaseStudies = Array.from({ length: 99 }, (_, index) => ({
  id: `buyer-case-${String(index + 1).padStart(3, "0")}`,
  title: `Buyer case study ${index + 1}`,
  body: `Buyer context note ${index + 1}: a synthetic persona-scoped case-study entry used only to exercise route-budget preflight. It contains enough canonical prose to contribute measurable bytes to the routed context for the Buyer persona on the outbound-copy-brief job without asserting any real customer outcome, certification, compliance status, or past performance.`,
  applies_to: ["Buyer"],
  evidence: [],
  avoid: [],
}));

// Five Buyer-relevant entries with narrow applicability so the ready pack
// stays under the 64-entry / 65536-byte budget through structure, not by
// raising limits or dropping guardrails.
const readyCaseStudies = Array.from({ length: 5 }, (_, index) => ({
  id: `buyer-case-${String(index + 1).padStart(3, "0")}`,
  title: `Buyer case study ${index + 1}`,
  body: `Buyer context note ${index + 1}: a short synthetic persona-scoped entry kept narrow so the routed context fits the declared budget.`,
  applies_to: ["Buyer"],
  evidence: [],
  avoid: [],
}));

function writeCaseStudiesCard(path, entries) {
  const lines = [
    "id: buyer-case-studies",
    "kind: claims",
    "title: Synthetic Buyer case studies",
    "description: Synthetic Buyer-persona case studies for route-budget preflight demonstration.",
    "personas:",
    "- Buyer",
    "tags:",
    "- buyer",
    "- case-study",
    "- synthetic",
    "- route-budget",
    "entries:",
  ];
  for (const entry of entries) {
    lines.push(`- id: ${entry.id}`);
    lines.push(`  title: ${entry.title}`);
    lines.push(`  body: ${JSON.stringify(entry.body)}`);
    lines.push("  applies_to:");
    for (const persona of entry.applies_to) {
      lines.push(`  - ${persona}`);
    }
    lines.push("  evidence: []");
    lines.push("  avoid: []");
  }
  writeFileSync(path, lines.join("\n") + "\n");
}

function patchManifest(manifestPath, { budget }) {
  let raw = readFileSync(manifestPath, "utf8");
  // Declare the Buyer persona used by the synthetic case-study card.
  raw = raw.replace(
    /^personas:\n- GTM Engineering\n- PMM\n- PM\n/m,
    "personas:\n- GTM Engineering\n- PMM\n- PM\n- Buyer\n",
  );
  // Add the buyer-case-studies card reference after the existing claims card.
  raw = raw.replace(
    /^- id: claims\n  path: cards\/claims.yaml\n  kind: claims\n  description: Approved claims and proof requirements an agent may use.\n  personas:\n  - PMM\n  - GTM Engineering\n  tags:\n  - claim\n  - proof\n  - evidence\n/m,
    (match) =>
      `${match}- id: buyer-case-studies\n  path: cards/buyer-case-studies.yaml\n  kind: claims\n  description: Synthetic Buyer-persona case studies for route-budget preflight demonstration.\n  personas:\n  - Buyer\n  tags:\n  - buyer\n  - case-study\n  - synthetic\n  - route-budget\n`,
  );
  // Keep the outbound-copy-brief budget at 64/65536 so the overflow fixture
  // overflows through applicability, not larger limits.
  raw = raw.replace(
    /^  context_budget:\n    max_entries: 64\n    max_bytes: 65536\n- id: outbound-copy-review\n/m,
    `  context_budget:\n    max_entries: ${budget.max_entries}\n    max_bytes: ${budget.max_bytes}\n- id: outbound-copy-review\n`,
  );
  writeFileSync(manifestPath, raw);
}

function buildFixture(name, { budget, entries }) {
  const dest = join(exampleRoot, name);
  rmSync(dest, { recursive: true, force: true });
  mkdirSync(dest, { recursive: true });
  cpSync(join(templateDir, ".mdp"), join(dest, ".mdp"), { recursive: true });
  const manifestPath = join(dest, ".mdp", "manifest.yaml");
  patchManifest(manifestPath, { budget });
  writeCaseStudiesCard(join(dest, ".mdp", "cards", "buyer-case-studies.yaml"), entries);
  return dest;
}

const budget = { max_entries: 64, max_bytes: 65536 };
const overflowDest = buildFixture("overflow", { budget, entries: overflowCaseStudies });
const readyDest = buildFixture("ready", { budget, entries: readyCaseStudies });

// Synthetic prospect for the ready fixture's brief --context dry-run. It uses
// the PMM persona (fully supported by the starter template) so the ready
// fixture demonstrates a draft-ready governed context, while the
// route-budget preflight separately confirms every declared persona
// (including Buyer) fits the declared budget.
const prospect = {
  name: "Dana Rivera",
  title: "VP Product Marketing",
  company: "Acme Synthetics",
  company_domain: "acme-synthetics.example",
  source_kind: "synthetic-example",
  segment: "agent-assisted GTM",
  persona: "PMM",
  trigger: "Team adopted a local CLI for outbound copy and wants versioned context.",
  background: "Synthetic prospect for route-budget demonstration only.",
  signals: [
    {
      id: "pmm-signal-1",
      title: "PMM asked about route-budget guardrails",
      source: "synthetic-example",
    },
  ],
};
const prospectPath = join(readyDest, "synthetic-prospect.json");
writeFileSync(prospectPath, JSON.stringify(prospect, null, 2) + "\n");

console.log(`overflow fixture: ${overflowDest}`);
console.log(`ready fixture: ${readyDest}`);
console.log(`ready prospect: ${prospectPath}`);
