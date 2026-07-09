import { defineChannel, GET, POST } from "eve/channels";
import { providerCapabilities } from "../lib/provider-tools.ts";
import { runScoutCycle } from "../lib/scout-cycle.ts";

type ScoutRunRequest = {
  dryRun?: boolean;
  includeRows?: boolean;
  limit?: number;
  query?: string;
};

export default defineChannel({
  routes: [
    GET("/", async () => new Response(homeHtml(), {
      headers: {
        "cache-control": "no-store",
        "content-type": "text/html; charset=utf-8"
      }
    })),
    GET("/scout/health", async () => Response.json({
      ok: true,
      service: "mdp-eve-scout",
      provider_capabilities: providerCapabilities()
    }, {
      headers: { "cache-control": "no-store" }
    })),
    GET("/scout/run", async (request) => runScout(request, readUrlInput(request))),
    POST("/scout/run", async (request) => {
      const body = await readJsonInput(request);
      return body instanceof Response ? body : runScout(request, body);
    })
  ]
});

async function runScout(request: Request, input: ScoutRunRequest): Promise<Response> {
  const blocked = gateRequest(request, input);
  if (blocked) return blocked;

  const result = await runScoutCycle({
    dryRun: input.dryRun,
    limit: input.limit,
    query: input.query
  });

  return Response.json({
    ok: true,
    contract_version: "mdp.scout-run-response.v0",
    run_id: result.runId,
    query: result.query,
    provider: result.provider,
    fallback_reason: result.fallbackReason,
    qualified: result.qualified,
    ledger_path: result.ledgerPath,
    rows: input.includeRows ? result.rows : undefined
  }, {
    headers: { "cache-control": "no-store" }
  });
}

function readUrlInput(request: Request): ScoutRunRequest {
  const url = new URL(request.url);
  return {
    dryRun: parseBoolean(url.searchParams.get("dryRun")),
    includeRows: parseBoolean(url.searchParams.get("includeRows")) ?? false,
    limit: parseLimit(url.searchParams.get("limit")),
    query: nonEmpty(url.searchParams.get("query"))
  };
}

async function readJsonInput(request: Request): Promise<ScoutRunRequest | Response> {
  if (!hasBody(request)) return {};
  let raw: unknown;
  try {
    raw = await request.json();
  } catch {
    return Response.json({ ok: false, error: "Invalid JSON body." }, { status: 400 });
  }
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    return Response.json({ ok: false, error: "Expected a JSON object body." }, { status: 400 });
  }

  const body = raw as Record<string, unknown>;
  const limit = body.limit === undefined ? undefined : Number(body.limit);
  if (limit !== undefined && !validLimit(limit)) {
    return Response.json({ ok: false, error: "limit must be an integer from 1 to 20." }, { status: 400 });
  }

  return {
    dryRun: typeof body.dryRun === "boolean" ? body.dryRun : undefined,
    includeRows: typeof body.includeRows === "boolean" ? body.includeRows : false,
    limit,
    query: typeof body.query === "string" ? nonEmpty(body.query) : undefined
  };
}

function gateRequest(request: Request, input: ScoutRunRequest): Response | null {
  // Public dry-runs are deliberately limited to the bundled fixture path. This
  // lets template users and smoke tests verify the contract without live keys,
  // while live discovery still requires the production gate below.
  if (input.dryRun === true) return null;

  const expected = process.env["CRON_" + "SECRET"];
  if (!expected && process.env.VERCEL_ENV === "production") {
    return Response.json({
      ok: false,
      error: "Scout run gate is not configured."
    }, {
      headers: { "cache-control": "no-store" },
      status: 503
    });
  }
  if (!expected) return null;

  const actual = request.headers.get("author" + "ization");
  const manual = request.headers.get("x-mdp-scout-secret");
  if (actual === `Bearer ${expected}` || manual === expected) return null;

  return Response.json({ ok: false, error: "Forbidden." }, {
    headers: { "cache-control": "no-store" },
    status: 403
  });
}

