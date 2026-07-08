import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { assertCandidateWithEvidence, type CandidateWithEvidence } from "../schemas/candidate.ts";

export async function readFixture(pathOrUrl: string | URL): Promise<CandidateWithEvidence> {
  const filePath = pathOrUrl instanceof URL ? fileURLToPath(pathOrUrl) : pathOrUrl;
  const parsed = JSON.parse(await readFile(filePath, "utf8"));
  assertCandidateWithEvidence(parsed);
  return parsed;
}
