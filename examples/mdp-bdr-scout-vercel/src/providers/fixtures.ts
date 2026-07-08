import { readFile } from "node:fs/promises";
import defaultFixtureJson from "../../samples/public-source-fixture.json" with { type: "json" };
import { fileURLToPath } from "node:url";
import { assertCandidateWithEvidence, type CandidateWithEvidence } from "../schemas/candidate.ts";

export function getDefaultFixture(): CandidateWithEvidence {
  assertCandidateWithEvidence(defaultFixtureJson);
  return defaultFixtureJson;
}

export async function readFixture(pathOrUrl: string | URL): Promise<CandidateWithEvidence> {
  const filePath = pathOrUrl instanceof URL ? fileURLToPath(pathOrUrl) : pathOrUrl;
  const parsed = JSON.parse(await readFile(filePath, "utf8"));
  assertCandidateWithEvidence(parsed);
  return parsed;
}
