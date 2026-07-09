import { invoke } from '@flue/runtime';
import { flue } from '@flue/runtime/routing';
import { Hono } from 'hono';
import draftResponse from './workflows/draft-response.ts';

const app = new Hono();

app.get('/health', (c) => c.json({ ok: true, service: 'profound-mdp-flue-webhook-agent' }));

app.post('/webhooks/mdp/prospect', async (c) => {
  const payload = await c.req.json().catch(() => null);
  if (!isRecord(payload)) {
    return c.json({ error: 'Expected a JSON webhook payload object.' }, 400);
  }

  const { runId } = await invoke(draftResponse, {
    input: {
      ...payload,
      webhook: payload,
    },
  });

  return c.json({ accepted: true, runId }, 202);
});

app.route('/', flue());

export default app;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
