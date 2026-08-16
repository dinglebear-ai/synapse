use serde_json::json;

use super::*;

#[test]
fn nodes_basic() {
    let data = json!({"hosts":[{"name":"squirts","host":"squirts.local","protocol":"ssh"},{"name":"boops","host":"boops.local","protocol":"ssh"}]});
    let output = render_scout_nodes_markdown(&data);
    assert!(output.starts_with("Scout Nodes"));
    assert!(output.contains("Hosts: 2"));
    assert!(output.contains("squirts"));
    assert!(output.contains("boops"));
    assert!(output.contains("| Host |"));
}

#[test]
fn peek_file_and_directory() {
    let file = render_scout_peek_markdown(
        &json!({"host":"squirts","path":"/etc/hostname","kind":"file","content":"squirts\n","size_bytes":4096,"truncated":true}),
    );
    assert!(file.contains("File Read: squirts:/etc/hostname"));
    assert!(file.contains("Size: 4.0 KB | truncated: yes"));
    assert!(file.contains("```"));

    let directory = render_scout_peek_markdown(
        &json!({"host":"squirts","path":"/etc","kind":"directory","entries":["hostname","hosts","passwd"]}),
    );
    assert!(directory.contains("Directory Listing: squirts:/etc"));
    assert!(directory.contains("Items: 3"));
    assert!(directory.contains("hostname"));
}

#[test]
fn peek_tree_exposes_truncation_and_contains_embedded_fences() {
    let tree = render_scout_peek_markdown(&json!({
        "host":"squirts", "path":"/srv", "depth":2,
        "tree":"/srv\n```\n/srv/app", "truncated":true
    }));
    assert!(tree.contains("Depth: 2 | truncated: yes"));
    assert!(tree.contains("````\n/srv\n```\n/srv/app\n````"));
}

#[test]
fn exec_success_and_failure() {
    let success = render_scout_exec_markdown(
        &json!({"host":"squirts","path":"/tmp","command":"uptime","exit_code":0,"stdout":"up 3 days","stderr":""}),
    );
    assert!(success.starts_with('✓'));
    assert!(success.contains("Command Execution: squirts:/tmp"));
    assert!(success.contains("Exit: 0"));
    assert!(success.contains("As of (UTC):"));

    let failure = render_scout_exec_markdown(
        &json!({"host":"squirts","path":"/tmp","command":"cat /nonexistent","exit_code":1,"stdout":"","stderr":"No such file or directory"}),
    );
    assert!(failure.starts_with('✗'));
    assert!(failure.contains("Exit: 1"));
}

#[test]
fn syslog_legacy_and_current_shapes() {
    let legacy = render_scout_syslog_markdown(
        &json!({"host":"squirts","lines_requested":50,"logs":"one\ntwo"}),
    );
    assert!(legacy.starts_with("Syslog: squirts"));
    assert!(legacy.contains("Lines requested: 50 | Returned: 2"));

    let current = render_scout_syslog_markdown(
        &json!({"host":"squirts","subaction":"syslog","lines":100,"grep":"sshd","output":"Accepted publickey\nsession opened"}),
    );
    assert!(current.contains("Lines requested: 100 | Returned: 2 | truncated: no | Filter: sshd"));
    assert!(current.contains("Accepted publickey"));
}

#[test]
fn zfs_pools_annotates_health() {
    let output = render_scout_zfs_pools_markdown(
        &json!({"host":"squirts","pools":"NAME SIZE HEALTH\ntank 10T ONLINE\nbad 1T DEGRADED"}),
    );
    assert!(output.starts_with("ZFS Pools: squirts"));
    assert!(output.contains('●'));
    assert!(output.contains('⚠'));
}
