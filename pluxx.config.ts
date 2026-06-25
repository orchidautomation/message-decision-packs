import { definePlugin } from 'pluxx'

export default definePlugin({
  name: 'message-decision-packs',
  version: '0.1.3',
  description: 'Author, validate, and use Message Decision Packs with the local mdp CLI.',
  homepage: 'https://orchidautomation.com',
  author: {
    name: 'Orchid Labs',
    url: 'https://orchidautomation.com',
  },
  brand: {
    "displayName": "Message Decision Packs",
    "shortDescription": "Create and route GTM message context.",
    "longDescription": "Message Decision Packs helps supported agents create, validate, and use local GTM messaging decision packs. MDP stores ICP, fit rules, signals, approved claims, message angles, CTA policy, avoid-rules, source evidence, eval fixtures, and explicit gaps, then routes the right cards into agent-readable briefs with the local mdp CLI. MDP stops at pack validation, fit checks, claim checks, and briefs; sending, CRM updates, enrichment, scraping, and sequencing stay outside MDP.",
    "icon": "./assets/brand/icon.png",
    "screenshots": [
      "./assets/brand/screenshot.png"
    ],
    "category": "Productivity",
    "defaultPrompts": [
      "Use $mdp-lfg to orchestrate this Message Decision Pack workflow.",
      "Use Message Decision Packs to create an MDP for this ICP.",
      "Review this .mdp pack and produce a LinkedIn brief."
    ]
  },

  skills: './skills/',
  scripts: './scripts/',
  assets: './assets/',

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
