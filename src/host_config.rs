//! `HostRepository` trait and `FileHostRepository` implementation.
//!
//! Loads and merges host configurations from multiple sources in precedence order:
//!
//! 1. `SYNAPSE_HOSTS_CONFIG` env (JSON array) — highest priority
//! 2. `SYNAPSE_CONFIG_FILE` env (path override)
//! 3. `./synapse.config.json`
//! 4. `$XDG_CONFIG_HOME/synapse-mcp/config.json`
//! 5. `~/.config/synapse-mcp/config.json`
//! 6. `~/.synapse-mcp.json`
//! 7. `~/.ssh/config` (auto-discovered, additive below the explicit sources)
//! 8. Built-in `local` fallback (lowest priority)
//!
//! **Precedence semantics:**
//! - Explicit sources (1–6): first non-empty source wins entirely (no merging between them).
//! - SSH config (7): additive; any host name already present in the explicit set is skipped.
//! - Ensure-local (8): `HostConfig::local()` is appended if no host named `"local"` exists.
//!
//! **Error policy:**
//! - Malformed JSON in explicit config → propagated as a hard error.
//! - SSH config parse failure → logged and silently returns empty (non-fatal; server must start).
//!
//! **Include directives:** Synapse expands `Include` directives before parsing so relative,
//! absolute, wildcard, and nested includes use the including file's directory consistently.
//! Recursive include cycles are ignored after the first active visit.
//!
//! **Wildcard Host blocks** (e.g. `Host *`, `Host *.example.com`): skipped — they don't
//! represent connectable hosts.

use anyhow::Result;
use std::collections::HashSet;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ssh2_config::{ParseRule, SshConfig};

use crate::synapse::{HostConfig, HostProtocol, HostsFile};

#[cfg(test)]
#[path = "host_config_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Known non-infrastructure hosts to skip during SSH auto-discovery
// ---------------------------------------------------------------------------

/// Well-known git-hosting and backup services that appear in `~/.ssh/config`
/// but are not Synapse-connectable infrastructure hosts.
const SKIP_SSH_HOSTS: &[&str] = &[
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "ssh.github.com",
    "backup.unraid.net",
];

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Abstraction over host config loading, so tests can inject fixtures.
pub trait HostRepository: Send + Sync {
    /// Return the full, merged list of configured hosts.
    fn load_hosts(&self) -> Result<Vec<HostConfig>>;
}

// ---------------------------------------------------------------------------
// FileHostRepository — production implementation
// ---------------------------------------------------------------------------

/// Loads hosts from disk / env following the precedence chain documented
/// in [`crate::host_config`].
///
/// Call [`FileHostRepository::default()`] in production.  In tests, construct
/// with explicit tempfile paths using [`FileHostRepository::for_test`] to avoid
/// reading process env or the real `~/.ssh/config`.
pub struct FileHostRepository {
    /// Pre-captured value of `SYNAPSE_HOSTS_CONFIG` env var (if any).
    env_hosts_json: Option<String>,
    /// Ordered list of JSON config file paths to check (first non-empty wins).
    config_file_paths: Vec<PathBuf>,
    /// Path to the SSH config file, or `None` to skip SSH auto-discovery.
    ssh_config_path: Option<PathBuf>,
    snapshot: Mutex<Option<HostSnapshot>>,
}

#[derive(Clone)]
struct HostSnapshot {
    revision: Vec<(PathBuf, Option<(u64, u128)>)>,
    hosts: Vec<HostConfig>,
}

impl Default for FileHostRepository {
    /// Production constructor — reads env vars and resolves default paths.
    fn default() -> Self {
        Self {
            env_hosts_json: std::env::var("SYNAPSE_HOSTS_CONFIG").ok(),
            config_file_paths: default_config_paths(),
            ssh_config_path: default_ssh_config_path(),
            snapshot: Mutex::new(None),
        }
    }
}

impl FileHostRepository {
    /// Test constructor: all sources explicit, bypasses process env.
    pub fn for_test(
        env_hosts_json: Option<String>,
        config_file_paths: Vec<PathBuf>,
        ssh_config_path: Option<PathBuf>,
    ) -> Self {
        Self {
            env_hosts_json,
            config_file_paths,
            ssh_config_path,
            snapshot: Mutex::new(None),
        }
    }

    // ------------------------------------------------------------------
    // Explicit sources
    // ------------------------------------------------------------------

