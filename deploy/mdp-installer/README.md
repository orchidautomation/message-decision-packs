# MDP Installer Vanity Routes

This directory is the source for the tiny Vercel project behind `mdp.orchidlabs.dev`.

It intentionally serves redirects only. Release artifacts continue to live on GitHub
Releases; Vercel provides stable, short URLs for install and agent-readable context.

Project:

- Vercel team: `brandon-orchidautoma1s-projects`
- Vercel project: `mdp-installer`
- Production alias: `https://mdp.orchidlabs.dev`

Deploy from this directory:

```bash
vercel link --yes --project mdp-installer --scope brandon-orchidautoma1s-projects
vercel deploy --prod --yes --scope brandon-orchidautoma1s-projects
```

After deploy, verify:

```bash
curl -fsSL https://mdp.orchidlabs.dev/install.sh | head
curl -fsSL https://mdp.orchidlabs.dev/llms.txt | head
curl -fsSL https://mdp.orchidlabs.dev/llms-full.txt | head
curl -I https://mdp.orchidlabs.dev/eve
curl -I https://mdp.orchidlabs.dev/eve/docs
```

## Eve scout vanity routes

These routes keep the deployable Eve demo close to the MDP installer domain:

- `https://mdp.orchidlabs.dev/eve` -> deployed Eve scout MDP landing page
- `https://mdp.orchidlabs.dev/eve/scout/run` -> protected scout endpoint; use `dryRun: true` for public fixture checks
- `https://mdp.orchidlabs.dev/eve/scout/health` -> health/capability endpoint
- `https://mdp.orchidlabs.dev/eve/docs` -> canonical Vercel Eve docs
