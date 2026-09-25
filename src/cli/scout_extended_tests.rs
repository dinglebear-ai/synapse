use super::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn logs_line_count_is_clamped() {
    let command =
        parse_scout_logs("journal", &args(&["--host", "devhost", "--lines", "9999"])).unwrap();
    let Command::ScoutLogs(parsed) = command else {
        panic!("expected ScoutLogs");
    };
    assert_eq!(parsed.lines, MAX_LINES);
}

#[test]
fn zfs_dataset_recursive_flag_is_preserved() {
    let command = parse_scout_zfs(
        "datasets",
        &args(&["--host", "devhost", "--pool", "tank", "--recursive"]),
    )
    .unwrap();
    let Command::ScoutZfs(parsed) = command else {
        panic!("expected ScoutZfs");
    };
    assert!(parsed.recursive);
    assert_eq!(parsed.pool.as_deref(), Some("tank"));
}

#[test]
fn unknown_extended_subactions_fail_closed() {
    assert!(parse_scout_logs("everything", &args(&["--host", "devhost"])).is_err());
    assert!(parse_scout_zfs("destroy", &args(&["--host", "devhost"])).is_err());
}