    /// Load from `SYNAPSE_HOSTS_CONFIG` env (pre-captured at construction).
    ///
    /// Returns `Err` on malformed JSON.
    fn load_from_env_json(&self) -> Result<Vec<HostConfig>> {
        let raw = match &self.env_hosts_json {
            Some(s) if !s.trim().is_empty() => s,
            _ => return Ok(Vec::new()),
        };
        // Hard error on malformed JSON — do not silently fall back.
        let hosts: Vec<HostConfig> = serde_json::from_str(raw)?;
        Ok(hosts)
    }

    /// Scan `config_file_paths` and return hosts from the first non-empty file.
    ///
    /// Returns `Err` on malformed JSON in any file that actually exists.
    fn load_from_files(&self) -> Result<Vec<HostConfig>> {
        for path in &self.config_file_paths {
            if !path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(path)?;
            // Hard error on malformed JSON.
            let parsed: HostsFile = serde_json::from_str(&raw)?;
            if !parsed.hosts.is_empty() {
                tracing::info!(
                    path = %path.display(),
                    count = parsed.hosts.len(),
                    "loaded hosts from config file"
                );
                return Ok(parsed.hosts);
            }
        }
        Ok(Vec::new())
    }

    // ------------------------------------------------------------------
    // SSH auto-discovery
    // ------------------------------------------------------------------

    /// Load hosts from the SSH config file at `self.ssh_config_path`.
    ///
    /// Failures are soft (logged + returns empty) so the server still starts
    /// even with a malformed / missing `~/.ssh/config`.
    fn load_from_ssh_config(&self) -> Vec<HostConfig> {
        let path = match &self.ssh_config_path {
            Some(p) => p,
            None => return Vec::new(),
        };

        if !path.exists() {
            return Vec::new();
        }

        match load_ssh_config_file(path) {
            Ok(hosts) => {
                if !hosts.is_empty() {
                    tracing::info!(
                        path = %path.display(),
                        count = hosts.len(),
                        "auto-discovered hosts from SSH config"
                    );
                }
                hosts
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "SSH config parse failed — continuing without auto-discovered hosts"
                );
                Vec::new()
            }
        }
    }
}

impl HostRepository for FileHostRepository {
    fn load_hosts(&self) -> Result<Vec<HostConfig>> {
        let revision = self.source_revision();
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| anyhow::anyhow!("host topology snapshot lock poisoned"))?;
        if let Some(cached) = snapshot.as_ref()
            && cached.revision == revision
        {
            return Ok(cached.hosts.clone());
        }
        // Step 1: Find the single winning explicit source.
        let mut explicit: Vec<HostConfig> = self.load_from_env_json()?;
        if explicit.is_empty() {
            explicit = self.load_from_files()?;
        }

        // Reject unsupported protocols early — Http/Https would silently route
        // as SSH otherwise (A-H3 / S-M6).
        for host in &explicit {
            reject_unsupported_protocol(host)?;
        }

        // Step 2: SSH auto-discovery (additive, explicit wins on name conflict).
        let ssh_hosts = self.load_from_ssh_config();
        let hosts = merge_hosts(explicit, ssh_hosts);

        // Step 3: Ensure the built-in `local` host is always present.
        let hosts = ensure_local(hosts);

        *snapshot = Some(HostSnapshot {
            revision,
            hosts: hosts.clone(),
        });
        Ok(hosts)
    }
}

impl FileHostRepository {
    fn source_revision(&self) -> Vec<(PathBuf, Option<(u64, u128)>)> {
        let mut paths = self.config_file_paths.clone();
        if let Some(ssh_config) = &self.ssh_config_path {
            paths.extend(ssh_config_dependency_paths(ssh_config));
        }

        let mut seen = HashSet::new();
        paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .map(|path| {
                let revision = std::fs::metadata(&path).ok().map(|metadata| {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_nanos())
                        .unwrap_or(0);
                    (metadata.len(), modified)
                });
                (path, revision)
            })
            .collect()
    }
}

/// Return the complete dependency set for an SSH config, including files and
/// directories referenced by recursive Include directives. Directory entries
/// make wildcard includes refresh when a new matching file appears.
fn ssh_config_dependency_paths(root: &Path) -> Vec<PathBuf> {
    fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }

    fn collect(path: &Path, visited: &mut HashSet<PathBuf>, paths: &mut Vec<PathBuf>) {
        let identity = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if !visited.insert(identity) {
            return;
        }
        push_unique(paths, path.to_path_buf());

        let Ok(contents) = std::fs::read_to_string(path) else {
            return;
        };
        for raw_line in contents.lines() {
            let line = raw_line.split('#').next().unwrap_or_default().trim();
            let mut fields = line.split_whitespace();
            if !fields
                .next()
                .is_some_and(|field| field.eq_ignore_ascii_case("include"))
            {
                continue;
            }
            for raw_pattern in fields {
                let pattern = resolve_include_pattern(path, raw_pattern.trim_matches('"'));
                let has_wildcard = pattern
                    .to_string_lossy()
                    .chars()
                    .any(|ch| matches!(ch, '*' | '?' | '['));
                if has_wildcard {
                    if let Some(directory) = include_watch_directory(&pattern) {
                        push_unique(paths, directory);
                    }
                    if let Ok(matches) = glob::glob(&pattern.to_string_lossy()) {
                        for matched in matches.flatten() {
                            collect(&matched, visited, paths);
                        }
                    }
                } else {
                    collect(&pattern, visited, paths);
                }
            }
        }
    }

    let mut visited = HashSet::new();
    let mut paths = Vec::new();
    collect(root, &mut visited, &mut paths);
    paths
}

