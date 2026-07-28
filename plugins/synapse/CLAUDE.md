# plugins/synapse — Claude Code instructions

## What this directory is

Multi-platform plugin package for the Synapse MCP server. Contains manifests for Claude Code, Codex, and Gemini CLI — all pointing at the same MCP connection config and skills.

## File map

| File | Role |
|---|---|
| `.claude-plugin/plugin.json` | Claude Code manifest — identity, skills, monitors, `userConfig` |
| `.codex-plugin/plugin.json` | Codex manifest — same data + Codex UI fields (`interface`) |
| `gemini-extension.json` | Gemini CLI manifest — uses `settings` array instead of `userConfig` |
| `mcp.json` | Shared MCP server connection config used by all three platforms |
| `bin/synapse` | Optional local release binary — populate with `just install` |
| `monitors/monitors.json` | Background health monitor config (requires Claude Code v2.1.105+) |
| `skills/synapse/SKILL.md` | Three-tier tool documentation shared by Claude, Codex, and Gemini |

## Versioning rule

**Do not add a `version` field to any manifest.** The marketplace derives version from the git commit SHA. An explicit `version` field causes every push to register as a new version and creates duplicate marketplace entries.

## Updating a manifest

When changing connection config (URL, auth headers), update `.mcp.json` — do not duplicate the values into each manifest separately. All three platforms read `.mcp.json`.

When changing user-configurable settings, update all three manifests: `userConfig` in the Claude and Codex `plugin.json` files, and `settings` in `gemini-extension.json`. Keep field names and descriptions consistent across all three.

## No hooks

This plugin ships **no lifecycle hooks**: there is no `hooks/` directory and no
`hooks` key in any manifest. `scripts/validate-plugin-layout.sh` asserts this.
Do not reintroduce either.

What the removed `SessionStart`/`ConfigChange` hook used to do — translate
`CLAUDE_PLUGIN_OPTION_*` into `SYNAPSE_*`, create the appdata dir, write/repair
`.env`, validate auth and port, and refresh `~/.local/bin/synapse` — is now a
manual operator step. The binary still implements all of it:

```bash
synapse setup install                  # refresh ~/.local/bin/synapse
synapse setup plugin-hook              # check, repair if blocking failures
synapse setup plugin-hook --no-repair  # audit only
```

Export the `SYNAPSE_*` vars yourself (or put them in `~/.synapse/.env`) before
running these. See `plugins/README.md` for the full option→env-var mapping.
Client mode — connecting to a server that is already running elsewhere — needs
none of this; `mcp.json` reads the plugin settings directly.

## Monitors (Claude Code v2.1.105+)

`monitors/monitors.json` invokes `synapse watch` from PATH directly. Plugin
monitors must not assume a bundled binary in the plugin directory, and must not
reference a wrapper script under `hooks/`.

The monitor command uses `${user_config.server_url}` substitution — this is resolved at runtime from the user's plugin settings. Do not hardcode URLs in `monitors.json`.

When adding a new monitor: add an entry to `monitors.json` invoking the `synapse`
binary from PATH. Note that the old `watch.sh` wrapper exited 0 when the binary
was missing; a direct invocation surfaces that as a monitor error instead.

## Updating the skill

`skills/synapse/SKILL.md` is shared by Claude Code and Codex. Gemini reads it via the `skills` path in `gemini-extension.json`. Edit it once — all platforms see the change.

The three-tier structure must be preserved:
- **Tier 1** (above fold): tool name, quick action table, critical gotchas
- **Tier 2** (middle): full action reference with parameters and response shapes
- **Tier 3** (bottom): workflows, HTTP fallback, error handling

## Adding a userConfig field

`userConfig` fields are consumed at runtime by `mcp.json` substitution and by
the operator when they export the matching `SYNAPSE_*` variable. When you add or
rename a field, update all three manifests **and** the option→env-var table in
`plugins/README.md`, which is now the only record of that mapping.

Sensitive fields declared `"sensitive": true` in `plugin.json` are **never**
substituted into skill content.

## Template adaptation

When renaming `synapse` → your service:

1. Replace all `synapse` / `Synapse` / `SYNAPSE_` identifiers in every file in this directory.
2. Rename `skills/synapse/` to `skills/<your-service>/`.
3. Update the monitor command in `monitors/monitors.json` to your binary name.
4. Keep the no-version rule: do not add `"version"` to any manifest.
5. Keep the no-hooks rule: do not add a `hooks/` directory or a `hooks` key.
