import { definePlugin } from 'pluxx'

export default definePlugin({
  name: 'message-decision-packs',
  version: '0.1.101',
  description: 'Author, validate, and use Message Decision Packs with the local mdp CLI.',
  homepage: 'https://orchidautomation.com',
  author: {
    name: 'Orchid Labs',
    url: 'https://orchidautomation.com',
  },
  brand: {
    "displayName": "Message Decision Packs",
    "shortDescription": "Versioned decision context for agents.",
    "longDescription": "Message Decision Packs gives supported agents versioned decision context for GTM messaging and proposal review workflows. MDP stores personas or roles, fit or review rules, signals, approved claims, proof, avoid-rules, output-rules, source evidence, eval fixtures, and explicit gaps, then deterministically routes bounded job-specific context and emits inspectable, hash-bound traces with the local mdp CLI. MCP-capable hosts use one profile-neutral local stdio path: mdp_run_tools, mdp_prepare_run, mdp_run, then mdp_verify_run; MCP transports CLI-owned artifacts and adds no authority. MDP is not an agent runtime, graph database, memory layer, or orchestration framework. Model calls, sending, CRM updates, enrichment, scraping, sequencing, proposal submission, and approval workflow ownership stay outside MDP.",
    "icon": "./assets/brand/icon.png",
    "screenshots": [
      "./assets/brand/screenshot.png"
    ],
    "category": "Productivity",
    "defaultPrompts": [
      "Turn these GTM or proposal source notes into a local Message Decision Pack, then validate it.",
      "Review this .mdp pack for gaps, unsupported claims, routing issues, and weak eval coverage.",
      "Run the right MDP route, fit, or review gate before drafting; for MCP, use the canonical four-stage prepare/run/verify path."
    ]
  },

  skills: './plugin/skills/',
  scripts: './scripts/',
  assets: './assets/',
  passthrough: ['./plugin/skill-evals/'],

  // MDP is a migrated/manual plugin with repo-owned skill evals; keep Pluxx's
  // generic semantic rubric advisory without blocking release-package checks.
  eval: {
    warningThreshold: 60,
    failureThreshold: 0,
  },

  hooks: {
    sessionStart: [
      {
        command: 'bash "${PLUGIN_ROOT}/scripts/mdp-activate.sh" --mode=full --plugin-root="${PLUGIN_ROOT}"',
        timeout: 10000,
      },
    ],
    beforeSubmitPrompt: [
      {
        command: 'bash "${PLUGIN_ROOT}/scripts/mdp-activate.sh" --mode=compact --plugin-root="${PLUGIN_ROOT}"',
        timeout: 10000,
      },
    ],
    postToolUse: [
      {
        command: 'bash "${PLUGIN_ROOT}/scripts/mdp-post-edit-validate.sh"',
        matcher: 'Edit|Write|apply_patch',
        timeout: 120000,
      },
    ],
  },

  platforms: {
    "codex": {
      "interface": {
        "developerName": "Orchid Labs",
        "websiteURL": "https://orchidautomation.com",
        "capabilities": [
          "Interactive",
          "Write"
        ]
      }
    }
  },

  // Migrated from codex plugin
  targets: ['claude-code', 'cursor', 'codex', 'opencode'],
})
