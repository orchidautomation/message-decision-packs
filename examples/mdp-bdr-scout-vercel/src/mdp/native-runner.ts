import { mkdtemp, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { MdpRunnerInput } from "./runner.ts";
import type { MdpDecision } from "../schemas/ledger.ts";

const execFileAsync = promisify(execFile);

export async function runNativeMdp(input: MdpRunnerInput): Promise<MdpDecision> {
  const mdpBin = process.env.MDP_BIN ?? "mdp";
  const packDir = process.env.MDP_PACK_DIR;
  if (!packDir) throw new Error("MDP_PACK_DIR is required for native MDP runner mode");

  const scratch = await mkdtemp(join(tmpdir(), "mdp-bdr-scout-"));
  try {
    const prospectPath = join(scratch, "prospect.json");
    await writeFile(prospectPath, JSON.stringify(input.candidate, null, 2));

    const fit = await execFileAsync(mdpBin, ["--json", "fit", "--dir", packDir, "--prospect", prospectPath], { timeout: 60_000 });
    const fitJson = JSON.parse(fit.stdout);

    let briefJsonUrl: string | null = null;
    try {
      const brief = await execFileAsync(mdpBin, ["--json", "brief", "--context", "--dir", packDir, "--prospect", prospectPath], { timeout: 60_000 });
      const briefPath = join(scratch, "brief.json");
      await writeFile(briefPath, brief.stdout);
      JSON.parse(await readFile(briefPath, "utf8"));
      briefJsonUrl = briefPath;
    } catch {
      briefJsonUrl = null;
    }

    return {
      fit_status: fitJson.status === "fit" || fitJson.fit === true ? "fit" : "insufficient_context",
      persona: input.candidate.persona ?? input.candidate.title,
      route: fitJson.route ?? "operator_review",
      brief_json_url: briefJsonUrl,
      brief_md_url: null,
      gaps: Array.isArray(fitJson.gaps) ? fitJson.gaps : []
    };
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}
