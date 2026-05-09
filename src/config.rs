use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const CONFIG_FILENAME: &str = ".release.conf";
const CHANGELOG_CANDIDATES: &[&str] = &["changelog.md", "CHANGELOG.md", "Changelog.md"];

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub changelog: PathBuf,
    pub branch: String,
    pub remote: String,
    pub pre_commit: Vec<String>,
}

#[derive(Debug, Default)]
struct RawConfig {
    changelog: Option<String>,
    branch: Option<String>,
    remote: Option<String>,
    pre_commit: Vec<String>,
}

impl Config {
    /// Load config from `<root>/.release.conf`, falling back to defaults if absent.
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILENAME);
        let raw = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            parse(&text).with_context(|| format!("parsing {}", path.display()))?
        } else {
            RawConfig::default()
        };

        let changelog = match raw.changelog {
            Some(name) => root.join(name),
            None => detect_changelog(root).with_context(|| {
                format!(
                    "no changelog found in {} (tried {:?})",
                    root.display(),
                    CHANGELOG_CANDIDATES
                )
            })?,
        };

        Ok(Self {
            root: root.to_path_buf(),
            changelog,
            branch: raw.branch.unwrap_or_else(|| "master".to_string()),
            remote: raw.remote.unwrap_or_else(|| "origin".to_string()),
            pre_commit: raw.pre_commit,
        })
    }
}

/// Parse a `key = value` config file.
///
/// Format:
///   - One assignment per line; blank lines and `#` comments are ignored.
///   - Values are not quoted; everything after the first `=` (trimmed) is the value.
///   - The `pre_commit` key may repeat; each occurrence appends one command.
///     Commands are passed to `sh -c`, so chain with `&&` or `;` for compound steps.
fn parse(text: &str) -> Result<RawConfig> {
    let mut raw = RawConfig::default();
    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (key, value) = match trimmed.split_once('=') {
            Some(kv) => kv,
            None => bail!("line {lineno}: expected `key = value`, got: {trimmed}"),
        };
        let key = key.trim();
        let value = value.trim().to_string();
        match key {
            "changelog" => raw.changelog = Some(value),
            "branch" => raw.branch = Some(value),
            "remote" => raw.remote = Some(value),
            "pre_commit" => raw.pre_commit.push(value),
            _ => bail!("line {lineno}: unknown key `{key}`"),
        }
    }
    Ok(raw)
}

fn detect_changelog(root: &Path) -> Option<PathBuf> {
    CHANGELOG_CANDIDATES
        .iter()
        .map(|n| root.join(n))
        .find(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "relog-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn defaults_when_no_config_file() {
        let dir = tmp_dir();
        fs::write(dir.join("changelog.md"), "# x").unwrap();
        let cfg = Config::load(&dir).unwrap();
        assert_eq!(cfg.branch, "master");
        assert_eq!(cfg.remote, "origin");
        assert_eq!(cfg.changelog, dir.join("changelog.md"));
        assert!(cfg.pre_commit.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_file_overrides_defaults() {
        let dir = tmp_dir();
        fs::write(dir.join("CHANGELOG.md"), "# x").unwrap();
        fs::write(
            dir.join(".release.conf"),
            "\
# example config
changelog = CHANGELOG.md
branch = main
remote = upstream

pre_commit = ./scripts/update-readme.sh
",
        )
        .unwrap();
        let cfg = Config::load(&dir).unwrap();
        assert_eq!(cfg.branch, "main");
        assert_eq!(cfg.remote, "upstream");
        assert_eq!(cfg.changelog, dir.join("CHANGELOG.md"));
        assert_eq!(cfg.pre_commit, vec!["./scripts/update-readme.sh"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repeated_hook_keys_accumulate_in_order() {
        let dir = tmp_dir();
        fs::write(dir.join("changelog.md"), "# x").unwrap();
        fs::write(
            dir.join(".release.conf"),
            "\
pre_commit = first
pre_commit = second
pre_commit = third
",
        )
        .unwrap();
        let cfg = Config::load(&dir).unwrap();
        assert_eq!(cfg.pre_commit, vec!["first", "second", "third"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_key_is_a_parse_error() {
        let dir = tmp_dir();
        fs::write(dir.join("changelog.md"), "# x").unwrap();
        fs::write(dir.join(".release.conf"), "wat = 1\n").unwrap();
        let err = Config::load(&dir).unwrap_err();
        assert!(err.to_string().contains("parsing") || format!("{err:#}").contains("unknown key"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn line_without_equals_is_a_parse_error() {
        let dir = tmp_dir();
        fs::write(dir.join("changelog.md"), "# x").unwrap();
        fs::write(dir.join(".release.conf"), "branch main\n").unwrap();
        let err = Config::load(&dir).unwrap_err();
        assert!(format!("{err:#}").contains("expected `key = value`"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn value_with_equals_keeps_everything_after_the_first() {
        let dir = tmp_dir();
        fs::write(dir.join("changelog.md"), "# x").unwrap();
        fs::write(
            dir.join(".release.conf"),
            "pre_commit = sh -c 'git config user.email=ci@example.com && git add x'\n",
        )
        .unwrap();
        let cfg = Config::load(&dir).unwrap();
        assert_eq!(
            cfg.pre_commit,
            vec!["sh -c 'git config user.email=ci@example.com && git add x'"]
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn errors_when_no_changelog_anywhere() {
        let dir = tmp_dir();
        let err = Config::load(&dir).unwrap_err();
        assert!(err.to_string().contains("no changelog"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detects_changelog_when_only_uppercase_present() {
        let dir = tmp_dir();
        fs::write(dir.join("CHANGELOG.md"), "# x").unwrap();
        let cfg = Config::load(&dir).unwrap();
        // On case-insensitive filesystems any of the candidate paths resolves to the
        // same file; what we care about is that detection succeeded and points at a
        // real file in the right directory.
        assert_eq!(cfg.changelog.parent(), Some(dir.as_path()));
        assert!(cfg.changelog.exists());
        fs::remove_dir_all(&dir).ok();
    }
}
