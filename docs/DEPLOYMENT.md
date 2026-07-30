---
title: "Deployment"
created: 2026-05-22
updated: 2026-07-30
---

---
title: "Deployment"
doc_type: "guide"
status: "active"
owner: "synapse"
audience:
  - "contributors"
  - "agents"
scope: "synapse"
source_of_truth: true
last_reviewed: "2026-07-27"
---

# Deployment

Synapse supports three deployment modes:

1. **Local development** with `just dev`.
2. **Docker Compose** with `just docker-up`.
3. **User systemd** with an installed release binary.

## Binary command surface

Every server binary exposes exactly two server modes and a CLI:

| Command | Mode | Description |
|---|---|---|
| `synapse mcp` | stdio MCP | For Claude Code `~/.claude/settings.json` stdio servers |
| `synapse serve` | Streamable HTTP MCP | For Docker/remote deployment |
| `synapse [subcommand]` | CLI | Direct `flux`/`scout` operations; all subcommands support `--json` |
| `synapse doctor` | Pre-flight check | Validates environment and config |
| `synapse --help` | Help | Print usage |
| `synapse --version` | Version | Print version |

## Deployment checklist

1. Build and test locally:
   ```bash
   just verify
   scripts/pre-release-check.sh
   ```
2. Create a `.env` from `.env.example` and set real credentials.
3. Generate a bearer token:
   ```bash
   just gen-token
   ```
4. Choose Docker or systemd.
5. Verify runtime freshness:
   ```bash
   just runtime-current
   ```
6. Smoke-test auth:
   ```bash
   SYNAPSE_MCP_TOKEN=<token> just auth-smoke
   ```
7. Run MCP integration tests:
   ```bash
   just test-mcporter
   ```

Before replacing a bare-metal binary, retain the currently installed version.
Both installer scripts now stage the new executable on the destination
filesystem, atomically rename it into place, and preserve the old executable as
`~/.local/bin/synapse.previous`. Roll back and restart with:

```bash
mv -f ~/.local/bin/synapse.previous ~/.local/bin/synapse
systemctl --user restart synapse.service
synapse --version
```

For Compose, pin `SYNAPSE_VERSION` to a release or `sha-<full-commit>` tag and
record the previous tag before `pull`/`up`; `docs/DOCKER.md` contains the exact
rollback commands.

## Binary environment awareness

The binary normalizes paths, bind hosts, and ports based on its deployment context:

```rust
fn is_containerized() -> bool {
    std::path::Path::new("/.dockerenv").exists()
        || std::env::var("RUNNING_IN_CONTAINER").is_ok()
        || std::env::var("container").is_ok()
}

fn resolve_data_dir(config_path: Option<&str>) -> PathBuf {
    if let Some(p) = config_path { return PathBuf::from(p); }
    if is_containerized() { return PathBuf::from("/data"); }
    dirs::home_dir().unwrap_or_default().join(".synapse")
}

// Bind behavior is explicit in every environment. The binary default remains
// 127.0.0.1; Compose sets SYNAPSE_MCP_HOST=0.0.0.0 inside the container when a
// published container port is required.
```

## Appdata convention

All deployments share `~/.<service>` as the logical data root:

| Deployment | Data directory |
|---|---|
| Local binary | `~/.synapse/` |
| Docker | `/data/` in container, mounted from `~/.synapse/` on host |
| Plugin | `$CLAUDE_PLUGIN_DATA` (symlinked to `~/.synapse/`) |

### Upgrade from Synapse2

This release completes a breaking product-identity cutover. Before deploying it,
stop the service and make a recoverable copy of the old appdata directory. Then
move the directory, update the Compose override, and recreate the service:

```bash
cp -a ~/.synapse2 ~/.synapse2.pre-synapse-cutover
mv ~/.synapse2 ~/.synapse
sed -i 's|\.synapse2|.synapse|g' ~/.synapse/docker-compose.env.yml
docker compose up -d --force-recreate synapse
```

