import { definePlugin } from 'pluxx'

export default definePlugin({
  name: 'message-decision-packs',
  version: '0.1.37',
  description: 'Author, validate, and use Message Decision Packs with the local mdp CLI.',
  homepage: 'https://orchidautomation.com',
  author: {
    name: 'Orchid Labs',
    url: 'https://orchidautomation.com',
  },
  brand: {
    "displayName": "Message Decision Packs",
    "shortDescription": "Create and route message decision context.",
    "longDescription": "Message Decision Packs helps supported agents create, validate, and use local decision packs for GTM messaging and proposal review workflows. MDP stores personas or roles, fit or review rules, signals, approved claims, proof, avoid-rules, output-rules, source evidence, eval fixtures, and explicit gaps, then routes the right cards into agent-readable context with the local mdp CLI. MDP stops at pack validation, fit/review checks, claim and output checks, gaps, and briefs; sending, CRM updates, enrichment, scraping, sequencing, proposal submission, and approval workflow ownership stay outside MDP.",
    "icon": "./assets/brand/icon.png",
    "screenshots": [
      "./assets/brand/screenshot.png"
    ],
    "category": "Productivity",
    "defaultPrompts": [
      "Turn these GTM or proposal source notes into a local Message Decision Pack, then validate it.",
      "Review this .mdp pack for gaps, unsupported claims, routing issues, and weak eval coverage.",
      "Given this prospect or proposal context, run the right MDP route, fit, or review gate before drafting."
    ]
  },

  skills: './skills/',
  scripts: './scripts/',
  assets: './assets/',

  hooks: {
    sessionStart: [
      {
        command: 'bash "${PLUGIN_ROOT}/scripts/mdp-activate.sh"',
        timeout: 10000,
      },
    ],
    beforeSubmitPrompt: [
      {
        command: 'bash "${PLUGIN_ROOT}/scripts/mdp-activate.sh"',
        timeout: 10000,
      },
    ],
    postToolUse: [
      {
        command: 'bash "${PLUGIN_ROOT}/scripts/mdp-post-edit-validate.sh"',
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
