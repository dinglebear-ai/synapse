# Plugin Surfaces

Synapse ships one service plugin package with three host-specific entrypoints:

- Claude Code: `plugins/synapse/.claude-plugin/plugin.json`
- Codex: `plugins/synapse/.codex-plugin/plugin.json`
- Gemini: `plugins/synapse/gemini-extension.json`

All three surfaces should describe the same MCP server, expose the same skills, and connect to the same HTTP MCP endpoint. The host manifests differ, but the service behavior should not.

## Layout

```text
plugins/synapse/
  .claude-plugin/
    plugin.json          # Claude Code manifest
  .codex-plugin/
    plugin.json          # Codex manifest
    README.md            # Codex manifest field reference
  mcp.json               # Shared Claude/Codex MCP connection config
  gemini-extension.json  # Gemini CLI extension manifest
  monitors/
    monitors.json        # Claude background health monitor config
  bin/
    synapse             # Optional Git LFS-tracked plugin binary artifact
  skills/
    synapse/
      SKILL.md           # Shared action documentation
```

When changing the action surface, keep the plugin package, skill text, and manifests aligned with the Synapse binary and `flux`/`scout` tools.

## Shared Contract

Each plugin surface should agree on:

- service name and repository URL
- MCP server name
- HTTP MCP URL shape: `<server_url>/mcp`
- bearer token setting name
- upstream service credential names
- action list and skill documentation
- read/write capability claims

Keep the plugin manifests thin. Runtime setup belongs in the service binary, not in manifest-specific shell code.

## Claude Code

Claude Code uses `plugins/synapse/.claude-plugin/plugin.json`.

Responsibilities:

- identifies the plugin and repository
- declares `mcpServers`, `skills`, and `experimental.monitors` paths
- defines `userConfig` settings exposed in Claude Code
- marks sensitive values with `sensitive: true`

**This package ships no lifecycle hooks.** There is no `hooks/` directory and no
`hooks` key in any manifest; `scripts/validate-plugin-layout.sh` asserts both.

Setup is therefore an explicit operator step rather than something that runs at
session start. Client mode — pointing at a server that is already running —
needs nothing: `mcp.json` substitutes `server_url` and `api_token` directly from
plugin settings. Server mode needs a one-time bootstrap:

```bash
<binary> setup install                  # put/refresh the binary on PATH
<binary> setup plugin-hook              # check, repair only if needed
<binary> setup plugin-hook --no-repair  # rollout audit; never mutates
```

The operator is responsible for exporting the service's `SYNAPSE_*` env vars (or
writing them into the appdata `.env`) before running setup; `plugins/README.md`
holds the plugin-option→env-var mapping. Policy, repair behavior, and failure
classification live in the binary, never in manifest-specific shell code.

## Codex

Codex uses `plugins/synapse/.codex-plugin/plugin.json`.

Responsibilities:

- identifies the plugin for Codex listings
- points at shared `skills` and `mcp.json`
- describes the interface shown in Codex UI
- declares read/write capabilities
- provides example prompts
- provides branding fields such as `brandColor`, `composerIcon`, and `logo`

Codex does not use Claude lifecycle hooks. Its manifest should still point to the same MCP server and shared skills so behavior stays aligned with Claude Code.

Codex-specific fields to adapt:

| Field | Purpose |
| --- | --- |
| `interface.displayName` | human-readable plugin name |
| `interface.shortDescription` | short listing text |
| `interface.longDescription` | full listing text |
| `interface.capabilities` | `["Read"]` or `["Read", "Write"]` |
| `interface.defaultPrompt` | three realistic prompts |
| `interface.brandColor` | service-appropriate hex color |

See `plugins/synapse/.codex-plugin/README.md` for the full manifest field reference.

## Gemini

Gemini uses `plugins/synapse/gemini-extension.json`.

Responsibilities:

- identifies the extension
- declares Gemini settings
- connects to the MCP HTTP endpoint
- points at shared skills
- optionally points Gemini at a context file with `contextFileName`

The Gemini manifest uses `settings.*` interpolation instead of Claude/Codex `user_config.*` interpolation:

```json
"url": "${settings.server_url}/mcp"
```

Sensitive Gemini settings use:

```json
"secret": true
```

Keep Gemini setting names aligned with Claude/Codex where possible. For example, prefer `server_url`, `api_token`, `<service>_api_url`, and `<service>_api_key` across all three surfaces.

## Plugin Validation

Run the plugin layout validator after changing manifests, MCP config, hooks, or
skills:

```bash
just validate-plugin
# or
scripts/validate-plugin-layout.sh
```

The validator checks:

- Claude, Codex, and Gemini manifests are valid JSON
- plugin manifests do not contain a `version` field
- manifests point to the shared `mcp.json`, monitors, and skills paths
- shared MCP config exposes the `synapse` HTTP server at `${user_config.server_url}/mcp`
- Gemini config exposes the same `synapse` HTTP server at `${settings.server_url}/mcp`
- no manifest declares a `hooks` key and no `hooks/` directory is shipped
- monitors invoke the PATH binary rather than a wrapper script
- every skill has `name:` and `description:` frontmatter

Use `PLUGIN_ROOT=plugins/<service>` when validating an adapted service package.

