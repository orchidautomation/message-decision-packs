import { definePlugin } from 'pluxx'

export default definePlugin({
  name: 'message-decision-packs',
  version: '0.1.0',
  description: 'Author, validate, and use Message Decision Packs with the local mdp CLI.',
  author: {
    name: 'Brandon Guerrero',
  },
  brand: {
    "displayName": "Message Decision Packs",
    "shortDescription": "Create and route GTM message context.",
    "longDescription": "Message Decision Packs helps Codex orchestrate end-to-end MDP workflows, create modular GTM messaging packs, sharpen ICP and fit rules, capture signals, maintain approved claims, brainstorm message angles, codify channel and CTA rules, enforce avoid-rules, surface gaps, run eval fixtures, extract source evidence, validate with the mdp CLI, and route Clay, Deepline, CSV, or enrichment-style prospect rows into agent-readable copy briefs without turning the pack into a sender, CRM, sequencer, or enrichment tool.",
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
        "developerName": "Brandon Guerrero",
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
