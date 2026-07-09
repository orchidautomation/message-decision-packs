import type { MdpRunnerInput } from "./runner.ts";
import type { MdpDecision } from "../schemas/ledger.ts";

export async function runSandboxMdp(_input: MdpRunnerInput): Promise<MdpDecision> {
  throw new Error("Vercel Sandbox MDP runner is intentionally gated until a project has @vercel/sandbox credentials, binary download policy, and artifact readback configured. Use native mode locally first.");
}
