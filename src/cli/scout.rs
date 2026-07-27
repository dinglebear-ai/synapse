//! CLI scout subtree — parse and run helpers for `scout *`.
//!
//! `parse_scout` builds the `Command` variant; `run_scout` executes it.
//! All calls delegate to `ScoutService` via the thin shim.

use crate::{
    actions::{
        ScoutBeamArgs, ScoutDeltaArgs, ScoutEmitArgs, ScoutEmitTarget, ScoutExecArgs,
        ScoutFindArgs, ScoutPsArgs,
    },
    app::SynapseService,
    elicitation_gate::CliStderrWarn,
};
use anyhow::{Result, anyhow, bail};
use serde_json::Value;

use super::Command;

#[path = "scout_extended.rs"]
mod extended;

// ── parse ─────────────────────────────────────────────────────────────────────

pub(super) fn parse_scout(args: &[String]) -> Result<Command> {
    match args {
        [action, rest @ ..] if action == "nodes" => Ok(Command::ScoutNodes {
            response_format: super::parse_output_format_flag(rest, "scout nodes")?,
        }),
        [action, rest @ ..] if action == "peek" => {
            super::validate_named_args(
                rest,
                &["--depth", "--host", "--path", "--response-format"],
                &["--tree"],
            )?;
            let tree = rest.iter().any(|a| a == "--tree");
            let value_args: Vec<String> = rest.iter().filter(|a| *a != "--tree").cloned().collect();
            let depth = super::parse_optional_number::<u8>(&value_args, "--depth")?
                .map(|v| v.clamp(1, 10))
                .unwrap_or(3);
            Ok(Command::ScoutPeek {
                response_format: super::parse_optional_response_format(&value_args)?,
                host: super::parse_required_named_value(&value_args, "--host")?,
                path: super::parse_required_named_value(&value_args, "--path")?,
                tree,
                depth,
            })
        }
        [action, rest @ ..] if action == "find" => {
            super::validate_named_args(
                rest,
                &[
                    "--depth",
                    "--limit",
                    "--host",
                    "--path",
                    "--pattern",
                    "--response-format",
                ],
                &[],
            )?;
            let depth =
                super::parse_optional_number::<u8>(rest, "--depth")?.map(|v| v.clamp(1, 20));
            let limit = super::parse_optional_number::<u32>(rest, "--limit")?;
            Ok(Command::ScoutFind(Box::new(ScoutFindArgs {
                response_format: super::parse_optional_response_format(rest)?,
                host: super::parse_required_named_value(rest, "--host")?,
                path: super::parse_required_named_value(rest, "--path")?,
                pattern: super::parse_required_named_value(rest, "--pattern")?,
                depth,
                limit,
            })))
        }
        [action, rest @ ..] if action == "ps" => {
            super::validate_named_args(
                rest,
                &[
                    "--limit",
                    "--host",
                    "--sort",
                    "--grep",
                    "--user",
                    "--response-format",
                ],
                &[],
            )?;
            let limit = super::parse_optional_number::<u32>(rest, "--limit")?;
            Ok(Command::ScoutPs(Box::new(ScoutPsArgs {
                response_format: super::parse_optional_response_format(rest)?,
                host: super::parse_required_named_value(rest, "--host")?,
                sort: super::parse_optional_named_value(rest, "--sort")?,
                grep: super::parse_optional_named_value(rest, "--grep")?,
                user: super::parse_optional_named_value(rest, "--user")?,
                limit,
            })))
        }
        [action, rest @ ..] if action == "df" => {
            super::validate_named_args(rest, &["--host", "--path", "--response-format"], &[])?;
            Ok(Command::ScoutDf {
                response_format: super::parse_optional_response_format(rest)?,
                host: super::parse_required_named_value(rest, "--host")?,
                path: super::parse_optional_named_value(rest, "--path")?,
            })
        }
        [action, rest @ ..] if action == "delta" => {
            super::validate_named_args(
                rest,
                &[
                    "--source-host",
                    "--source-path",
                    "--target-host",
                    "--target-path",
                    "--content",
                    "--response-format",
                ],
                &[],
            )?;
            Ok(Command::ScoutDelta(Box::new(ScoutDeltaArgs {
                response_format: super::parse_optional_response_format(rest)?,
                source_host: super::parse_required_named_value(rest, "--source-host")?,
                source_path: super::parse_required_named_value(rest, "--source-path")?,
                target_host: super::parse_optional_named_value(rest, "--target-host")?,
                target_path: super::parse_optional_named_value(rest, "--target-path")?,
                content: super::parse_optional_named_value(rest, "--content")?,
            })))
        }
        [action, rest @ ..] if action == "exec" => {
            let (option_args, command_args) = parse_variadic_args(rest)?;
            super::validate_named_args(
                &option_args,
                &[
                    "--timeout",
                    "--host",
                    "--path",
                    "--command",
                    "--response-format",
                ],
                &[],
            )?;
            let timeout_secs = super::parse_optional_number::<u64>(&option_args, "--timeout")?;
            Ok(Command::ScoutExec(Box::new(ScoutExecArgs {
                response_format: super::parse_optional_response_format(&option_args)?,
                host: super::parse_required_named_value(&option_args, "--host")?,
                path: super::parse_optional_named_value(&option_args, "--path")?,
                command: super::parse_required_named_value(&option_args, "--command")?,
                args: command_args,
                timeout_secs,
            })))
        }
        [action, rest @ ..] if action == "emit" => {
            let (option_args, command_args) = parse_variadic_args(rest)?;
            super::validate_named_args(
                &option_args,
                &["--target", "--command", "--timeout", "--response-format"],
                &[],
            )?;
            // --target HOST:PATH[,HOST:PATH,...] (comma-separated)
            let raw_targets = super::parse_required_named_value(&option_args, "--target")?;
            let targets: Vec<ScoutEmitTarget> = raw_targets
                .split(',')
                .map(|s| {
                    let s = s.trim();
                    if let Some((host, path)) = s.split_once(':') {
                        ScoutEmitTarget {
                            host: host.to_owned(),
                            path: Some(path.to_owned()),
                        }
                    } else {
                        ScoutEmitTarget {
                            host: s.to_owned(),
                            path: None,
                        }
                    }
                })
                .collect();
            let timeout_secs = super::parse_optional_number::<u64>(&option_args, "--timeout")?;
            Ok(Command::ScoutEmit(Box::new(ScoutEmitArgs {
                response_format: super::parse_optional_response_format(&option_args)?,
                targets,
                command: super::parse_required_named_value(&option_args, "--command")?,
                args: command_args,
                timeout_secs,
            })))
        }
        [action, rest @ ..] if action == "beam" => {
            super::validate_named_args(
                rest,
                &[
                    "--source-host",
                    "--source-path",
                    "--dest-host",
                    "--dest-path",
                    "--response-format",
                ],
                &[],
            )?;
            Ok(Command::ScoutBeam(Box::new(ScoutBeamArgs {
                response_format: super::parse_optional_response_format(rest)?,
                source_host: super::parse_required_named_value(rest, "--source-host")?,
                source_path: super::parse_required_named_value(rest, "--source-path")?,
                dest_host: super::parse_required_named_value(rest, "--dest-host")?,
                dest_path: super::parse_required_named_value(rest, "--dest-path")?,
            })))
        }
        [action, subaction, rest @ ..] if action == "zfs" => {
            extended::parse_scout_zfs(subaction, rest)
        }
        [action, subaction, rest @ ..] if action == "logs" => {
            extended::parse_scout_logs(subaction, rest)
        }
        _ => Err(anyhow!("unknown scout command")),
    }
}

