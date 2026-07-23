# MDP Proposal Flow Remotion Video

This Remotion project renders the motion-graphics walkthrough for `examples/proposal-flow-video`.

## Commands

```bash
npm ci
npm run dev      # Remotion Studio, no browser auto-open
npm run still    # render a preview PNG to out/proposal-flow-video.png
npm run render   # render MP4 to out/proposal-flow-video.mp4
npm run lint
```

The root-level helper also works:

```bash
bash ../scripts/render-video.sh
```

## Composition

- Composition ID: `ProposalFlow`
- Size: 1920×1080
- FPS: 30
- Duration: 1050 frames / 35 seconds
- Output: `out/proposal-flow-video.mp4`

The output directory is gitignored. Commit the Remotion source, not rendered binaries.

## Notes

This video uses only synthetic/public-safe MDP proposal-flow content. It repeats the same boundary as the CLI demo: the default walkthrough uses the local proposal runner in offline mock mode, so the receipt is blocked/non-audit-grade by design. Production pilots need a real native/headless runner audit and an audit-grade `mdp run-receipt` result before claiming model-context isolation.
