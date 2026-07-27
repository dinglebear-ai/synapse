use std::io::Read;

use super::*;

#[test]
fn descriptor_open_reads_regular_file_beneath_root() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("value.txt");
    std::fs::write(&path, "safe").unwrap();
    let mut host = HostConfig::local();
    host.scout_read_roots = vec![dir.path().to_string_lossy().into_owned()];

    let mut content = String::new();
    bind_read_path(&host, path.to_str().unwrap())
        .unwrap()
        .into_file()
        .read_to_string(&mut content)
        .unwrap();
    assert_eq!(content, "safe");
}

#[test]
fn descriptor_write_creates_and_truncates_regular_file_beneath_root() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("value.txt");
    std::fs::write(&path, "old payload").unwrap();
    let mut host = HostConfig::local();
    host.scout_read_roots = vec![dir.path().to_string_lossy().into_owned()];

    let mut file = bind_write_path(&host, path.to_str().unwrap())
        .unwrap()
        .into_file();
    file.write_all(b"new").unwrap();
    drop(file);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "new");
}

#[cfg(unix)]
#[test]
fn descriptor_open_rejects_intermediate_symlink() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    let mut host = HostConfig::local();
    host.scout_read_roots = vec![root.path().to_string_lossy().into_owned()];

    let escaped = root.path().join("link/secret.txt");
    assert!(bind_read_path(&host, escaped.to_str().unwrap()).is_err());
    assert!(bind_write_path(&host, escaped.to_str().unwrap()).is_err());
}

#[cfg(unix)]
#[test]
fn descriptor_write_rejects_final_symlink() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("target.txt");
    std::fs::write(&target, "outside").unwrap();
    let link = root.path().join("destination.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let mut host = HostConfig::local();
    host.scout_read_roots = vec![root.path().to_string_lossy().into_owned()];
    assert!(bind_write_path(&host, link.to_str().unwrap()).is_err());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "outside");
}
