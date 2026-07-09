import { defineTool } from "eve/tools";
import { z } from "zod";
import { capabilityFor, runFirecrawlScrape } from "../lib/provider-tools.ts";

export default defineTool({
  description: "Optionally extract clean evidence from an already accepted public URL with Firecrawl. If FIRECRAWL_API_KEY is absent, returns an explicit unavailable result instead of pretending extraction ran.",
  inputSchema: z.object({ url: z.string().url() }),
  async execute({ url }) {
    const capability = capabilityFor("firecrawl");
    if (!capability.enabled) {
      return {
        ok: false,
        provider: "firecrawl" as const,
        capability,
        evidence: null,
        gap: "Firecrawl accepted-URL cleanup skipped because FIRECRAWL_API_KEY is not configured. Preserve the original Exa/source receipt or route to operator review."
      };
    }

    const evidence = await runFirecrawlScrape({ url });
    return { ok: true, provider: "firecrawl" as const, capability, evidence, gap: null };
  }
});
