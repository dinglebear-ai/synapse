//! Unit tests for SynapseService — sidecar file for src/app.rs
//!
//! Declared in app.rs as:
//! ```rust
//! #[cfg(test)]
//! #[path = "app_tests.rs"]
//! mod tests;
//! ```
//!
//! The facade tests verify that `SynapseService` correctly delegates to its
//! sub-services. Domain-specific behavior lives in the sub-service sidecars
//! (flux_service_tests.rs, scout_service_tests.rs) and the scaffold contract
//! tests live in scaffold_tests.rs.

use super::*;

/// Build a stub SynapseService for testing without real credentials.
fn stub_service() -> SynapseService {
    SynapseService::new()
}

#[test]
fn test_scaffold_intent_delegates_through_facade() {
    let service = stub_service();
    let result = service
        .scaffold_intent(ScaffoldIntent {
            display_name: "Lab Gateway".into(),
            crate_name: "lab-gateway-mcp".into(),
            binary_name: "lab-gateway".into(),
            server_category: "application platform".into(),
            env_prefix: "lab".into(),
            auth_kind: "api key".into(),
            host: "".into(),
            port: 3100,
            mcp_transport: "streamable-http".into(),
            mcp_primitives: "tools, resources".into(),
            deployment: "containers".into(),
            plugins: "claude".into(),
            publish_mcp: true,
            crawl_urls: "https://docs.synapse.test".into(),
            crawl_repos: "".into(),
            crawl_search_topics: "Lab API".into(),
        })
        .expect("valid scaffold intent should build through the facade");

    assert_eq!(result["kind"], "synapse_scaffold_intent");
    assert_eq!(result["project"]["service_name"], "lab_gateway");
}
