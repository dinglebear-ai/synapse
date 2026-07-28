---
type: Documentation Quickstart
title: "OpenWiki Quickstart"
description: "Point of entry for this repository's generated wiki, with links to the OpenWiki automation workflow, source guidance, and where to continue in existing long-form docs."
tags: [openwiki, synapse, documentation]
---

# OpenWiki Quickstart

This is the entrypoint for repository-local generated documentation under `/openwiki`. It was introduced so the current docs update flow can keep generated pages aligned with recent changes in automation and guidance files.

## What this OpenWiki snapshot covers

- It documents **how OpenWiki is refreshed and tracked** for this repository.
- It links to authoritative source files that define the current generation workflow and source-of-truth usage guidance.
- It points to the project's primary hand-authored docs in `docs/` for deep technical details (architecture, runtime, API, and deployment) while the generated OpenWiki is kept intentionally focused and minimal.

## OpenWiki refresh flow

The source of this process is [`.github/workflows/openwiki-update.yml`](/.github/workflows/openwiki-update.yml):

- Runs on `workflow_dispatch` and a daily cron (`0 8 * * *`).
- Uses one `update` job on `ubuntu-latest`.
- Installs the OpenWiki CLI from npm, then runs `openwiki code --update --print`.
- Sets provider/model via OpenRouter environment for the run.
- Opens a PR using `peter-evans/create-pull-request` and includes updates to `openwiki`, `AGENTS.md`, `CLAUDE.md`, and the workflow file itself.

Why it matters:
- The workflow file is the source of automation truth, while [`.last-update metadata`](./.last-update.json) records when and from which commit this wiki was generated.
- The repository's contributor guidance in [`CLAUDE.md`](/CLAUDE.md) now points back to this page, so drift here blocks onboarding and recovery.

## Last-update provenance

The authoritative run stamp is kept in [`.last-update.json`](./.last-update.json). If you are troubleshooting stale content, compare:

- `gitHead` in that file vs current `HEAD`.
- `command` (`init` or `update`) vs the expected run mode.
- `updatedAt` timestamp vs the schedule or manual run history.

## Where to read the details next

- Start with [`CLAUDE.md`](/CLAUDE.md) for repository operator guidance and agent handoff rules.
- Read `docs/ARCHITECTURE.md`, `docs/API.md`, and `docs/DEPLOYMENT.md` for implementation-level behavior.
- Track workflow and release behavior in `/.github/workflows/` and `/.github/workflows/openwiki-update.yml` for generation or PR automation changes.
- Use `/openwiki/.last-update.json` to verify whether this page likely reflects your local working tree.

## Backlog

- `openwiki/architecture.md` - no generated architecture page exists yet; the repository already has canonical architecture text in `docs/ARCHITECTURE.md`.
- `openwiki/operations.md` - no generated operations/process page exists yet; canonical procedures are in `docs/CI.md`, `docs/DEPLOYMENT.md`, and `docs/SECURITY.md`.
- `openwiki/domain.md` - no generated service-domain page exists yet; canonical runtime/domain detail is spread across `docs/QUICKSTART.md` and `README.md`.
