export async function GET(_request: Request, context: { params: Promise<{ runId: string }> }): Promise<Response> {
  const { runId } = await context.params;
  return Response.json({
    runId,
    status: "stub",
    note: "Connect this route to Neon scout_runs in the storage adapter."
  });
}
