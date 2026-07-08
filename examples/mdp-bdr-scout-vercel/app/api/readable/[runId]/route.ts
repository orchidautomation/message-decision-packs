import { getRun } from "workflow/api";

export async function GET(_request: Request, context: { params: Promise<{ runId: string }> }): Promise<Response> {
  const { runId } = await context.params;
  let run;
  try {
    run = await getRun(runId);
  } catch {
    return Response.json({ ok: false, error: { code: "RUN_NOT_FOUND", message: `Run ${runId} not found` } }, { status: 404 });
  }

  const readable = run.getReadable();
  const encoder = new TextEncoder();
  const sseStream = (readable as unknown as ReadableStream).pipeThrough(
    new TransformStream({
      transform(chunk, controller) {
        const data = typeof chunk === "string" ? chunk : JSON.stringify(chunk);
        controller.enqueue(encoder.encode(`data: ${data}

`));
      }
    })
  );

  return new Response(sseStream, {
    headers: {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no"
    }
  });
}
