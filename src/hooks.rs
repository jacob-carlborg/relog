use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::changelog::Version;

#[derive(Debug, Clone)]
pub struct ReleaseEnv {
    pub version: Version,
    pub prev_version: Version,
    pub tag: String,
}

impl ReleaseEnv {
    fn apply(&self, cmd: &mut Command) {
        cmd.env("RELEASE_VERSION", self.version.to_string());
        cmd.env("RELEASE_PREV_VERSION", self.prev_version.to_string());
        cmd.env("RELEASE_TAG", &self.tag);
        cmd.env("RELEASE_MAJOR", self.version.major().to_string());
    }
}

/// Run pre_commit hooks sequentially. Each command is passed to `sh -c`,
/// inheriting stdio so its output is visible. Fails on the first non-zero exit.
pub fn run(commands: &[String], workdir: &Path, env: &ReleaseEnv) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }
    eprintln!("Running pre_commit hooks ({})", commands.len());
    for command in commands {
        eprintln!("  $ {command}");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(workdir);
        env.apply(&mut cmd);
        let status = cmd
            .status()
            .with_context(|| format!("starting pre_commit hook: {command}"))?;
        if !status.success() {
            bail!(
                "pre_commit hook failed (exit code {:?}): {command}",
                status.code()
            );
        }
    }
    Ok(())
}

/// Render hook commands as they would be executed in dry-run mode (just for display).
pub fn describe(commands: &[String]) {
    if commands.is_empty() {
        return;
    }
    eprintln!("[dry-run] pre_commit hooks:");
    for command in commands {
        eprintln!("  $ {command}");
    }
}
