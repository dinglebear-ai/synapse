# synapse2 plugin

Multi-platform plugin package that connects Claude Code, Codex, and Gemini CLI to the Synapse2 MCP server.

## Structure

```
plugins/synapse2/
├── .claude-plugin/
│   └── plugin.json         # Claude Code manifest
├── .codex-plugin/
│   ├── plugin.json         # Codex manifest
│   └── README.md           # Codex manifest field reference
├── gemini-extension.json   # Gemini CLI extension manifest
├── mcp.json                # Shared MCP server connection config (Claude/Codex)
├── bin/
│   └── synapse             # Release binary (populate with: just install)
├── monitors/
│   └── monitors.json       # Background health monitor (requires Claude Code v2.1.105+)
└── skills/
    └── synapse2/
        └── SKILL.md        # Tool documentation (shared by Claude, Codex, Gemini)
```

**No hooks.** This package ships no lifecycle hooks — no `hooks/` directory and
no `hooks` key in any manifest. See [Setup](#setup).

## Platform manifests

Claude Code and Codex read their MCP connection config from the shared `mcp.json`. Gemini CLI embeds its `mcpServers` config inline in `gemini-extension.json` (its own format). All three share the same `skills/` directory.

| File | Platform | MCP config | Variable syntax |
|---|---|---|---|
| `.claude-plugin/plugin.json` | Claude Code | `mcp.json` | `${user_config.*}` |
| `.codex-plugin/plugin.json` | Codex | `mcp.json` | `${user_config.*}` |
| `gemini-extension.json` | Gemini CLI | inline `mcpServers` | `${settings.*}` |

**No `version` field in any manifest.** The marketplace assigns version from the git commit SHA. Adding an explicit version creates duplicate entries on every push.

## MCP connection

`mcp.json` is shared by Claude Code and Codex:

```json
{
  "mcpServers": {
    "synapse": {
      "type": "http",
      "url": "${user_config.server_url}/mcp",
      "headers": { "Authorization": "Bearer ${user_config.api_token}" }
    }
  }
}
```

The `${user_config.*}` / `${settings.*}` variables are populated from each platform's user-configurable settings at runtime.

## Setup

Nothing runs automatically at session start. Connecting to a server that is
already running requires no setup at all — `mcp.json` substitutes your
`server_url` and `api_token` directly.

If this machine also runs the server, bootstrap it once by hand:

```bash
synapse setup install                  # put/refresh the binary on PATH
synapse setup plugin-hook              # check, then repair on blocking failures
synapse setup plugin-hook --no-repair  # audit only; never mutates appdata
```

Export the matching `SYNAPSE_*` variables (or write them into `~/.synapse2/.env`)
first — the removed hook used to translate the plugin options for you. See
`plugins/README.md` for the option→variable mapping. Re-run
`synapse setup install` after a plugin update.

## Monitors

**Requires Claude Code v2.1.105+.**

`monitors/monitors.json` declares a background `server-health` monitor that starts automatically at session start. It runs `synapse watch` from `PATH` and delivers each stdout line to Claude as a notification whenever the MCP server changes state.

The monitor emits only on state transitions — Claude is not notified while the server is stable. Three states:

- `UP` — `/health` returned 2xx
- `DOWN` — connection refused / timeout
- `DEGRADED(HTTP N)` — non-2xx HTTP response

The plugin does not ship or install a binary. Install `synapse` separately before
enabling the monitor.

Disabling the plugin mid-session does not stop an already-running monitor; it stops when the session ends.

## Skills

`skills/synapse2/SKILL.md` is the three-tier structured documentation for the `synapse2` MCP tool. The AI reads Tier 1 for quick lookups, Tier 2 for parameter details, Tier 3 for multi-step workflows.

## Packaging checklist

1. Confirm the plugin does not rely on a bundled `synapse` binary.
2. Confirm `synapse` is installed separately when testing runtime setup.
3. Run `cargo test --test plugin_contract` and `just validate-plugin`.
4. Verify all manifests still omit explicit `version` fields.
5. Verify no `hooks/` directory and no `hooks` manifest key have reappeared.
6. Install through the target marketplace or local plugin path.