fn resolve_include_pattern(config_path: &Path, pattern: &str) -> PathBuf {
    let expanded = if let Some(relative) = pattern.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(relative)
    } else {
        PathBuf::from(pattern)
    };
    if expanded.is_absolute() {
        expanded
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(expanded)
    }
}

fn include_watch_directory(pattern: &Path) -> Option<PathBuf> {
    let mut prefix = PathBuf::new();
    for component in pattern.components() {
        let text = component.as_os_str().to_string_lossy();
        if text.chars().any(|ch| matches!(ch, '*' | '?' | '[')) {
            break;
        }
        prefix.push(component.as_os_str());
    }
    (!prefix.as_os_str().is_empty()).then_some(prefix)
}

/// Expand SSH `Include` directives in place while retaining their declaration
/// order. Relative patterns are resolved against the declaring file, wildcard
/// matches are processed lexically, and active-path tracking prevents cycles.
fn expand_ssh_config_includes(root: &Path) -> Result<String> {
    fn expand(path: &Path, active: &mut HashSet<PathBuf>, output: &mut String) -> Result<()> {
        let identity = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if !active.insert(identity.clone()) {
            tracing::warn!(path = %path.display(), "skipping cyclic SSH Include");
            return Ok(());
        }

        let contents = std::fs::read_to_string(path)?;
        for raw_line in contents.lines() {
            let line = raw_line.split('#').next().unwrap_or_default().trim();
            let mut fields = line.split_whitespace();
            if !fields
                .next()
                .is_some_and(|field| field.eq_ignore_ascii_case("include"))
            {
                output.push_str(raw_line);
                output.push('\n');
                continue;
            }

            let patterns: Vec<_> = fields.collect();
            if patterns.is_empty() {
                output.push_str(raw_line);
                output.push('\n');
                continue;
            }

            for raw_pattern in patterns {
                let pattern = resolve_include_pattern(path, raw_pattern.trim_matches('\"'));
                let has_wildcard = pattern
                    .to_string_lossy()
                    .chars()
                    .any(|ch| matches!(ch, '*' | '?' | '['));
                if has_wildcard {
                    let mut matches = glob::glob(pattern.to_string_lossy().as_ref())?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    matches.sort();
                    for matched in matches {
                        if matched.is_file() {
                            expand(&matched, active, output)?;
                        }
                    }
                } else if pattern.is_file() {
                    expand(&pattern, active, output)?;
                }
            }
        }

        active.remove(&identity);
        Ok(())
    }

    let mut active = HashSet::new();
    let mut output = String::new();
    expand(root, &mut active, &mut output)?;
    Ok(output)
}

// ---------------------------------------------------------------------------
// SSH config parsing helper
// ---------------------------------------------------------------------------

