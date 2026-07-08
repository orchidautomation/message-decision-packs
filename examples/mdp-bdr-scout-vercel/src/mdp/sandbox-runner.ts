import type { MdpRunnerInput } from "./runner.ts";
import type { MdpDecision } from "../schemas/ledger.ts";

export async function runSandboxMdp(_input: MdpRunnerInput): Promise<MdpDecision> {
  throw new Error("Vercel Sandbox MDP runner is a planned follow-up. Keep native/simulated mode until sandbox package, binary download, and artifact readback are implemented.");
}
