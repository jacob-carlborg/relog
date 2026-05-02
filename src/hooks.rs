use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::changelog::Version;

#[derive(Debug, Clone, Copy)]
pub enum Phase {
    PreCommit,
    PostTag,
    PrePush,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::PreCommit => "pre_commit",
            Phase::PostTag => "post_tag",
            Phase::PrePush => "pre_push",
        }
    }
}

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

/// Run a list of hook commands sequentially. Each command is passed to `sh -c`,
/// inheriting stdio so its output is visible. Fails on the first non-zero exit.
pub fn run(phase: Phase, commands: &[String], workdir: &Path, env: &ReleaseEnv) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }
    eprintln!("Running {} hooks ({})", phase.label(), commands.len());
    for command in commands {
        eprintln!("  $ {command}");
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(workdir);
        env.apply(&mut cmd);
        let status = cmd
            .status()
            .with_context(|| format!("starting {} hook: {command}", phase.label()))?;
        if !status.success() {
            bail!(
                "{} hook failed (exit code {:?}): {command}",
                phase.label(),
                status.code()
            );
        }
    }
    Ok(())
}

/// Render a hook command as it would be executed in dry-run mode (just for display).
pub fn describe(phase: Phase, commands: &[String]) {
    if commands.is_empty() {
        return;
    }
    eprintln!("[dry-run] {} hooks:", phase.label());
    for command in commands {
        eprintln!("  $ {command}");
    }
}
