//! Descriptor-confined, bounded file transfer over local files or SSH.

use anyhow::{Context, Result, bail};
use openssh::Stdio;
use tokio::io::AsyncWriteExt;

use crate::secure_path::{bind_read_path, bind_write_path, root_and_relative};
use crate::synapse::HostConfig;

use super::SshPool;

/// File transfers are intentionally bounded so one request cannot consume
/// unbounded memory or saturate the control plane.
pub const MAX_TRANSFER_BYTES: usize = 64 * 1024 * 1024;

const REMOTE_READ_SCRIPT: &str = r#"import os, stat, sys
root, rel, cap = sys.argv[1], sys.argv[2], int(sys.argv[3])
fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
try:
    for part in [p for p in root.split('/') if p] + [p for p in rel.split('/') if p]:
        nxt = os.open(part, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        os.close(fd); fd = nxt
    meta = os.fstat(fd)
    if not stat.S_ISREG(meta.st_mode): raise RuntimeError('source is not a regular file')
    if meta.st_size > cap: raise RuntimeError('source exceeds transfer byte limit')
    while True:
        data = os.read(fd, 65536)
        if not data: break
        sys.stdout.buffer.write(data)
finally:
    os.close(fd)
"#;

const REMOTE_WRITE_SCRIPT: &str = r#"import os, sys
root, rel, cap = sys.argv[1], sys.argv[2], int(sys.argv[3])
parts = [p for p in root.split('/') if p] + [p for p in rel.split('/') if p]
if not parts: raise RuntimeError('destination must name a file')
fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
try:
    for part in parts[:-1]:
        nxt = os.open(part, os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY, dir_fd=fd)
        os.close(fd); fd = nxt
    out = os.open(parts[-1], os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600, dir_fd=fd)
    try:
        total = 0
        while True:
            data = sys.stdin.buffer.read(65536)
            if not data: break
            total += len(data)
            if total > cap: raise RuntimeError('destination exceeded transfer byte limit')
            view = memoryview(data)
            while view:
                written = os.write(out, view)
                view = view[written:]
    finally:
        os.close(out)
finally:
    os.close(fd)
"#;

pub async fn transfer_file(
    pool: &SshPool,
    source_host: &HostConfig,
    source_path: &str,
    dest_host: &HostConfig,
    dest_path: &str,
) -> Result<u64> {
    let bytes = read_source(pool, source_host, source_path).await?;
    write_destination(pool, dest_host, dest_path, &bytes).await?;
    Ok(bytes.len() as u64)
}

async fn read_source(pool: &SshPool, host: &HostConfig, path: &str) -> Result<Vec<u8>> {
    if host.is_local() {
        let bound = bind_read_path(host, path)?;
        let metadata = bound.file().metadata()?;
        if !metadata.is_file() {
            bail!("beam source is not a regular file");
        }
        if metadata.len() > MAX_TRANSFER_BYTES as u64 {
            bail!("beam source exceeds {MAX_TRANSFER_BYTES} byte limit");
        }
        return crate::runtime_budget::read_bytes_limited(
            tokio::fs::File::from_std(bound.into_file()),
            MAX_TRANSFER_BYTES,
        )
        .await;
    }

    let (root, relative) = root_and_relative(host, path)?;
    let pooled = pool.checkout(host).await?;
    let _permit = pooled
        .permits
        .acquire()
        .await
        .context("SSH transfer semaphore closed")?;
    let mut command = pooled.session().arc_command("python3".to_owned());
    command
        .arg("-c")
        .arg(REMOTE_READ_SCRIPT)
        .arg(root)
        .arg(relative)
        .arg(MAX_TRANSFER_BYTES.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().await?;
    let stdout = child
        .stdout()
        .take()
        .ok_or_else(|| anyhow::anyhow!("SSH transfer stdout unavailable"))?;
    let stderr = child
        .stderr()
        .take()
        .ok_or_else(|| anyhow::anyhow!("SSH transfer stderr unavailable"))?;
    let (bytes, stderr) = tokio::try_join!(
        crate::runtime_budget::read_bytes_limited(stdout, MAX_TRANSFER_BYTES),
        crate::runtime_budget::drain_bounded(
            stderr,
            crate::runtime_budget::SERVICE_TEXT_FIELD_BYTE_CAP,
        ),
    )?;
    let status = child.wait().await?;
    if !status.success() {
        bail!(
            "remote beam read failed on {}: {}",
            host.name,
            stderr.trim()
        );
    }
    Ok(bytes)
}

async fn write_destination(
    pool: &SshPool,
    host: &HostConfig,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    if host.is_local() {
        let bound = bind_write_path(host, path)?;
        let mut file = tokio::fs::File::from_std(bound.into_file());
        file.write_all(bytes).await?;
        file.flush().await?;
        return Ok(());
    }

    let (root, relative) = root_and_relative(host, path)?;
    let pooled = pool.checkout(host).await?;
    let _permit = pooled
        .permits
        .acquire()
        .await
        .context("SSH transfer semaphore closed")?;
    let mut command = pooled.session().arc_command("python3".to_owned());
    command
        .arg("-c")
        .arg(REMOTE_WRITE_SCRIPT)
        .arg(root)
        .arg(relative)
        .arg(MAX_TRANSFER_BYTES.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().await?;
    let mut stdin = child
        .stdin()
        .take()
        .ok_or_else(|| anyhow::anyhow!("SSH transfer stdin unavailable"))?;
    stdin.write_all(bytes).await?;
    stdin.shutdown().await?;
    drop(stdin);
    let stderr = child
        .stderr()
        .take()
        .ok_or_else(|| anyhow::anyhow!("SSH transfer stderr unavailable"))?;
    let stderr = crate::runtime_budget::drain_bounded(
        stderr,
        crate::runtime_budget::SERVICE_TEXT_FIELD_BYTE_CAP,
    )
    .await?;
    let status = child.wait().await?;
    if !status.success() {
        bail!(
            "remote beam write failed on {}: {}",
            host.name,
            stderr.trim()
        );
    }
    Ok(())
}
