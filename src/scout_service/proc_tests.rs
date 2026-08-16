//! Unit tests for scout process/disk operations (ps/df).

use crate::ssh::{CommandOutput, SshExecutor};
use crate::synapse::{HostConfig, HostProtocol};
use async_trait::async_trait;

fn ssh_host() -> HostConfig {
    let mut host = HostConfig::local();
    host.name = "remote".into();
    host.host = "remote.example".into();
    host.protocol = HostProtocol::Ssh;
    host
}

struct PsExec;

#[async_trait]
impl SshExecutor for PsExec {
    async fn exec(
        &self,
        _: &HostConfig,
        program: &str,
        args: &[&str],
    ) -> anyhow::Result<CommandOutput> {
        if program == "ps" && args == ["aux", "--sort", "-%mem"] {
            Ok(CommandOutput {
                stdout: "USER PID %CPU %MEM\nroot 42 1.0 9.5\n".into(),
                stderr: String::new(),
                exit_code: Some(0),
            })
        } else {
            Ok(CommandOutput {
                stdout: String::new(),
                stderr: "error: unknown sort specifier".into(),
                exit_code: Some(1),
            })
        }
    }
}

#[test]
fn ps_memory_sort_uses_percent_mem_key() {
    let result = tokio::runtime::Runtime::new().unwrap().block_on(super::ps(
        &ssh_host(),
        &PsExec,
        Some("mem"),
        None,
        None,
        None,
    ));

    let value = result.expect("memory sort should use GNU ps's %mem key");
    assert_eq!(value["header"], "USER PID %CPU %MEM");
    assert_eq!(value["rows"], serde_json::json!(["root 42 1.0 9.5"]));
}

#[test]
fn ps_reports_nonzero_command_exit_with_stderr() {
    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(super::ps(
            &ssh_host(),
            &PsExec,
            Some("pid"),
            None,
            None,
            None,
        ))
        .expect_err("non-zero ps exit must not become an empty success");

    let message = error.to_string();
    assert!(message.contains("exit code 1"), "{message}");
    assert!(message.contains("unknown sort specifier"), "{message}");
}

#[test]
fn ps_rejects_invalid_sort() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let host = HostConfig::local();

    struct NoopExec;
    #[async_trait]
    impl SshExecutor for NoopExec {
        async fn exec(&self, _: &HostConfig, _: &str, _: &[&str]) -> anyhow::Result<CommandOutput> {
            Ok(CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
    }

    let result = rt.block_on(super::ps(
        &host,
        &NoopExec,
        Some("inject; rm -rf /"),
        None,
        None,
        None,
    ));
    assert!(result.is_err(), "invalid sort must be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("invalid sort"), "{msg}");
}

#[test]
fn df_rejects_relative_path() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let host = HostConfig::local();

    use crate::ssh::{CommandOutput, SshExecutor};
    use async_trait::async_trait;
    struct NoopExec;
    #[async_trait]
    impl SshExecutor for NoopExec {
        async fn exec(&self, _: &HostConfig, _: &str, _: &[&str]) -> anyhow::Result<CommandOutput> {
            Ok(CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: Some(0),
            })
        }
    }

    let result = rt.block_on(super::df(&host, &NoopExec, Some("relative/path")));
    assert!(result.is_err(), "relative path must be rejected");
}

#[test]
fn df_reports_nonzero_command_exit() {
    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(super::df(&ssh_host(), &PsExec, None))
        .expect_err("non-zero df exit must not become empty disk usage");
    assert!(error.to_string().contains("exit code 1"), "{error}");
}
