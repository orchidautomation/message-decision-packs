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
