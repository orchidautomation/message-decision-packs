import { join, resolve } from "node:path";

export function appRoot(): string {
  return resolve(process.cwd());
}

export function packRoot(): string {
  return resolve(process.env.MDP_PACK_ROOT ?? appRoot());
}

export function packDir(): string {
  return join(packRoot(), ".mdp");
}

export function outputDir(): string {
  return resolve(process.env.SCOUT_OUTPUT_DIR ?? (process.env.VERCEL ? "/tmp/mdp-scout-artifacts" : join(appRoot(), "artifacts")));
}

export function fixturePath(): string {
  return resolve(process.env.SCOUT_FIXTURE_PATH ?? join(appRoot(), "samples", "synthetic-public-source-fixture.json"));
}
