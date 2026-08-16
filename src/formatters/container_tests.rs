use serde_json::json;

use crate::formatters::render_action_output;

#[test]
fn generic_markdown_is_bounded_and_uses_a_safe_fence() {
    let injected = json!({"large": "x".repeat(32 * 1024)});
    let rendered =
        render_action_output("flux", "container", Some("stats"), None, &injected).unwrap();

    assert!(rendered.len() < 17 * 1024, "generic output was not bounded");
    assert!(rendered.contains("Output truncated at 16 KiB"));
    assert!(rendered.contains(r#""truncated":true"#));
    assert!(rendered.ends_with("full result."));

    let fenced = render_action_output(
        "flux",
        "container",
        Some("stats"),
        None,
        &json!({"value": "```\nnot outside the block"}),
    )
    .unwrap();
    assert!(fenced.contains("````json\n"));
    assert!(fenced.ends_with("\n````"));
}

#[test]
fn current_container_payloads_render_their_data_in_default_markdown() {
    let cases = [
        (
            "list",
            json!({"count":1,"containers":[{"host":"dookie","name":"api","image":"app:v1","state":"running"}],"partial":false}),
            "api",
        ),
        (
            "inspect",
            json!({"host":"dookie","container":{"Name":"/api","Config":{"Image":"app:v1"},"State":{"Status":"running"}},"summary":false}),
            "app:v1",
        ),
        (
            "search",
            json!({"query":"api","count":1,"containers":[{"host":"dookie","name":"api","image":"app:v1","state":"running"}],"partial":false}),
            "api",
        ),
    ];
    for (subaction, payload, expected) in cases {
        let rendered =
            render_action_output("flux", "container", Some(subaction), None, &payload).unwrap();
        assert!(
            rendered.contains(expected),
            "container {subaction} dropped {expected:?}: {rendered}"
        );
    }
}

#[test]
fn current_container_inspect_is_routed_and_redacts_secrets() {
    let payload = json!({
        "host": "dookie",
        "container": {
            "Name": "/api",
            "Config": {"Image": "app:v1", "Env": ["API_TOKEN=do-not-print", "MODE=prod"]},
            "State": {"Status": "running"}
        },
        "summary": false
    });
    let rendered =
        render_action_output("flux", "container", Some("inspect"), None, &payload).unwrap();

    assert!(rendered.contains("Container: api (dookie)"));
    assert!(rendered.contains("API_TOKEN=****"));
    assert!(rendered.contains("MODE=prod"));
    assert!(!rendered.contains("do-not-print"));
}