For release checks, `just pre-release` includes this validator and the other
template gates.

## Shared MCP Config

Claude Code and Codex share `plugins/synapse/mcp.json`:

```json
{
  "mcpServers": {
    "synapse": {
      "type": "http",
      "url": "${user_config.server_url}/mcp",
      "headers": {
        "Authorization": "Bearer ${user_config.api_token}"
      }
    }
  }
}
```

Gemini carries equivalent MCP config directly in `gemini-extension.json` because its interpolation model is different.

## Skills

`plugins/synapse/skills/synapse/SKILL.md` is shared across Claude, Codex, and Gemini. Every skill follows the three-tier fallback pattern — agents try each tier in order and stop when one works:

```markdown
# synapse — Claude Code Skill

Use this skill whenever you need to query or manage Synapse.

## Tier 1: MCP tool (preferred)
Use when the Synapse MCP server is configured in your agent.

scout(action="nodes")
flux(action="docker", subaction="info")
scout(action="help")          # always available, no auth required

## Tier 2: CLI binary
Use when MCP is unavailable but the binary is installed in $PATH.

synapse scout nodes --json
synapse flux docker info --json
synapse doctor

Env required for HTTP mode: SYNAPSE_MCP_TOKEN, SYNAPSE_MCP_HOST, SYNAPSE_MCP_PORT

## Tier 3: Direct API (last resort)
Use when neither MCP nor CLI is available.

curl -H "Authorization: Bearer $SYNAPSE_MCP_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"action":"scout.nodes","params":{}}' \
     "http://${SYNAPSE_MCP_HOST:-127.0.0.1}:${SYNAPSE_MCP_PORT:-40080}/v1/synapse"

## Gotchas
- [service-specific pitfalls go here]
- [e.g. pagination, required headers, rate limits]
```

The skill should also include:

- quick action table (action → description → required params)
- full parameter reference with types
- common workflows (status check → list → inspect)
- response shapes for key actions
- sensitive-value handling notes (never log tokens, etc.)

Do not maintain separate skill docs per host. Update the shared skill when the action surface changes; Claude, Codex, and Gemini all read the same file.

## Binary-Owned Setup Standard

These commands are invoked by an operator, not by a plugin hook. Every Rust
server with a Claude plugin should still expose:

```bash
<binary> setup plugin-hook
<binary> setup plugin-hook --no-repair
<binary> setup check
<binary> setup repair
```

`setup plugin-hook` should:

- run `setup check` first
- run `setup repair` only when needed and only when `--no-repair` is absent
- emit structured JSON when the global JSON flag is used
- include `exit_policy`, `blocking_failures`, `advisory_failures`, `ran_repair`, and `no_repair`
- exit `0` for success or advisory failures
- exit nonzero for blocking failures
- enforce a bounded total runtime

Advisory failures are non-blocking local conditions such as missing `.env` files when process env already supplies values, occupied MCP ports, optional startup proofs, or model prewarm. Blocking failures are prerequisites required for the plugin to function, such as missing appdata directories, missing required upstream credentials, or invalid OAuth/auth configuration.

## Version And Release Sync

Keep version and metadata synchronized across:

| File | Fields |
| --- | --- |
| `Cargo.toml` | package `version`, homepage/repository when present |
| `plugins/synapse/.claude-plugin/plugin.json` | identity, repository, user config; no `version` field |
| `plugins/synapse/.codex-plugin/plugin.json` | identity, repository, interface metadata; no `version` field |
| `plugins/synapse/gemini-extension.json` | identity, repository, settings |
| `server.json` | package version and registry metadata, when present |

`Cargo.toml` is the canonical version source. Use
`scripts/bump-version.sh` to update Cargo and `server.json` together, then use
`scripts/check-version-sync.sh` or `just pre-release` to verify that
version-bearing files still agree. Plugin manifests should remain versionless.

Synapse has write-capable `flux` and `scout` actions guarded by confirmation. Keep Codex/Claude/Gemini capability claims synchronized with those guarded write paths.

## Adaptation Checklist

When updating the Synapse plugin:

1. Update all three manifests with the current repository, description, author, keywords, and capability claims.
3. Keep credential names aligned across Claude `userConfig`, Codex shared `mcp.json`, and Gemini `settings`.
4. Update the plugin-option→env-var mapping table in `plugins/README.md` when `userConfig` changes.
5. Keep `synapse setup plugin-hook`, `--no-repair`, `check`, and `repair` working.
7. Update shared skill docs for the actual action surface.
8. Replace Codex `defaultPrompt` entries with realistic prompts.
9. Update Gemini `description`, `settings`, and `contextFileName` if needed.
10. Run `just validate-plugin` and plugin contract tests before release.

## Required Tests

Each server should include tests that prove:

- no `hooks/` directory and no `hooks` key in any manifest
- monitors invoke the PATH binary rather than a wrapper script
- `setup plugin-hook --no-repair` parses and does not mutate appdata
- JSON plugin-hook output contains `exit_policy`, `blocking_failures`, `advisory_failures`, `ran_repair`, and `no_repair`
- advisory failures exit `0`
- blocking failures exit nonzero
- Claude, Codex, and Gemini manifests use the same service name, endpoint, token setting, and credential fields
