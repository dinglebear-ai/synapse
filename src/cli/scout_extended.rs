//! Focused parsing helpers for Scout ZFS and log command families.

use anyhow::{Result, bail};

use crate::{
    actions::{ScoutLogsArgs, ScoutZfsArgs},
    scout_service::logs::{DEFAULT_LINES, MAX_LINES},
};

use super::super::{
    parse_optional_named_value, parse_optional_number, parse_optional_response_format,
    parse_required_named_value, validate_named_args,
};
use super::Command;

#[cfg(test)]
#[path = "scout_extended_tests.rs"]
mod tests;

pub(super) fn parse_scout_zfs(subaction: &str, rest: &[String]) -> Result<Command> {
    validate_named_args(
        rest,
        &[
            "--host",
            "--pool",
            "--type",
            "--dataset",
            "--limit",
            "--response-format",
        ],
        &["--recursive"],
    )?;
    let recursive = rest.iter().any(|arg| arg == "--recursive");
    let value_args: Vec<String> = rest
        .iter()
        .filter(|arg| *arg != "--recursive")
        .cloned()
        .collect();
    let host = parse_required_named_value(&value_args, "--host")?;

    match subaction {
        "pools" => Ok(Command::ScoutZfs(Box::new(ScoutZfsArgs {
            response_format: parse_optional_response_format(&value_args)?,
            host,
            subaction: "pools".to_owned(),
            pool: parse_optional_named_value(&value_args, "--pool")?,
            ..Default::default()
        }))),
        "datasets" => Ok(Command::ScoutZfs(Box::new(ScoutZfsArgs {
            response_format: parse_optional_response_format(&value_args)?,
            host,
            subaction: "datasets".to_owned(),
            pool: parse_optional_named_value(&value_args, "--pool")?,
            dataset_type: parse_optional_named_value(&value_args, "--type")?,
            recursive,
            ..Default::default()
        }))),
        "snapshots" => Ok(Command::ScoutZfs(Box::new(ScoutZfsArgs {
            response_format: parse_optional_response_format(&value_args)?,
            host,
            subaction: "snapshots".to_owned(),
            pool: parse_optional_named_value(&value_args, "--pool")?,
            dataset: parse_optional_named_value(&value_args, "--dataset")?,
            limit: parse_optional_number::<u32>(rest, "--limit")?,
            ..Default::default()
        }))),
        other => {
            bail!("unknown zfs subaction `{other}`; must be one of: pools, datasets, snapshots")
        }
    }
}

pub(super) fn parse_scout_logs(subaction: &str, rest: &[String]) -> Result<Command> {
    validate_named_args(
        rest,
        &[
            "--host",
            "--lines",
            "--grep",
            "--unit",
            "--priority",
            "--since",
            "--until",
            "--response-format",
        ],
        &[],
    )?;
    let host = parse_required_named_value(rest, "--host")?;
    let lines = parse_optional_number::<u32>(rest, "--lines")?
        .unwrap_or(DEFAULT_LINES)
        .clamp(1, MAX_LINES);
    let grep = parse_optional_named_value(rest, "--grep")?;

    match subaction {
        "syslog" => Ok(Command::ScoutLogs(Box::new(ScoutLogsArgs {
            response_format: parse_optional_response_format(rest)?,
            host,
            subaction: "syslog".to_owned(),
            lines,
            grep,
            ..Default::default()
        }))),
        "journal" => Ok(Command::ScoutLogs(Box::new(ScoutLogsArgs {
            response_format: parse_optional_response_format(rest)?,
            host,
            subaction: "journal".to_owned(),
            lines,
            grep,
            unit: parse_optional_named_value(rest, "--unit")?,
            priority: parse_optional_named_value(rest, "--priority")?,
            since: parse_optional_named_value(rest, "--since")?,
            until: parse_optional_named_value(rest, "--until")?,
        }))),
        "dmesg" => Ok(Command::ScoutLogs(Box::new(ScoutLogsArgs {
            response_format: parse_optional_response_format(rest)?,
            host,
            subaction: "dmesg".to_owned(),
            lines,
            grep,
            ..Default::default()
        }))),
        "auth" => Ok(Command::ScoutLogs(Box::new(ScoutLogsArgs {
            response_format: parse_optional_response_format(rest)?,
            host,
            subaction: "auth".to_owned(),
            lines,
            grep,
            ..Default::default()
        }))),
        other => {
            bail!("unknown logs subaction `{other}`; must be one of: syslog, journal, dmesg, auth")
        }
    }
}
