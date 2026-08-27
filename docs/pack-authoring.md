# Failure-safe pack authoring

Use a complete candidate tree and a sealed change set for any authoring pass
that changes more than one `.mdp/` file. The live pack is never the scratch
workspace.

1. Copy the live pack to a disjoint candidate directory.
2. Make and review the intended changes only in the candidate.
3. Refresh candidate projections such as `.mdp/README.md`.
4. Preview and validate the complete candidate.
5. Apply only the exact reviewed change set.

```bash
mdp --json readme refresh --dir /tmp/my-pack-candidate
mdp --json author preview \
  --dir ./my-pack \
  --candidate /tmp/my-pack-candidate \
  --out /tmp/my-pack-change-set.json

mdp --json author apply \
  --dir ./my-pack \
  --candidate /tmp/my-pack-candidate \
  --change-set /tmp/my-pack-change-set.json
```

Preview runs the same pack validation used by `mdp validate`, captures the
expected live hashes and candidate hashes, and reports bounded path-only lists:
`created`, `changed`, `unchanged`, and `deleted`. It does not modify the live
pack. A successful preview creates the required `--out` change-set file; a
refused preview creates no plan. The change-set contains no pack bodies.

Apply revalidates the candidate and refuses when either side differs from the
preview. `refused` lists the conflicting logical paths. Publication changes
only portable `.mdp/` authority files; `.mdp/briefs`, `.mdp/traces`, and files
outside `.mdp/` are preserved. A handled failure removes newly installed files,
restores backed-up files, and returns `rolled-back` paths. An indeterminate
recovery is a hard error and retains its recovery backup for operator action.
An interrupted apply leaves identity-bound recovery state; the next apply
reconciles that state before accepting another transaction.

Because another process can keep a writable descriptor to a file after it is
moved, apply never unlinks committed backups or rollback-quarantined installs.
It archives those bounded, identity- and hash-sealed files beside the pack as
`.mdp.author.evidence.*` with a matching evidence-state record and reports the
content-free `recovery-evidence-retained` reason code. This evidence is not
live pack authority and is not automatically deleted.

Neither command requires Git, creates a commit, or promotes generated content
to reviewed authority.