/// Remove a single variadic `--args` segment while preserving its argv values.
fn parse_variadic_args(args: &[String]) -> Result<(Vec<String>, Vec<String>)> {
    let Some(start) = args.iter().position(|arg| arg == "--args") else {
        return Ok((args.to_vec(), Vec::new()));
    };
    if args.iter().skip(start + 1).any(|arg| arg == "--args") {
        bail!("duplicate --args");
    }
    let end = args[start + 1..]
        .iter()
        .position(|arg| arg.starts_with("--"))
        .map(|offset| start + 1 + offset)
        .unwrap_or(args.len());
    if end == start + 1 {
        bail!("--args requires at least one value");
    }
    let mut remaining = args[..start].to_vec();
    remaining.extend_from_slice(&args[end..]);
    Ok((remaining, args[start + 1..end].to_vec()))
}

// ── run helpers ───────────────────────────────────────────────────────────────

pub(super) async fn run_scout(
    cmd: &Command,
    service: &SynapseService,
    confirmer: &CliStderrWarn,
) -> Result<Value> {
    let result = match cmd {
        Command::ScoutNodes { .. } => service.scout().nodes().await?,
        Command::ScoutPeek {
            host,
            path,
            tree,
            depth,
            ..
        } => service.scout().peek(host, path, *tree, *depth).await?,
        Command::ScoutFind(a) => {
            service
                .scout()
                .find(&a.host, &a.path, &a.pattern, a.depth, a.limit)
                .await?
        }
        Command::ScoutPs(a) => {
            service
                .scout()
                .ps(
                    &a.host,
                    a.sort.as_deref(),
                    a.grep.as_deref(),
                    a.user.as_deref(),
                    a.limit,
                )
                .await?
        }
        Command::ScoutDf { host, path, .. } => service.scout().df(host, path.as_deref()).await?,
        Command::ScoutDelta(a) => {
            service
                .scout()
                .delta(
                    &a.source_host,
                    &a.source_path,
                    a.target_host.as_deref(),
                    a.target_path.as_deref(),
                    a.content.as_deref(),
                )
                .await?
        }
        Command::ScoutExec(a) => {
            service
                .scout()
                .exec(
                    &a.host,
                    a.path.as_deref(),
                    &a.command,
                    &a.args,
                    a.timeout_secs,
                    confirmer,
                )
                .await?
        }
        Command::ScoutEmit(a) => {
            let targets = service.scout().resolve_emit_targets(
                &a.targets
                    .iter()
                    .map(|t| (t.host.clone(), t.path.clone()))
                    .collect::<Vec<_>>(),
            )?;
            service
                .scout()
                .emit(&targets, &a.command, &a.args, a.timeout_secs, confirmer)
                .await?
        }
        Command::ScoutBeam(a) => {
            service
                .scout()
                .beam(
                    &a.source_host,
                    &a.source_path,
                    &a.dest_host,
                    &a.dest_path,
                    confirmer,
                )
                .await?
        }
        Command::ScoutZfs(a) => crate::actions::scout::dispatch_scout_zfs(service, a).await?,
        Command::ScoutLogs(a) => crate::actions::scout::dispatch_scout_logs(service, a).await?,
        _ => unreachable!("run_scout called with non-scout command"),
    };
    Ok(result)
}
