//! Scout command execution operations: `exec`, `emit`, `beam`.
//!
//! # Security invariants (B0 + B14)
//!
//! - **exec** and **emit** validate the command name against `validate_command`
//!   + `ALLOWED_READ_COMMANDS` BEFORE any IO. Non-allowlisted names, names in
//!     `EXEC_DENYLIST`, and invalid names all produce a hard error.
//!
//! - **exec** and **emit** are gated by the `Confirmer` trait (B5) even though
//!   the allowlist limits them to read-only commands. synapse-mcp classifies
//!   all exec variants as destructive; we follow the same convention.
//!
//! - Commands are passed via `SshExecutor::exec` (execvp-style: no `sh -c`,
//!   no shell expansion). HARD INVARIANT — never use shell wrapping.
//!
//! - Local exec runs via `std::process::Command` for local hosts. The `path`
//!   parameter (optional working directory) is applied via `current_dir` only
//!   for local exec and local emit targets. Remote exec cannot change directory
//!   without a shell, so remote emit targets reject `path` instead of silently
//!   ignoring it.
//!
//! - **beam** validates BOTH endpoints against each host's configured read roots
//!   and sensitive-path policy. Transfer is delegated to `SshExecutor`, whose
//!   production implementation uses descriptor-confined local/remote file IO
//!   with a bounded payload and no ambient `scp` process.
//!
//! - `git` is deliberately NOT in `ALLOWED_READ_COMMANDS` (removed by B0 security
//!   review: arbitrary config injection via `git -c core.editor=...`). Requests
//!   for `git` are rejected by `validate_command` as "not allowlisted."

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::elicitation_gate::Confirmer;
use crate::fanout::{FanoutOutcome, fanout};
use crate::flux_service::host::is_local_host;
use crate::ssh::SshExecutor;
use crate::synapse::{HostConfig, validate_command, validate_command_args};
use crate::synapse::{command_filesystem_operand_indices, validate_scout_read_path};

