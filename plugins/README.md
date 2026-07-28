# plugins

Claude Code and Codex plugin packages for the MCP server. Both platforms share the same skills and MCP connection config — only the manifests differ.

## Structure

```
plugins/synapse2/
├── .claude-plugin/
│   └── plugin.json       # Claude Code manifest
├── .codex-plugin/
│   ├── plugin.json       # Codex manifest
│   └── README.md         # Codex manifest field reference
├── gemini-extension.json # Gemini CLI manifest
├── mcp.json              # Shared MCP server connection config
├── bin/synapse           # Optional local release binary (`just install`)
├── monitors/
│   └── monitors.json     # Background health monitor config
└── skills/
    └── synapse2/
        └── SKILL.md      # Tool documentation for Claude, Codex, and Gemini
```

This plugin ships **no lifecycle hooks**. It is a pure connection package:
manifests, MCP config, one monitor, and skills. See [Setup](#setup) below for
the manual equivalent of what the removed `SessionStart` hook used to do.

---

## Manifests

### `.claude-plugin/plugin.json`

Claude Code plugin manifest. Defines the plugin identity, MCP server connection, skills, monitors, and user-configurable options. It declares no `hooks` key — see [Setup](#setup).

**User config fields** (set via Claude Code plugin settings):

| Field | Type | Description |
|---|---|---|
| `server_url` | string | MCP HTTP server base URL (default: `http://localhost:40080`) |
| `api_token` | string (sensitive) | Bearer token for auth |
| `no_auth` | boolean | Disable auth (loopback dev only; non-loopback requires an upstream gateway) |
| `auth_mode` | string | `bearer` or `oauth` |
| `public_url` | string | Public URL for OAuth callbacks |
| `google_client_id` | string (sensitive) | Google OAuth client ID |
| `google_client_secret` | string (sensitive) | Google OAuth client secret |
| `auth_admin_email` | string | OAuth admin email |

### `.codex-plugin/plugin.json`

Codex equivalent of the Claude Code manifest. Shares `.mcp.json` and `skills/` with the Claude plugin. Adds Codex-specific UI fields under `interface`:

- `displayName`, `shortDescription`, `longDescription` — registry presentation
- `defaultPrompt` — three sample prompts shown in the Codex UI
- `brandColor` — hex color for the plugin icon (e.g., `#6366F1`)
- `composerIcon`, `logo` — asset paths (512×512 PNG, SVG)

See `.codex-plugin/README.md` for a full field reference and `brandColor` guide.

### `.mcp.json`

Shared MCP server connection config used by both plugins. Points both clients at the same HTTP endpoint with the same auth headers.

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

---

## Setup

This plugin ships no hooks, so nothing runs automatically at session start.
**Client mode** — connecting to an already-running Synapse server — needs no
setup at all: `mcp.json` reads `server_url` and `api_token` straight from your
plugin settings.

**Server mode** — where this machine also runs the server — needs a one-time
manual bootstrap. All of the policy still lives in the Rust binary; only the
automatic invocation is gone:

```bash
# 1. Put the binary on PATH (also refresh it after a plugin update).
synapse setup install

# 2. Export the settings the hook used to translate for you, or put the
#    same values in ~/.synapse2/.env.
export SYNAPSE_MCP_TOKEN=...          # was plugin option: api_token
export SYNAPSE_SERVER_URL=...         # was plugin option: server_url
export SYNAPSE_HOSTS_CONFIG=...       # was plugin option: synapse_hosts_config
export SYNAPSE_CONFIG_FILE=...        # was plugin option: synapse_config_file
export SYNAPSE_MCP_AUTH_MODE=...      # was plugin option: auth_mode
export SYNAPSE_MCP_NO_AUTH=...        # was plugin option: no_auth
# OAuth mode only:
export SYNAPSE_MCP_PUBLIC_URL=...            SYNAPSE_MCP_GOOGLE_CLIENT_ID=...
export SYNAPSE_MCP_GOOGLE_CLIENT_SECRET=...  SYNAPSE_MCP_AUTH_ADMIN_EMAIL=...

# 3. Create/repair appdata + .env and validate auth and port.
synapse setup plugin-hook              # check, then repair if needed
synapse setup plugin-hook --no-repair  # audit only; report without mutating
```

`setup check`, `setup repair`, `setup install`, and `setup plugin-hook` are all
still shipped by the binary and emit the same JSON report the hook used to
print. Re-run step 1 after `/plugin update`; the hook used to do that on every
session start.

---

## Skills

### `skills/synapse2/SKILL.md`

Three-tier structured documentation for the Synapse2 `flux` and `scout` MCP tools, used by Claude Code and Codex to understand when and how to invoke them.

**Tier 1** (above the fold): tool name, quick action table, most common usage.  
**Tier 2**: full action reference — parameters, types, example calls, response shapes.  
**Tier 3**: multi-step workflows demonstrating real-world use.

Tier 3 also includes a REST fallback for when the MCP transport is unavailable: `POST /v1/synapse2` using the `SYNAPSE_MCP_HOST`, `SYNAPSE_MCP_PORT`, and `SYNAPSE_MCP_TOKEN` env vars.


---

## Versioning

Plugin manifests intentionally do not contain a `version` field. Marketplace
versions are derived from git commits; release version synchronization is owned
by `Cargo.toml`, the npm launcher package, and the release manifest.

---

## Maintenance checklist

1. Keep Claude, Codex, and Gemini manifests pointed at the same Synapse2 server.
2. Keep `skills/synapse2/SKILL.md` aligned with the canonical operation registry
   in `src/actions/operations.rs` (59 operations).
3. Preserve the no-`version` manifest contract.
4. Preserve the no-hooks contract: do not reintroduce a `hooks/` directory or a
   `hooks` key in any manifest.
5. Run `scripts/validate-plugin-layout.sh` after plugin changes.
