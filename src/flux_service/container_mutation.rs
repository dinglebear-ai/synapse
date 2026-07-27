//! Container lifecycle, pull, recreate, and exec driver methods.

use anyhow::Result;
use serde_json::Value;

use super::{
    FluxService,
    container_lifecycle::{self, ExecParams, RecreateParams},
    container_read,
};
use crate::elicitation_gate::Confirmer;
use crate::scout;

#[cfg(test)]
#[path = "container_mutation_tests.rs"]
mod tests;

impl FluxService {
    /// Perform a simple lifecycle action, requiring an explicit host.
    pub async fn container_lifecycle(
        &self,
        host: Option<&str>,
        container_id: &str,
        subaction: &str,
        confirmer: &dyn Confirmer,
    ) -> Result<Value> {
        let host = host.ok_or_else(|| {
            anyhow::anyhow!("host is required for container {subaction} operations")
        })?;
        if subaction == "stop" {
            confirmer
                .require("container stop", &format!("stop container {container_id}"))
                .await
                .map_err(anyhow::Error::from)?;
        }
        let subaction = subaction.to_owned();
        self.find_host_op(Some(host), container_id, move |client, host_name, id| {
            let subaction = subaction.clone();
            Box::pin(async move {
                container_lifecycle::lifecycle_action_on_host(client, host_name, id, &subaction)
                    .await
            })
        })
        .await
    }

    /// Pull the latest image for a container on an explicit host.
    pub async fn container_pull(&self, host: Option<&str>, container_id: &str) -> Result<Value> {
        let host = host.ok_or_else(|| anyhow::anyhow!("host is required for container pull"))?;
        let inspect = self
            .find_host_op(Some(host), container_id, |client, host_name, id| {
                Box::pin(container_read::inspect_on_host(
                    client, host_name, id, false,
                ))
            })
            .await?;

        let host_name = inspect
            .get("host")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("inspect returned no host"))?;
        let image_ref = inspect
            .pointer("/container/Config/Image")
            .or_else(|| inspect.pointer("/container/config/Image"))
            .or_else(|| inspect.pointer("/container/config/image"))
            .and_then(Value::as_str)
            .unwrap_or_default();

        let resolved = scout::resolve_host(self.host_repo.as_ref(), host_name)?;
        let client = self.docker_clients.client_for(&resolved).await?;
        container_lifecycle::pull_image_on_host(client.as_ref(), &resolved.name, image_ref)
            .await
            .map_err(Into::into)
    }

    /// Recreate a container after confirmation.
    pub async fn container_recreate(
        &self,
        host: Option<&str>,
        container_id: &str,
        params: RecreateParams,
        confirmer: &dyn Confirmer,
    ) -> Result<Value> {
        let host =
            host.ok_or_else(|| anyhow::anyhow!("host is required for container recreate"))?;
        let resolved = self.target_hosts(Some(host))?[0].clone();
        confirmer
            .require(
                "container recreate",
                &format!("recreate container {container_id} on {}", resolved.name),
            )
            .await
            .map_err(anyhow::Error::from)?;

        let client = self.docker_clients.client_for(&resolved).await?;
        container_lifecycle::recreate_on_host(
            client.as_ref(),
            &resolved.name,
            container_id,
            &params,
        )
        .await
        .map_err(Into::into)
    }

    /// Execute a command inside a container after confirmation.
    pub async fn container_exec(
        &self,
        host: Option<&str>,
        params: ExecParams,
        confirmer: &dyn Confirmer,
    ) -> Result<Value> {
        let host = host.ok_or_else(|| anyhow::anyhow!("host is required for container exec"))?;
        let container_id = params.container_id.clone();
        confirmer
            .require(
                "container exec",
                &format!(
                    "{} on {}",
                    params.command.first().map(String::as_str).unwrap_or(""),
                    container_id
                ),
            )
            .await
            .map_err(anyhow::Error::from)?;

        self.find_host_op(Some(host), &container_id, move |client, host_name, _| {
            let params = params.clone();
            Box::pin(
                async move { container_lifecycle::exec_on_host(client, host_name, &params).await },
            )
        })
        .await
    }
}
