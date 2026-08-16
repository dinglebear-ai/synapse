use serde_json::json;

use super::render_action_output;

#[test]
fn current_scout_payloads_render_their_data_in_default_markdown() {
    let cases = [
        (
            "ps",
            None,
            json!({"host":"dookie","header":"USER PID %MEM","rows":["root 42 9.5"]}),
            "root 42 9.5",
        ),
        (
            "find",
            None,
            json!({"host":"dookie","path":"/etc","pattern":"*.conf","files":["/etc/app.conf"]}),
            "/etc/app.conf",
        ),
        (
            "delta",
            None,
            json!({"source":"a:/one","target":"b:/two","diff":"-old\n+new"}),
            "a:/one",
        ),
        (
            "zfs",
            Some("pools"),
            json!({"host":"shart","header":"NAME HEALTH","rows":["tank ONLINE"]}),
            "tank ONLINE",
        ),
        (
            "beam",
            None,
            json!({"source":"a:/one","destination":"b:/two","status":"transferred","bytes":12}),
            "b:/two",
        ),
    ];

    for (action, subaction, payload, expected) in cases {
        let rendered = render_action_output("scout", action, subaction, None, &payload).unwrap();
        assert!(
            rendered.contains(expected),
            "{action}/{subaction:?} dropped {expected:?}: {rendered}"
        );
    }
}

#[test]
fn peek_tree_and_emit_render_current_envelopes() {
    let tree = render_action_output(
        "scout",
        "peek",
        None,
        None,
        &json!({"host":"dookie","path":"/srv","depth":2,"tree":"/srv/app"}),
    )
    .unwrap();
    assert!(tree.contains("/srv/app"), "{tree}");

    let emit = render_action_output(
        "scout",
        "emit",
        None,
        None,
        &json!({"command":"uptime","status":"all_ok","results":[{"host":"dookie","ok":true}]}),
    )
    .unwrap();
    assert!(emit.contains("dookie"), "{emit}");
    assert!(emit.contains("all_ok"), "{emit}");
}

#[test]
fn unmatched_action_markdown_preserves_values_not_only_field_names() {
    let rendered = render_action_output(
        "flux",
        "container",
        Some("stats"),
        None,
        &json!({"host":"dookie","container":"api","cpu_percent":12.5}),
    )
    .unwrap();
    assert!(rendered.contains("dookie"), "{rendered}");
    assert!(rendered.contains("12.5"), "{rendered}");
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
