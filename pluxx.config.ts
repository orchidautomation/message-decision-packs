import { definePlugin } from 'pluxx'

export default definePlugin({
  name: 'message-decision-packs',
  version: '0.1.20',
  description: 'Author, validate, and use Message Decision Packs with the local mdp CLI.',
  homepage: 'https://orchidautomation.com',
  author: {
    name: 'Orchid Labs',
    url: 'https://orchidautomation.com',
  },
  brand: {
    "displayName": "Message Decision Packs",
    "shortDescription": "Create and route GTM message context.",
    "longDescription": "Message Decision Packs helps supported agents create, validate, and use local GTM messaging decision packs. MDP stores ICP, fit rules, signals, approved claims, message angles, CTA policy, avoid-rules, output-rules, source evidence, eval fixtures, and explicit gaps, then routes the right cards into agent-readable briefs with the local mdp CLI. MDP stops at pack validation, fit checks, claim and output checks, and briefs; sending, CRM updates, enrichment, scraping, and sequencing stay outside MDP.",
    "icon": "./assets/brand/icon.png",
    "screenshots": [
      "./assets/brand/screenshot.png"
    ],
    "category": "Productivity",
    "defaultPrompts": [
      "Turn these ICP, positioning, and source notes into a local Message Decision Pack, then validate it.",
      "Review this .mdp pack for gaps, unsupported claims, routing issues, and weak eval coverage.",
      "Given this prospect context, run the MDP fit gate and create a message brief only if the context is sufficient."
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
