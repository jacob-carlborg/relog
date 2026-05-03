use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

const CONFIG_FILENAME: &str = ".release.toml";
const CHANGELOG_CANDIDATES: &[&str] = &["changelog.md", "CHANGELOG.md", "Changelog.md"];

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub changelog: PathBuf,
    pub branch: String,
    pub remote: String,
    pub hooks: Hooks,
}

#[derive(Debug, Clone, Default)]
pub struct Hooks {
    pub pre_commit: Vec<String>,
    pub post_tag: Vec<String>,
    pub pre_push: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    changelog: Option<String>,
    branch: Option<String>,
    remote: Option<String>,
    #[serde(default)]
    hooks: RawHooks,
}

#[derive(Debug, Default, Deserialize)]
struct RawHooks {
    #[serde(default)]
    pre_commit: Vec<String>,
    #[serde(default)]
    post_tag: Vec<String>,
    #[serde(default)]
    pre_push: Vec<String>,
}

impl Config {
    /// Load config from `<root>/.release.toml`, falling back to defaults if absent.
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILENAME);
        let raw = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            toml::from_str::<RawConfig>(&text)
                .with_context(|| format!("parsing {}", path.display()))?
        } else {
            RawConfig::default()
        };

        let changelog = match raw.changelog {
            Some(name) => root.join(name),
            None => detect_changelog(root)
                .with_context(|| format!("no changelog found in {} (tried {:?})", root.display(), CHANGELOG_CANDIDATES))?,
        };

        Ok(Self {
            root: root.to_path_buf(),
            changelog,
            branch: raw.branch.unwrap_or_else(|| "master".to_string()),
            remote: raw.remote.unwrap_or_else(|| "origin".to_string()),
            hooks: Hooks {
                pre_commit: raw.hooks.pre_commit,
                post_tag: raw.hooks.post_tag,
                pre_push: raw.hooks.pre_push,
            },
        })
    }
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
        assert!(cfg.hooks.pre_commit.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_file_overrides_defaults() {
        let dir = tmp_dir();
        fs::write(dir.join("CHANGELOG.md"), "# x").unwrap();
        fs::write(
            dir.join(".release.toml"),
            r#"
changelog = "CHANGELOG.md"
branch = "main"
remote = "upstream"

[hooks]
pre_commit = ["./scripts/update-readme.sh"]
post_tag = ["git tag -f v$RELEASE_MAJOR"]
"#,
        )
        .unwrap();
        let cfg = Config::load(&dir).unwrap();
        assert_eq!(cfg.branch, "main");
        assert_eq!(cfg.remote, "upstream");
        assert_eq!(cfg.changelog, dir.join("CHANGELOG.md"));
        assert_eq!(cfg.hooks.pre_commit, vec!["./scripts/update-readme.sh"]);
        assert_eq!(cfg.hooks.post_tag, vec!["git tag -f v$RELEASE_MAJOR"]);
        assert!(cfg.hooks.pre_push.is_empty());
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
