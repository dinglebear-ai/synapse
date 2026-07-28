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
preserved those deletions in a separate cleanup commit before PR #79 merged.
The remaining product identity has now been cut over from `synapse2` to
`synapse` across the crate, MCP contract, REST endpoint, plugins, docs, web
client, deployment configuration, and the live appdata mount.

## Key Findings

- PR #79 changes only `.github/workflows/openwiki-update.yml`, adding an ASCII
  normalization and validation step after OpenWiki generation.
- `origin/main` is the integration base. The branch is one commit ahead and
  has no merge conflict with it.
- The repository uses release-please. Release identity is synchronized through
  its managed files, so no manual version bump is appropriate for this cleanup.
- Main has separate failed CodeQL (pnpm install) and Docker Publish (Trivy)
  runs on release commit `6f95560`; PR #79's CI and MSRV runs are successful.
- The live `synapse` container was still mounted from `~/.synapse2`; it had
  config, environment, and logs but no OAuth database or signing key.
- A recoverable copy was created at
  `~/.synapse2.pre-cutover-20260728T155000Z`; the container now mounts
  `~/.synapse` and `/status` returns `server: synapse` and `status: ok`.

## Files Changed

| Status | Path | Purpose |
|---|---|---|
| deleted | `.full-review/00-scope.md` through `05-final-report.md` | Remove completed review artifacts. |
| deleted | `openwiki/.last-update.json`, `openwiki/index.md`, `openwiki/quickstart.md` | Remove generated OpenWiki output pending the workflow normalization fix. |
| created | `docs/sessions/2026-07-28-synapse-cutover.md` | Record the closeout and cutover work. |
| modified/renamed | Source, tests, web app, deployment docs, plugin package, generated OpenAPI | Replace active `synapse2` identity with `synapse`. |

## Beads Activity

- `rmcp-template-nghi` — created and claimed for this merge, cleanup, and
  naming-cutover task.

## Verification Evidence

| Command | Result |
|---|---|
| `repo_context.sh --json --include-gh` | PR #79 is mergeable; CI/MSRV checks pass. |
| `check_mergeability.sh origin/main ci/openwiki-ascii-normalize` | Mergeable in an isolated temporary worktree. |
| `SOLDR_BYPASS=1 cargo check --locked` | Passed after the normal wrapper stalled. |
| `SOLDR_BYPASS=1 cargo test --locked -q` | Rust suite passed after the plugin manifest correction. |
| `pnpm --dir apps/web check && pnpm --dir apps/web typecheck && pnpm --dir apps/web test` | 31 web tests passed. |
| `curl http://127.0.0.1:40080/status` | Live container reports a healthy `synapse` server. |

## Next Steps

1. Merge the cutover branch after its CI succeeds, then release it through the
   repository's release-please flow.
2. Deploy the released image tag when available; the live appdata/config mount
   is already migrated and its pre-cutover snapshot is retained for rollback.
