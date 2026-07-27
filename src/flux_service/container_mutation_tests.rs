//! Sidecar tests for container mutation driver preconditions.

use std::sync::Arc;

use crate::elicitation_gate::NoConfirm;
use crate::host_config::HostRepository;

use super::*;

struct EmptyRepo;

impl HostRepository for EmptyRepo {
    fn load_hosts(&self) -> anyhow::Result<Vec<crate::synapse::HostConfig>> {
        Ok(Vec::new())
    }
}

fn service() -> FluxService {
    FluxService::new(Arc::new(EmptyRepo))
}

#[tokio::test]
async fn lifecycle_requires_explicit_host_before_docker_or_confirmation() {
    let error = service()
        .container_lifecycle(None, "web", "stop", &NoConfirm)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("host is required"));
}

#[tokio::test]
async fn pull_and_recreate_require_explicit_host() {
    assert!(service().container_pull(None, "web").await.is_err());
    assert!(
        service()
            .container_recreate(None, "web", RecreateParams::default(), &NoConfirm)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn exec_requires_explicit_host() {
    let params = ExecParams {
        container_id: "web".into(),
        command: vec!["true".into()],
        user: None,
        workdir: None,
        timeout_ms: container_lifecycle::EXEC_TIMEOUT_DEFAULT_MS,
    };
    let error = service()
        .container_exec(None, params, &NoConfirm)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("host is required"));
}
