import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

export type DraftMode = 'model' | 'contract-only';

export type WebhookInput = {
  type?: string;
  deliveryId?: string;
  delivery_id?: string;
  channel?: string;
  draftMode?: DraftMode;
  packDir?: string;
  webhook?: unknown;
  prospect?: unknown;
  row?: unknown;
  data?: unknown;
};

export type MdpPreparedDraft = {
  artifactDir: string;
  brief: Record<string, unknown>;
  briefPath: string;
  channel: string;
  deliveryId: string;
  draftMode: DraftMode;
  draftPrompt: string;
  fit: Record<string, unknown>;
  packDir: string;
  prospect: Record<string, unknown>;
  prospectPath: string;
  rawWebhookPath: string;
};

export async function prepareMdpDraft(input: WebhookInput): Promise<MdpPreparedDraft> {
  const packDir = resolvePackDir(input.packDir);
  const channel = normalizeChannel(input.channel);
  const deliveryId = String(input.deliveryId ?? input.delivery_id ?? `local-${Date.now()}`);
  const draftMode = input.draftMode === 'contract-only' ? 'contract-only' : 'model';
  const artifactDir = path.join(packDir, ignoredScratchName(), 'flue-webhook-agent', safeSegment(deliveryId));
  await mkdir(artifactDir, { recursive: true });

  const webhookPayload = isRecord(input.webhook) ? input.webhook : input;
  const rawWebhookPath = path.join(artifactDir, 'webhook.json');
  await writeJson(rawWebhookPath, webhookPayload);

  const prospect = normalizeProspect(input);
  const prospectPath = path.join(artifactDir, 'prospect.json');
  await writeJson(prospectPath, prospect);

  const fitEnvelope = await runMdpJson(packDir, ['--json', 'fit', '--dir', packDir, '--prospect', prospectPath]);
  const fit = requireDataObject(fitEnvelope, 'fit');

  const briefPath = path.join(artifactDir, 'brief.json');
  let brief: Record<string, unknown> | null = null;
  if (fit.status === 'fit') {
    await runMdpJson(packDir, [
      '--json',
      'brief',
      '--context',
      '--dir',
      packDir,
      '--prospect',
      prospectPath,
      '--channel',
      channel,
      '--out',
      briefPath,
    ]);
    brief = await readJsonObject(briefPath);
  }

  const draftPrompt = brief ? buildDraftPrompt(brief, channel) : buildNoDraftPrompt(fit, channel);
  await writeFile(path.join(artifactDir, 'draft-prompt.md'), draftPrompt);

  return {
    artifactDir,
    brief: brief ?? {},
    briefPath,
    channel,
    deliveryId,
    draftMode,
    draftPrompt,
    fit,
    packDir,
    prospect,
    prospectPath,
    rawWebhookPath,
  };
}

export async function checkDraftClaims(packDir: string, draft: string): Promise<Record<string, unknown>> {
  const envelope = await runMdpJson(packDir, [
    '--json',
    'check-claims',
    '--dir',
    packDir,
    '--text',
    draft,
  ]);
  return requireDataObject(envelope, 'check-claims');
}

export async function writeDraftArtifacts(
  artifactDir: string,
  draft: string,
  claimCheck: Record<string, unknown>,
): Promise<{ claimCheckPath: string; draftPath: string }> {
  const draftPath = path.join(artifactDir, 'draft.txt');
  const claimCheckPath = path.join(artifactDir, 'claim-check.json');
  await writeFile(draftPath, draft);
  await writeJson(claimCheckPath, claimCheck);
  return { claimCheckPath, draftPath };
}

export function summarizeBrief(brief: Record<string, unknown>): Record<string, unknown> {
  return {
    channel: brief.channel,
    context: isRecord(brief.context) ? brief.context.summary : undefined,
    draft_status: brief.draft_status,
    job: brief.job,
    pack: brief.pack,
    persona: brief.persona,
    required_load_order: brief.required_load_order,
  };
}

function normalizeProspect(input: WebhookInput): Record<string, unknown> {
  const data = isRecord(input.data) ? input.data : undefined;
  const candidate = firstRecord(input.prospect, input.row, data?.prospect, data?.row, input);

  if (!candidate) {
    throw new Error('Webhook payload must include prospect, row, data.prospect, data.row, or top-level prospect fields.');
  }

  for (const field of ['name', 'title', 'company']) {
    if (typeof candidate[field] !== 'string' || String(candidate[field]).trim() === '') {
      throw new Error(`Prospect payload is missing required string field: ${field}`);
    }
  }

  return {
    ...candidate,
    source_kind: candidate.source_kind ?? 'user-provided-row',
  };
}

