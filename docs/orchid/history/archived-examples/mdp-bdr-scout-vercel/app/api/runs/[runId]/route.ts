import { getRun } from "workflow/api";

export async function GET(_request: Request, context: { params: Promise<{ runId: string }> }): Promise<Response> {
  const { runId } = await context.params;
  try {
    const run = await getRun(runId);
    const [status, workflowName, createdAt, startedAt, completedAt] = await Promise.all([
      run.status,
      run.workflowName,
      run.createdAt,
      run.startedAt,
      run.completedAt
    ]);

    return Response.json({
      runId,
      status,
      workflowName,
      createdAt: createdAt.toISOString(),
      startedAt: startedAt?.toISOString() ?? null,
      completedAt: completedAt?.toISOString() ?? null
    });
  } catch {
    return Response.json({ ok: false, error: { code: "RUN_NOT_FOUND", message: `Run ${runId} not found` } }, { status: 404 });
  }
}