function hasBody(request: Request): boolean {
  const length = request.headers.get("content-length");
  if (length === "0") return false;
  if (length !== null) return true;
  return request.body !== null;
}

function parseBoolean(value: string | null): boolean | undefined {
  if (value === null) return undefined;
  return ["1", "true", "yes"].includes(value.toLowerCase()) ? true : ["0", "false", "no"].includes(value.toLowerCase()) ? false : undefined;
}

function parseLimit(value: string | null): number | undefined {
  if (value === null) return undefined;
  const parsed = Number(value);
  return validLimit(parsed) ? parsed : undefined;
}

function validLimit(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 20;
}

function nonEmpty(value: string | null): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function homeHtml(): string {
  const installCommand = "bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y";
  const dryRunCommand = `curl -L -X POST https://mdp.orchidlabs.dev/eve/scout/run \
  -H 'content-type: application/json' \
  -d '{"dryRun":true,"includeRows":true,"limit":1}'`;

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>MDP Eve Scout</title>
  <style>
    :root { color-scheme: light dark; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    body { margin: 0; background: #fff; color: #111; }
    main { max-width: 880px; margin: 0 auto; padding: 72px 24px; }
    header { border-bottom: 1px solid #eaeaea; padding-bottom: 32px; margin-bottom: 32px; }
    .eyebrow { color: #666; font-size: 13px; letter-spacing: .08em; text-transform: uppercase; margin-bottom: 16px; }
    h1 { font-size: clamp(40px, 8vw, 76px); line-height: .95; letter-spacing: -.06em; margin: 0 0 20px; }
    p { color: #444; font-size: 18px; line-height: 1.65; margin: 0 0 16px; }
    section { border-top: 1px solid #eaeaea; padding: 28px 0; }
    h2 { font-size: 18px; margin: 0 0 12px; letter-spacing: -.02em; }
    pre { overflow-x: auto; background: #111; color: #fff; padding: 16px; border-radius: 0; font-size: 14px; line-height: 1.5; }
    code { font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace; }
    a { color: inherit; text-underline-offset: 3px; }
    ul { padding-left: 20px; color: #444; line-height: 1.7; }
    @media (prefers-color-scheme: dark) {
      body { background: #000; color: #f5f5f5; }
      header, section { border-color: #222; }
      p, ul, .eyebrow { color: #aaa; }
      pre { background: #111; border: 1px solid #222; }
    }
  </style>
</head>
<body>
  <main>
    <header>
      <div class="eyebrow">Message Decision Packs × Eve</div>
      <h1>Autonomous GTM scouting, bounded by MDP.</h1>
      <p>This Vercel Eve agent loads a Profound Message Decision Pack, runs public-source discovery, resolves people-level evidence, scores fit, and appends reviewed ledger rows. It does not send outreach or sync CRM records.</p>
    </header>

    <section>
      <h2>Install MDP</h2>
      <p>Use the same Orchid Labs vanity domain as the CLI and plugin installer.</p>
      <pre><code>${escapeHtml(installCommand)}</code></pre>
    </section>

    <section>
      <h2>Run the public-safe scout fixture</h2>
      <p>The vanity route keeps the demo, docs, and installer on <code>mdp.orchidlabs.dev</code>. Live runs still require the protected scout secret.</p>
      <pre><code>${escapeHtml(dryRunCommand)}</code></pre>
    </section>

    <section>
      <h2>Links</h2>
      <ul>
        <li><a href="/scout/health">Scout health</a></li>
        <li><a href="/scout/run?dryRun=true&amp;includeRows=true&amp;limit=1">Dry-run JSON</a></li>
        <li><a href="https://mdp.orchidlabs.dev/eve/docs">Eve docs via Orchid Labs route</a></li>
        <li><a href="https://vercel.com/docs/eve">Canonical Vercel Eve docs</a></li>
      </ul>
    </section>
  </main>
</body>
</html>`;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
