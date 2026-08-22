#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { relative, join } from "node:path";
import { spawnSync } from "node:child_process";

function usage() {
  console.error(
    "Usage: test-route-budget-installed-parity.mjs --source-bin PATH --installed-bin PATH --source-assets PATH --installed-assets PATH --dir PACK_DIR",
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

function assetInventory(root) {
  const records = [];
  const walk = (directory) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name);
      const stats = lstatSync(path);
      if (stats.isSymbolicLink()) {
        throw new Error(`Asset tree contains a symbolic link: ${path}`);
      }
      if (stats.isDirectory()) {
        walk(path);
      } else if (stats.isFile()) {
        records.push({
          path: relative(root, path).split("\\").join("/"),
          sha256: createHash("sha256").update(readFileSync(path)).digest("hex"),
        });
      } else {
        throw new Error(`Asset tree contains a non-regular file: ${path}`);
      }
    }
  };
  walk(root);
  return records;
}

function assertAssetParity(sourceAssets, installedAssets) {
  const source = assetInventory(sourceAssets);
  const installed = assetInventory(installedAssets);
  if (JSON.stringify(source) !== JSON.stringify(installed)) {
    throw new Error(
      `Installed authored assets differ from source assets:\nsource=${JSON.stringify(source)}\ninstalled=${JSON.stringify(installed)}`,
    );
  }
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
const sourceAssets = readFlag(args, "--source-assets");
const installedAssets = readFlag(args, "--installed-assets");
const packDir = readFlag(args, "--dir");
assertAssetParity(sourceAssets, installedAssets);
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
