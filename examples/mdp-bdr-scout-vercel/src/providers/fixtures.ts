import { readFile } from "node:fs/promises";
import defaultFixtureJson from "../../samples/public-source-fixture.json" with { type: "json" };
import profoundFixtureJson from "../../samples/profound-public-source-fixture.json" with { type: "json" };
import { fileURLToPath } from "node:url";
import { assertCandidateWithEvidence, type CandidateWithEvidence } from "../schemas/candidate.ts";

export function getDefaultFixture(): CandidateWithEvidence {
  assertCandidateWithEvidence(defaultFixtureJson);
  return defaultFixtureJson;
}

export async function readFixture(pathOrUrl: string | URL): Promise<CandidateWithEvidence> {
  const filePath = pathOrUrl instanceof URL ? fileURLToPath(pathOrUrl) : pathOrUrl;
  const parsed = resolveBuiltInFixture(filePath) ?? JSON.parse(await readFile(filePath, "utf8"));
  assertCandidateWithEvidence(parsed);
  return parsed;
}

function resolveBuiltInFixture(filePath: string): unknown | null {
  if (filePath === "profound" || filePath.endsWith("/profound-public-source-fixture.json") || filePath === "samples/profound-public-source-fixture.json") {
    return profoundFixtureJson;
  }
  if (filePath === "default" || filePath.endsWith("/public-source-fixture.json") || filePath === "samples/public-source-fixture.json") {
    return defaultFixtureJson;
  }
  return null;
}