const BOUND_EXEC_SCRIPT: &str = r#"import json, os, sys
command, argv, specs, cwd = sys.argv[1], json.loads(sys.argv[2]), json.loads(sys.argv[3]), json.loads(sys.argv[4])
fds = []
def bind(root, rel):
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    for part in [p for p in root.split('/') if p] + [p for p in rel.split('/') if p]:
        nxt = os.open(part, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        os.close(fd); fd = nxt
    os.set_inheritable(fd, True); fds.append(fd); return fd
for spec in specs:
    fd = bind(spec['root'], spec['relative'])
    argv[spec['index']] = '/proc/self/fd/' + str(fd)
if cwd: os.fchdir(bind(cwd['root'], cwd['relative']))
os.execvp(command, [command] + argv)
"#;

/// Default timeout for Scout command execution.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Upper bound for caller-controlled command timeouts.
pub const MAX_TIMEOUT_SECS: u64 = 300;

// ─── exec ────────────────────────────────────────────────────────────────────

/// Run `command` on `host`, with optional `path` as the working directory
/// (local only; ignored for remote hosts — see module doc).
///
/// The `args` parameter extends the command with positional arguments
/// (execvp-style; never shell-interpolated).
///
/// Destructive gate: `confirmer.require()` is called BEFORE any IO.
pub async fn exec(
    host: &HostConfig,
    executor: &dyn SshExecutor,
    confirmer: &dyn Confirmer,
    command: &str,
    args: &[String],
    path: Option<&str>,
) -> Result<Value> {
    exec_with_timeout(host, executor, confirmer, command, args, path, None).await
}

/// Run a single-host command with the caller-requested bounded deadline.
pub async fn exec_with_timeout(
    host: &HostConfig,
    executor: &dyn SshExecutor,
    confirmer: &dyn Confirmer,
    command: &str,
    args: &[String],
    path: Option<&str>,
    timeout_secs: Option<u64>,
) -> Result<Value> {
    // Syntactic + symlink guard for path (optional).
    if let Some(p) = path {
        validate_scout_read_path(host, p)?;
    }

    // Command allowlist check (hard error before any IO).
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    validate_command_args(host, command, &arg_refs)?;

    // Destructive gate (B5). Caller supplies confirmer; we just call .require().
    let details = format!(
        "command={command} host={}{}",
        host.name,
        path.map(|p| format!(" path={p}")).unwrap_or_default()
    );
    confirmer.require("scout:exec", &details).await?;

    let arg_strs = arg_refs;

    let timeout_secs = timeout_secs
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(1, MAX_TIMEOUT_SECS);
    let operation = async {
        if is_local_host(host) {
            exec_local(host, command, &arg_strs, path).await
        } else {
            if path.is_some() {
                bail!("path is only supported for local scout exec targets");
            }
            exec_remote(host, executor, command, &arg_strs).await
        }
    };
    crate::runtime_budget::with_deadline(
        &format!("scout exec `{command}` on {}", host.name),
        Duration::from_secs(timeout_secs),
        operation,
    )
    .await
}

fn bound_exec_args(host: &HostConfig, command: &str, args: &[&str]) -> Result<(String, String)> {
    let mut specs = Vec::new();
    for index in command_filesystem_operand_indices(command, args)? {
        let (root, relative) = crate::secure_path::root_and_relative(host, args[index])?;
        specs.push(json!({"index": index, "root": root, "relative": relative}));
    }
    Ok((serde_json::to_string(args)?, serde_json::to_string(&specs)?))
}

async fn exec_local(
    host: &HostConfig,
    command: &str,
    args: &[&str],
    path: Option<&str>,
) -> Result<Value> {
    let (argv, specs) = bound_exec_args(host, command, args)?;
    let cwd = if let Some(path) = path {
        let (root, relative) = crate::secure_path::root_and_relative(host, path)?;
        serde_json::to_string(&json!({"root": root, "relative": relative}))?
    } else {
        "null".into()
    };
    let output = crate::runtime_budget::run_local_command(
        "python3",
        &["-c", BOUND_EXEC_SCRIPT, command, &argv, &specs, &cwd],
        None,
    )
    .await?;
    Ok(json!({
        "host": host.name,
        "command": command,
        "args": args,
        "path": path,
        "exit_code": output.exit_code,
        "stdout": output.stdout,
        "stderr": output.stderr,
    }))
}

async fn exec_remote(
    host: &HostConfig,
    executor: &dyn SshExecutor,
    command: &str,
    args: &[&str],
) -> Result<Value> {
    let (argv, specs) = bound_exec_args(host, command, args)?;
    let out = executor
        .exec(
            host,
            "python3",
            &["-c", BOUND_EXEC_SCRIPT, command, &argv, &specs, "null"],
        )
        .await?;
    Ok(json!({
        "host": host.name,
        "command": command,
        "args": args,
        "path": null, // cwd change not supported for remote SSH exec (no shell)
        "exit_code": out.exit_code,
        "stdout": out.stdout,
        "stderr": out.stderr,
    }))
}

// ─── emit ─────────────────────────────────────────────────────────────────────

/// An `{host, path}` target for `emit`.
#[derive(Clone, Debug)]
pub struct EmitTarget {
    pub host: HostConfig,
    pub path: Option<String>,
}

/// Run `command` on each `targets` host with bounded concurrency (B6 fanout).
///
/// Uses `crate::fanout::fanout` with `min(N, 8)` concurrency and a per-host
/// timeout. The executor is passed as `Arc<dyn SshExecutor>` so it can be
/// cloned into the fanout closure without unsafe.
///
/// Destructive gate fires ONCE before the fanout — one confirmation for the
/// whole multi-host operation.
pub async fn emit(
    targets: &[EmitTarget],
    executor: Arc<dyn SshExecutor>,
    confirmer: &dyn Confirmer,
    command: &str,
    args: &[String],
    timeout_secs: Option<u64>,
) -> Result<Value> {
    if targets.is_empty() {
        bail!("emit: targets must not be empty");
    }

    let target_paths = target_paths_by_host(targets)?;

    // Validate every target's command policy before asking for confirmation.
    // A host-specific custom command is valid only when every selected target
    // explicitly allowlists it.
    for target in targets {
        validate_command(command, &target.host.exec_allowlist)?;
    }

    let target_labels: Vec<String> = targets
        .iter()
        .map(|t| match &t.path {
            Some(path) => format!("{}:{path}", t.host.name),
            None => t.host.name.clone(),
        })
        .collect();
    let details = format!("command={command} targets={}", target_labels.join(", "));
    confirmer.require("scout:emit", &details).await?;

    let timeout = Duration::from_secs(
        timeout_secs
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .clamp(1, MAX_TIMEOUT_SECS),
    );

    // Build the host list from targets (fanout works over HostConfig slices).
    let host_configs: Vec<HostConfig> = targets.iter().map(|t| t.host.clone()).collect();
    let command_owned = command.to_owned();
    let args_owned: Vec<String> = args.to_vec();
    let target_paths = Arc::new(target_paths);

    let outcome: FanoutOutcome<Value, String> = fanout(&host_configs, move |host| {
        let ex = Arc::clone(&executor);
        let cmd = command_owned.clone();
        let arg_strs: Vec<String> = args_owned.clone();
        let target_paths = Arc::clone(&target_paths);
        async move {
            // Per-host command validation (host-specific allowlist may differ).
            let arg_refs: Vec<&str> = arg_strs.iter().map(|s| s.as_str()).collect();
            validate_command_args(&host, &cmd, &arg_refs).map_err(|e| e.to_string())?;
            let path = target_paths
                .get(&host.name)
                .and_then(|path| path.as_deref());

            let fut = async {
                if is_local_host(&host) {
                    exec_local_fanout(&host, &cmd, &arg_refs, path).await
                } else if let Some(path) = path {
                    Err(anyhow::anyhow!(
                        "target path {path} is only supported for local emit targets"
                    ))
                } else {
                    exec_remote_fanout(&host, ex.as_ref(), &cmd, &arg_refs).await
                }
            };

            tokio::time::timeout(timeout, fut)
                .await
                .map_err(|_| format!("timed out after {}s", timeout.as_secs()))?
                .map_err(|e| e.to_string())
        }
    })
    .await;

    let total = host_configs.len();
    let ok_count = outcome.ok_results().len();
    let err_count = outcome.err_results().len();

    let status = match &outcome {
        FanoutOutcome::AllOk(_) => "all_ok",
        FanoutOutcome::PartialSuccess { .. } => "partial_success",
        FanoutOutcome::AllFailed(_) => "all_failed",
    };

    let mut results: Vec<Value> = Vec::with_capacity(total);
    for (host, v) in outcome.ok_results() {
        results.push(json!({ "host": host, "ok": true, "result": v }));
    }
    for (host, e) in outcome.err_results() {
        results.push(json!({ "host": host, "ok": false, "error": e }));
    }

    Ok(json!({
        "command": command,
        "total": total,
        "succeeded": ok_count,
        "failed": err_count,
        "status": status,
        "results": results,
    }))
}

fn target_paths_by_host(targets: &[EmitTarget]) -> Result<HashMap<String, Option<String>>> {
    let mut paths = HashMap::new();
    for target in targets {
        if let Some(path) = &target.path {
            validate_scout_read_path(&target.host, path)?;
        }
        match paths.get(&target.host.name) {
            Some(existing) if existing != &target.path => {
                bail!(
                    "emit target {} appears multiple times with different paths",
                    target.host.name
                );
            }
            _ => {
                paths.insert(target.host.name.clone(), target.path.clone());
            }
        }
    }
    Ok(paths)
}

async fn exec_local_fanout(
    host: &HostConfig,
    command: &str,
    args: &[&str],
    path: Option<&str>,
) -> Result<Value> {
    exec_local(host, command, args, path).await
}

async fn exec_remote_fanout(
    host: &HostConfig,
    executor: &dyn SshExecutor,
    command: &str,
    args: &[&str],
) -> Result<Value> {
    exec_remote(host, executor, command, args).await
}

// ─── beam ────────────────────────────────────────────────────────────────────

/// Transfer one bounded file between managed hosts. Both endpoints obey the
/// same read-root and sensitive-path policy as `peek`/`delta`; the production
/// executor performs descriptor-confined local or SSH file IO.
pub async fn beam(
    source_host: &HostConfig,
    source_path: &str,
    dest_host: &HostConfig,
    dest_path: &str,
    executor: &dyn SshExecutor,
    confirmer: &dyn Confirmer,
) -> Result<Value> {
    validate_scout_read_path(source_host, source_path)?;
    validate_scout_read_path(dest_host, dest_path)?;

    let source_label = format!("{}:{}", source_host.name, source_path);
    let dest_label = format!("{}:{}", dest_host.name, dest_path);
    let details = format!("{source_label} -> {dest_label}");
    confirmer.require("scout:beam", &details).await?;

    let transferred_bytes = executor
        .transfer_file(source_host, source_path, dest_host, dest_path)
        .await?;

    Ok(json!({
        "source": source_label,
        "destination": dest_label,
        "status": "transferred",
        "bytes": transferred_bytes,
    }))
}
