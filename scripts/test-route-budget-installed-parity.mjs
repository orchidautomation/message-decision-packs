#!/usr/bin/env node

import { spawnSync } from "node:child_process";

function usage() {
  console.error(
    "Usage: test-route-budget-installed-parity.mjs --source-bin PATH --installed-bin PATH --dir PACK_DIR",
  );
  process.exit(2);
}

function readFlag(args, name) {
  const index = args.indexOf(name);
  if (index < 0 || !args[index + 1]) usage();
  return args[index + 1];
}

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonicalize(value[key])]),
    );
  }
  return value;
}

function run(binary, args) {
  const result = spawnSync(binary, args, { encoding: "utf8" });
  if (result.error) throw result.error;
  let payload;
  try {
    payload = JSON.parse(result.stdout);
  } catch (error) {
    throw new Error(`${binary} emitted non-JSON route-budget output: ${error.message}\n${result.stdout}`);
  }
  if (payload.ok !== true) {
    throw new Error(`${binary} route-budget command failed: ${JSON.stringify(payload)}`);
  }
  return payload;
}

const args = process.argv.slice(2);
const sourceBin = readFlag(args, "--source-bin");
const installedBin = readFlag(args, "--installed-bin");
const packDir = readFlag(args, "--dir");
const projections = [
  {
    name: "full",
    command: ["--json", "route-budget", "--dir", packDir],
    select: (payload) => payload.data,
  },
  {
    name: "summary",
    command: ["--json", "--summary", "route-budget", "--dir", packDir],
    select: (payload) => payload.summary,
  },
  {
    name: "selector",
    command: [
      "--json",
      "route-budget",
      "--dir",
      packDir,
      "--job",
      "outbound-copy-brief",
      "--persona",
      "PMM",
    ],
    select: (payload) => payload.data,
  },
];

for (const projection of projections) {
  const source = canonicalize(projection.select(run(sourceBin, projection.command)));
  const installed = canonicalize(projection.select(run(installedBin, projection.command)));
  if (JSON.stringify(source) !== JSON.stringify(installed)) {
    console.error(`Installed route-budget ${projection.name} projection differs from source.`);
    console.error(`source=${JSON.stringify(source)}`);
    console.error(`installed=${JSON.stringify(installed)}`);
    process.exit(1);
  }
}

console.log("Installed route-budget parity passed for full, summary, and selector projections.");
