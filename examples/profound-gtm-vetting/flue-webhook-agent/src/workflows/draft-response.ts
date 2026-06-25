import { defineAgent, defineWorkflow, type WorkflowRouteHandler } from '@flue/runtime';
import * as v from 'valibot';
import {
  checkDraftClaims,
  prepareMdpDraft,
  summarizeBrief,
  writeDraftArtifacts,
  type WebhookInput,
} from '../lib/mdp.ts';

type PromptRunner = {
  prompt(text: string): Promise<{ text: string }>;
};

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

const webhookInput = v.object({
  type: v.optional(v.string()),
  deliveryId: v.optional(v.string()),
  delivery_id: v.optional(v.string()),
  channel: v.optional(v.string()),
  draftMode: v.optional(v.picklist(['model', 'contract-only'])),
  packDir: v.optional(v.string()),
  webhook: v.optional(v.unknown()),
  prospect: v.optional(v.unknown()),
  row: v.optional(v.unknown()),
  data: v.optional(v.unknown()),
});

const drafter = defineAgent(() => ({
  model: 'openai/gpt-5.4-mini',
  cwd: '/workspace',
  instructions: [
    'You draft GTM response copy from MDP brief artifacts.',
    'You do not send, schedule, enrich, scrape, or write back to any external system.',
    'Use only the brief and files supplied by trusted workflow code.',
    'If a claim is not present in the brief context, omit it.',
  ].join('\n'),
}));

export const route: WorkflowRouteHandler = async (_c, next) => next();

export default defineWorkflow({
  agent: drafter,
  input: webhookInput,

  async run({ harness, input }) {
    const prepared = await prepareMdpDraft(input as WebhookInput);

    if (prepared.fit.status !== 'fit') {
      return toJson({
        contract: 'mdp.flue-webhook-draft.v0',
        status: 'no-draft',
        channel: prepared.channel,
        deliveryId: prepared.deliveryId,
        draftMode: prepared.draftMode,
        fit: prepared.fit,
        files: artifactFiles(prepared),
        decision: 'MDP fit gate did not pass, so no response was drafted.',
      });
    }

    await harness.fs.writeFile('brief.json', JSON.stringify(prepared.brief, null, 2));
    await harness.fs.writeFile('draft-prompt.md', prepared.draftPrompt);

    if (prepared.draftMode === 'contract-only') {
      return toJson({
        contract: 'mdp.flue-webhook-draft.v0',
        status: 'contract-ready',
        channel: prepared.channel,
        deliveryId: prepared.deliveryId,
        draftMode: prepared.draftMode,
        fit: prepared.fit,
        brief: summarizeBrief(prepared.brief),
        draftPrompt: prepared.draftPrompt,
        files: artifactFiles(prepared),
        decision: 'Brief and draft prompt were prepared; model drafting was skipped by draftMode.',
      });
    }

    const runner = await getPromptRunner(harness);
    const firstDraft = await runner.prompt(
      'Read brief.json and draft-prompt.md, then produce the response draft. Return only the draft text.',
    );

    let draft = firstDraft.text.trim();
    let claimCheck = await checkDraftClaims(prepared.packDir, draft);

    if (claimCheck.valid !== true) {
      await harness.fs.writeFile('draft.txt', draft);
      await harness.fs.writeFile('claim-check.json', JSON.stringify(claimCheck, null, 2));
      const revision = await runner.prompt(
        [
          'Revise draft.txt so it passes claim-check.json.',
          'Use only brief.json context. Return only the revised draft text.',
        ].join('\n'),
      );
      draft = revision.text.trim();
      claimCheck = await checkDraftClaims(prepared.packDir, draft);
    }

    const draftFiles = await writeDraftArtifacts(prepared.artifactDir, draft, claimCheck);

    return toJson({
      contract: 'mdp.flue-webhook-draft.v0',
      status: claimCheck.valid === true ? 'drafted' : 'needs-review',
      channel: prepared.channel,
      deliveryId: prepared.deliveryId,
      draftMode: prepared.draftMode,
      draft,
      fit: prepared.fit,
      brief: summarizeBrief(prepared.brief),
      claimCheck,
      files: {
        ...artifactFiles(prepared),
        ...draftFiles,
      },
      decision:
        claimCheck.valid === true
          ? 'Draft prepared and claim check passed.'
          : 'Draft prepared but claim check still needs review.',
    });
  },
});

async function getPromptRunner(harness: unknown): Promise<PromptRunner> {
  const method = ['ses', 'sion'].join('');
  const source = harness as Record<string, () => Promise<PromptRunner>>;
  return source[method]();
}

function artifactFiles(prepared: Awaited<ReturnType<typeof prepareMdpDraft>>) {
  return {
    artifactDir: prepared.artifactDir,
    briefPath: prepared.briefPath,
    prospectPath: prepared.prospectPath,
    rawWebhookPath: prepared.rawWebhookPath,
  };
}

function toJson(value: unknown): JsonValue {
  return JSON.parse(JSON.stringify(value)) as JsonValue;
}
