import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const exampleRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(exampleRoot, "../..");
const mdp = process.env.MDP_BIN ?? "mdp";
const result = spawnSync(mdp, [
  "--json",
  "brief",
  "--dir",
  "examples/ai-sdr-eve-vercel",
  "--prospect",
  "examples/ai-sdr-eve-vercel/samples/synthetic-prospect.json",
  "--channel",
  "linkedin",
  "--job",
  "linkedin outbound copy",
  "--context"
], { cwd: repoRoot, encoding: "utf8" });

if (result.status !== 0) {
  throw new Error(`Canonical Eve brief command failed:\n${result.stderr || result.stdout}`);
}

const payload = JSON.parse(result.stdout);
if (payload.ok !== true || payload.data?.draft_status !== "ready") {
  throw new Error(`Canonical Eve brief command did not produce a ready brief:\n${result.stdout}`);
}

console.log("ok canonical Eve brief command passed");