Update MCP clients to use the server name `synapse` and REST clients to use
`POST /v1/synapse`. The old server name and endpoint are removed in this
release. Roll back by restoring the backup directory, its prior Compose
override, and the previous image tag.

## Managed-host prerequisites

Synapse executes only explicit local targets on the server itself. SSH targets
use their declared SSH transport, including loopback endpoints with custom ports.
Managed SSH hosts need:

- OpenSSH server access with strict known-host verification from the Synapse user.
- Python 3 for descriptor-confined Scout reads, exec, Compose name parsing, and
  bounded `beam` transfer.
- Docker CLI with the Compose plugin for remote Flux Compose operations.
- `find` for Compose filesystem discovery.
- The native tools requested by host/log/ZFS actions, such as `systemctl`,
  `journalctl`, `zpool`, and `zfs`, when those actions are used.

Per-host `dockerSocketPath`, `scoutReadRoots`, `composeSearchPaths`, and
`execAllowlist` should reflect the actual host. Custom allowlisted commands have
a zero-argument policy unless promoted into the built-in typed policy table.

## Auth expectations

Non-loopback HTTP deployments must use bearer auth or OAuth. OAuth public URLs must use HTTPS except for loopback development and may not contain userinfo. The server refuses to bind to a non-loopback address without authentication unless explicitly configured:

- Loopback bind or `SYNAPSE_MCP_NO_AUTH=true` → `LoopbackDev` (no auth)
- Non-loopback + bearer token → mounted bearer auth
- Non-loopback + `auth_mode=oauth` → mounted OAuth auth
- Non-loopback + `SYNAPSE_NOAUTH=true` → `TrustedGatewayUnscoped`; the upstream
  gateway must enforce both authentication and authorization, and network policy
  must prevent clients from reaching Synapse directly
- Non-loopback + no credentials + no gateway acknowledgment → startup error

## Claude Code stdio config

```json
{
  "mcpServers": {
    "synapse": {
      "type": "stdio",
      "command": "synapse",
      "args": ["mcp"]
    }
  }
}
```

The binary must be in `$PATH`. The plugin ships no hooks and will not install it
for you — run `synapse setup install` to copy it into `~/.local/bin/`, and re-run
that after upgrading.

## Public endpoints

- `/health` is public and fast.
- `/ready` is public and checks topology loading with a bounded timeout.
- `/status` is public but redacted.
- `/mcp` is the Streamable HTTP MCP endpoint.
- `/v1/synapse` is the REST action endpoint.

## Port assignments

Each service in the rmcp family uses a fixed port to avoid collisions:

| Service | MCP Port | Binary name |
|---|---|---|
| lab | 8765 | `labby` |
| axon_rust | 8001 | `axon` |
| syslog-mcp | 3100 | `syslog` |
| unraid-mcp (unrust) | 6970 | `unraid` |
| gotify-mcp (rustify) | 9158 | `gotify` |
| unifi-mcp (rustifi) | 7474 | `unifi` |
| tailscale-mcp (rustscale) | 7575 | `tailscale` |
| apprise-mcp | 8765 | `apprise` |
| synapse | 40080 | `synapse` |

Set the port via `SYNAPSE_MCP_PORT` or in `config.toml`. Update `EXPOSE` in the Dockerfile and the port mapping in `docker-compose.yml` to match.

## Worktree file propagation

Claude Code worktrees are fresh checkouts — gitignored files like `.env` and `config.toml` are absent by default. The `.worktreeinclude` file at the repo root tells Claude Code which gitignored files to copy into each new worktree automatically:

```
# .worktreeinclude
.env
config.toml
```

This ensures the server can start in a worktree without manual setup. Both files are one-way copied (main → worktree) at worktree creation time only.

`.gitignore` additions required alongside `.worktreeinclude`:

```gitignore
config.toml
.beagle/
```

See `docs/DOCKER.md`, `docs/SYSTEMD.md`, `docs/ENV.md`, and `docs/CONFIG.md` for deployment-specific details. See `docs/PATTERNS.md` §19, §27, §28, §46, §47, §A6 for port assignments, security, environment awareness, binary installation, and worktree patterns.