/// Parse an SSH config file and return `HostConfig` entries.
///
/// `ssh2-config` resolves every relative `Include` against `$HOME/.ssh`, even
/// when the root config lives elsewhere. Expand includes first so each relative
/// pattern is resolved against the file that declared it, matching OpenSSH.
/// The expanded text is then parsed with permissive rules so real-world config
/// directives that this crate does not model do not abort discovery.
pub fn load_ssh_config_file(path: &Path) -> Result<Vec<HostConfig>> {
    let expanded = expand_ssh_config_includes(path)?;
    let mut reader = BufReader::new(expanded.as_bytes());
    let config = SshConfig::default().parse(
        &mut reader,
        ParseRule::ALLOW_UNKNOWN_FIELDS | ParseRule::ALLOW_UNSUPPORTED_FIELDS,
    )?;

    let mut hosts: Vec<HostConfig> = Vec::new();

    for host in config.get_hosts() {
        // A Host block can have multiple patterns (e.g. `Host a b *.x`).
        // We only emit entries for non-wildcard, non-skipped concrete aliases.
        for clause in &host.pattern {
            // Skip negated clauses (they exclude, not include).
            if clause.negated {
                continue;
            }

            let alias = &clause.pattern;

            // Skip wildcard patterns — they match many hosts and don't represent
            // a single connectable endpoint.
            if alias.contains('*') || alias.contains('?') {
                continue;
            }

            // Skip well-known non-infrastructure services.
            if SKIP_SSH_HOSTS.contains(&alias.as_str()) {
                continue;
            }

            // Resolve per-host params (inherits Host * globals automatically).
            let params = config.query(alias.as_str());

            // `HostName` is required for a connectable host.  Skip alias-only
            // stanzas that lack a HostName directive.
            let hostname = match params.host_name {
                Some(ref h) => h.clone(),
                None => continue,
            };

            let port = params.port;
            let ssh_user = params.user.clone();
            let ssh_key_path = params
                .identity_file
                .as_ref()
                .and_then(|files| files.first())
                .map(|p| p.to_string_lossy().into_owned());

            hosts.push(HostConfig {
                name: alias.clone(),
                host: hostname,
                port,
                protocol: HostProtocol::Ssh,
                ssh_user,
                ssh_key_path,
                ssh_port: port,
                ssh_config_path: Some(path.to_string_lossy().into_owned()),
                docker_socket_path: None,
                tags: Vec::new(),
                compose_search_paths: Vec::new(),
                scout_read_roots: Vec::new(),
                exec_allowlist: Vec::new(),
            });
        }
    }

    // Deduplicate by name, first-seen wins (SSH first-match-wins semantics).
    let mut seen: HashSet<String> = HashSet::new();
    let deduped: Vec<HostConfig> = hosts
        .into_iter()
        .filter(|h| seen.insert(h.name.clone()))
        .collect();

    Ok(deduped)
}

// ---------------------------------------------------------------------------
// Merge helpers
// ---------------------------------------------------------------------------

/// Merge explicit and SSH-discovered hosts.
/// Explicit hosts take full precedence: SSH hosts with the same name are dropped.
pub fn merge_hosts(explicit: Vec<HostConfig>, ssh: Vec<HostConfig>) -> Vec<HostConfig> {
    let explicit_names: HashSet<String> = explicit.iter().map(|h| h.name.clone()).collect();

    let mut merged = explicit;
    for ssh_host in ssh {
        if !explicit_names.contains(&ssh_host.name) {
            merged.push(ssh_host);
        }
    }
    merged
}

/// Append the built-in `local` host if no host named `"local"` exists.
pub fn ensure_local(mut hosts: Vec<HostConfig>) -> Vec<HostConfig> {
    if !hosts.iter().any(|h| h.name == "local") {
        hosts.push(HostConfig::local());
    }
    hosts
}

// ---------------------------------------------------------------------------
// Protocol validation
// ---------------------------------------------------------------------------

/// Reject hosts whose protocol is `http` or `https`.
///
/// These variants exist in the `HostProtocol` enum but have never been
/// implemented. Accepting them silently causes them to be routed as SSH
/// (the else-branch in dispatch), which is a silent misconfiguration.
/// Fail loudly at load time instead (A-H3 / S-M6).
pub fn reject_unsupported_protocol(host: &HostConfig) -> Result<()> {
    match host.protocol {
        HostProtocol::Http | HostProtocol::Https => {
            anyhow::bail!(
                "host '{}': protocol '{}' is not supported; use 'local' or 'ssh'",
                host.name,
                match host.protocol {
                    HostProtocol::Http => "http",
                    HostProtocol::Https => "https",
                    _ => unreachable!(),
                }
            )
        }
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Default path resolution (separated for testability)
// ---------------------------------------------------------------------------

/// Ordered list of JSON config file paths for the production precedence chain.
pub fn default_config_paths() -> Vec<PathBuf> {
    if let Ok(path) = std::env::var("SYNAPSE_CONFIG_FILE") {
        return vec![PathBuf::from(path)];
    }
    let mut paths = vec![PathBuf::from("synapse.config.json")];
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        paths.push(Path::new(&xdg).join("synapse-mcp").join("config.json"));
    }
    if let Ok(home) = std::env::var("HOME") {
        paths.push(
            Path::new(&home)
                .join(".config")
                .join("synapse-mcp")
                .join("config.json"),
        );
        paths.push(Path::new(&home).join(".synapse-mcp.json"));
    }
    paths
}

/// Return `~/.ssh/config` path, or `None` if `HOME` is unset.
pub fn default_ssh_config_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| Path::new(&home).join(".ssh").join("config"))
}
