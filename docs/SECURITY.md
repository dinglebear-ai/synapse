---
title: "Security Model"
doc_type: "guide"
status: "active"
owner: "synapse"
audience:
  - "operators"
  - "contributors"
  - "agents"
scope: "service"
source_of_truth: false
upstream_refs:
  - "src/server.rs"
  - "src/server/routes.rs"
  - "src/synapse.rs"
  - "src/secure_path.rs"
  - "src/ssh/"
  - ".github/workflows/ci.yml"
last_reviewed: "2026-07-27"
---

# Security model

Synapse controls Docker daemons, host processes, logs, ZFS state, and files. A
Docker socket is root-equivalent access to its daemon host, so authentication,
authorization, host topology, SSH trust, and path confinement are deployment
boundaries rather than optional polish.

## HTTP authentication and scopes

- Loopback HTTP and stdio use the local process boundary.
- Non-loopback HTTP requires bearer, OAuth, or an explicitly isolated trusted
  gateway deployment. Startup fails closed when no valid policy is configured.
- Static bearer tokens receive `synapse:read` only. Write automation uses OAuth
  or an external authorization gateway.
- `synapse:write` satisfies `synapse:read`; unknown actions fail closed.
- `SYNAPSE_MCP_PUBLIC_URL` must use HTTPS except for loopback development and
  may not contain userinfo or wildcard hosts.
- Destructive confirmation bypass is rejected on non-loopback binds.

## Request and response budgets

Operational `/mcp`, REST, activity, and capability traffic is protected by a
global non-queueing concurrency cap and body-size limit. Health, readiness,
status, OAuth discovery, and static assets remain available while operational
traffic is shedding overload. Subprocess, SSH, Docker-stream, fanout, response,
and file-transfer paths have independent deadlines or byte/item ceilings.

## Host topology and SSH trust

- Host `protocol` is authoritative. Omitted protocols default to `ssh`; local
  execution requires `protocol: "local"` or the built-in `local` host.
- Loopback SSH endpoints remain SSH endpoints and retain configured ports, users,
  keys, aliases, ProxyJump behavior, and namespaces.
- OpenSSH strict known-host checking is required. Wildcard entries trigger a
  startup warning because they weaken host identity.
- Recursive SSH `Include` files and wildcard include directories participate in
  topology cache invalidation.
- ControlMaster and forwarded Docker sockets live under owner-only runtime
  directories. Forwarded socket names contain only a connection-identity hash.
- Per-host `dockerSocketPath` applies to both local and SSH-forwarded Docker.

## Files and commands

- Scout paths must be absolute, traversal-free, and beneath configured
  `scoutReadRoots` or Compose roots. Sensitive key, `.env`, and PEM paths are
  blocked.
- Local access uses Linux `openat2` with `BENEATH`, `NO_SYMLINKS`, and
  `NO_MAGICLINKS`. Remote wrappers open each component with `O_NOFOLLOW`.
- `scout beam` enforces the policy on both endpoints, never invokes ambient
  `scp`, and caps each transfer at 64 MiB.
- Built-in Scout commands use typed argv policies. Per-host `execAllowlist`
  commands are deliberately zero-argument until a typed policy is registered.
- User commands are never passed through `sh -c`.

## Container deployment

The production image contains Python 3 and the official Docker CLI plus Compose
plugin because those are runtime dependencies of descriptor wrappers and Flux
Compose. It contains no Docker daemon. Base images are pinned by manifest
digest. Appdata is mounted from `~/.synapse` to `/data`; the root entrypoint
hardens permissions and then drops privileges.

## GitHub Actions and self-hosted runners

The repository currently requires approval for every external contributor before
a pull-request workflow can run. Verify the live setting with:

```bash
gh api repos/dinglebear-ai/synapse/actions/permissions/fork-pr-contributor-approval
# expected: {"approval_policy":"all_external_contributors"}
```

Do not weaken this setting while pull-request jobs use persistent self-hosted
Unraid runners. Approval protects the runner from automatic execution; reviewers
still must inspect build scripts, package lifecycle scripts, local actions, and
workflow changes before approving a run. Protected push/release workflows should
continue using SHA-pinned third-party actions and least-privilege permissions.

## Operational checks

```bash
synapse doctor
curl -fsS http://127.0.0.1:40080/health
curl -fsS http://127.0.0.1:40080/ready
just auth-smoke
just deny
```

Treat unexpected wildcard known-host warnings, public HTTP OAuth URLs, missing
path roots, Docker socket permission changes, or disabled confirmation as
security-relevant configuration drift.
