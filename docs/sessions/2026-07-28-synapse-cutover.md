---
date: 2026-07-28
repo: git@github.com:dinglebear-ai/synapse.git
branch: ci/openwiki-ascii-normalize
head: f97abe0dd693eb149f8a53088b1020d6edfdd587
working directory: /home/jmagar/workspace/synapse
worktree: /home/jmagar/workspace/synapse
pr: "#79 ci(openwiki): normalize generated docs to ASCII before opening the PR"
beads: rmcp-template-nghi
---

# Prepare Synapse cutover

## User Request

Push the current changes, merge them into `main`, synchronize and clean the
merged branch/worktree, then finish the `synapse2` to `synapse` cutover.

## Session Overview

The initial live audit found PR #79 mergeable and fully green for its commit.
The current worktree also contained nine user-authorized deletions: completed
review reports and generated OpenWiki output. The requested quick-push flow
will preserve those deletions in a separate cleanup commit before the PR merge.

## Key Findings

- PR #79 changes only `.github/workflows/openwiki-update.yml`, adding an ASCII
  normalization and validation step after OpenWiki generation.
- `origin/main` is the integration base. The branch is one commit ahead and
  has no merge conflict with it.
- The repository uses release-please. Release identity is synchronized through
  its managed files, so no manual version bump is appropriate for this cleanup.
- Main has separate failed CodeQL (pnpm install) and Docker Publish (Trivy)
  runs on release commit `6f95560`; PR #79's CI and MSRV runs are successful.

## Files Changed

| Status | Path | Purpose |
|---|---|---|
| deleted | `.full-review/00-scope.md` through `05-final-report.md` | Remove completed review artifacts. |
| deleted | `openwiki/.last-update.json`, `openwiki/index.md`, `openwiki/quickstart.md` | Remove generated OpenWiki output pending the workflow normalization fix. |
| created | `docs/sessions/2026-07-28-synapse-cutover.md` | Record the closeout and cutover work. |

## Beads Activity

- `rmcp-template-nghi` — created and claimed for this merge, cleanup, and
  naming-cutover task.

## Verification Evidence

| Command | Result |
|---|---|
| `repo_context.sh --json --include-gh` | PR #79 is mergeable; CI/MSRV checks pass. |
| `check_mergeability.sh origin/main ci/openwiki-ascii-normalize` | Mergeable in an isolated temporary worktree. |

## Next Steps

1. Commit and push the authorized cleanup set with this session record.
2. Merge PR #79, synchronize the local checkout to `main`, and remove the
   merged branch/worktree.
3. Replace remaining current product references to `synapse2` with `synapse`,
   while retaining intentional compatibility or historical references.
