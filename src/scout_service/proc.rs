//! Scout process / disk operations: `ps`, `df`.
//!
//! Both run via the `HostExec` seam (local or SSH) so they work on any
//! configured host without spawning a shell.

use anyhow::{Result, bail};
use serde_json::{Value, json};

#[cfg(test)]
#[path = "proc_tests.rs"]
mod tests;

use crate::flux_service::host::{HostExec, LocalExec, RemoteExec, is_local_host};
use crate::ssh::SshExecutor;
use crate::synapse::{HostConfig, validate_safe_path};

/// Valid sort fields for `ps --sort`.
const VALID_PS_SORTS: &[&str] = &["cpu", "mem", "pid", "time"];

// ─── ps ──────────────────────────────────────────────────────────────────────

/// List processes on `host`.
///
/// Parameters:
/// - `sort` — one of `cpu|mem|pid|time` (default `cpu`)
/// - `grep` — substring filter applied after sort
/// - `user` — prefix match on user column
/// - `limit` — max rows returned (default 50)
pub async fn ps(
    host: &HostConfig,
    executor: &dyn SshExecutor,
    sort: Option<&str>,
    grep: Option<&str>,
    user: Option<&str>,
    limit: Option<u32>,
) -> Result<Value> {
    let sort_val = sort.unwrap_or("cpu");

    // Defense-in-depth: reject any sort value not in the safe list.
    if !VALID_PS_SORTS.contains(&sort_val) {
        bail!(
            "invalid sort `{sort_val}`; must be one of: {}",
            VALID_PS_SORTS.join(", ")
        );
    }

    // GNU ps calls the memory percentage field `%mem`; `mem` is our stable API
    // name and is not itself a valid sort specifier.
    let sort_key = match sort_val {
        "mem" => "%mem",
        other => other,
    };
    let sort_arg = format!("-{sort_key}");
    let args = &["aux", "--sort", sort_arg.as_str()];

    let output = if is_local_host(host) {
        LocalExec.run("ps", args).await?
    } else {
        RemoteExec { executor, host }.run("ps", args).await?
    };

    if !output.success() {
        let exit = output
            .exit_code
            .map_or_else(|| "unknown".to_owned(), |code| code.to_string());
        let stderr = output.stderr.trim();
        bail!("ps on {} failed with exit code {exit}: {stderr}", host.name);
    }

    let raw = output.stdout;
    let limit = limit.unwrap_or(50) as usize;

    // Parse: header line + data lines.
    let mut lines: Vec<&str> = raw.lines().collect();
    let header = lines.first().copied().unwrap_or("").to_owned();
    if !lines.is_empty() {
        lines.remove(0);
    }

    // Apply user prefix filter first, then grep substring filter.
    if let Some(u) = user {
        lines.retain(|l| l.starts_with(u));
    }
    if let Some(g) = grep {
        lines.retain(|l| l.contains(g));
    }

    let truncated = lines.len() > limit;
    let rows: Vec<String> = lines
        .into_iter()
        .take(limit)
        .map(|l| l.to_owned())
        .collect();

    Ok(json!({
        "host": host.name,
        "sort": sort_val,
        "header": header,
        "rows": rows,
        "truncated": truncated,
    }))
}

// ─── df ──────────────────────────────────────────────────────────────────────

/// Report disk usage on `host`.
///
/// If `path` is supplied it is appended as the positional argument to `df -h`.
/// The path is validated by `validate_safe_path` to prevent option injection.
pub async fn df(
    host: &HostConfig,
    executor: &dyn SshExecutor,
    path: Option<&str>,
) -> Result<Value> {
    if let Some(p) = path {
        validate_safe_path(p)?;
    }

    let path_owned;
    let args: Vec<&str> = match path {
        Some(p) => {
            path_owned = p.to_owned();
            vec!["-h", path_owned.as_str()]
        }
        None => vec!["-h"],
    };

    let output = if is_local_host(host) {
        LocalExec.run("df", &args).await?
    } else {
        RemoteExec { executor, host }.run("df", &args).await?
    }
    .require_success("df")?;

    Ok(json!({
        "host": host.name,
        "path": path,
        "disk_usage": output.stdout.trim(),
    }))
}