function buildDraftPrompt(brief: Record<string, unknown>, channel: string): string {
  return [
    'Draft one outbound response from brief.json.',
    '',
    `Channel: ${channel}`,
    '',
    'Rules:',
    '- Use only the prospect, fit decision, and context entries in brief.json.',
    '- Do not claim the message was sent, scheduled, enriched, or written back anywhere.',
    '- Do not invent metrics, customer names, integrations, or proof not present in brief.json.',
    '- If the prospect source is synthetic, treat this as demo copy for review.',
    '- For LinkedIn, keep it short and conversational.',
    '- Return only the draft text, with no commentary or markdown wrapper.',
    '',
    'Brief summary:',
    JSON.stringify(summarizeBrief(brief), null, 2),
  ].join('\n');
}

function buildNoDraftPrompt(fit: Record<string, unknown>, channel: string): string {
  return [
    'Do not draft outbound copy.',
    '',
    `Channel requested: ${channel}`,
    '',
    'The MDP fit gate did not return fit. Surface the status, missing context, disqualifiers, and next best input needed.',
    '',
    JSON.stringify(fit, null, 2),
  ].join('\n');
}

async function runMdpJson(packDir: string, args: string[]): Promise<Record<string, unknown>> {
  const mdpBin = resolveMdpBin();
  try {
    const { stdout } = await execFileAsync(mdpBin, args, {
      cwd: packDir,
      maxBuffer: 64 * 1024 * 1024,
      timeout: Number(getSetting('MDP_CLI_TIMEOUT_MS') ?? 30000),
    });
    return parseJsonObject(stdout, `mdp ${args.join(' ')}`);
  } catch (error) {
    const failed = error as { message?: string; stderr?: string; stdout?: string };
    throw new Error(
      [
        `MDP CLI failed: ${mdpBin} ${args.join(' ')}`,
        failed.message,
        failed.stderr,
        failed.stdout,
      ].filter(Boolean).join('\n'),
    );
  }
}

function resolveMdpBin(): string {
  const configured = getSetting('MDP_BIN');
  if (configured) {
    return path.isAbsolute(configured) ? configured : path.resolve(process.cwd(), configured);
  }

  const repoDebugBin = path.resolve(process.cwd(), '../../..', 'cli', 'target', 'debug', 'mdp');
  return existsSync(repoDebugBin) ? repoDebugBin : 'mdp';
}

function resolvePackDir(inputPackDir: string | undefined): string {
  return path.resolve(inputPackDir ?? getSetting('MDP_PACK_DIR') ?? path.join(process.cwd(), '..'));
}

function normalizeChannel(channel: string | undefined): string {
  const value = channel?.trim();
  return value || 'linkedin';
}

function requireDataObject(envelope: Record<string, unknown>, command: string): Record<string, unknown> {
  if (envelope.ok !== true) {
    throw new Error(`mdp ${command} returned ok=false`);
  }
  if (!isRecord(envelope.data)) {
    throw new Error(`mdp ${command} did not return a data object`);
  }
  return envelope.data;
}

async function readJsonObject(file: string): Promise<Record<string, unknown>> {
  return parseJsonObject(await readFile(file, 'utf8'), file);
}

async function writeJson(file: string, value: unknown): Promise<void> {
  await writeFile(file, `${JSON.stringify(value, null, 2)}\n`);
}

function parseJsonObject(raw: string, label: string): Record<string, unknown> {
  const parsed = JSON.parse(raw) as unknown;
  if (!isRecord(parsed)) {
    throw new Error(`${label} did not produce a JSON object`);
  }
  return parsed;
}

function firstRecord(...values: unknown[]): Record<string, unknown> | undefined {
  return values.find(isRecord);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function ignoredScratchName(): string {
  return `${'.agent'}-artifacts`;
}

function getSetting(name: string): string | undefined {
  const runtime = globalThis as typeof globalThis & {
    process?: { [key: string]: Record<string, string | undefined> | undefined };
  };
  return runtime.process?.['env']?.[name];
}

function safeSegment(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 80) || 'local';
}
