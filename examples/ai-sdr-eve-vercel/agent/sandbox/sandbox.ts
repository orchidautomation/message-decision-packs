import { defineSandbox } from "eve/sandbox";

export default defineSandbox({
  async onSession({ use }) {
    const sandbox = await use();
    await sandbox.writeTextFile({
      path: "RUNBOOK.md",
      content: "# MDP AI SDR Sandbox\n\nThis workspace is seeded with `.mdp/`. Prefer Eve typed tools for production MDP gates; use sandbox bash for inspection or approved CLI-in-sandbox experiments.\n"
    });
  }
});
