use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

/// Thin wrapper around the `git` CLI scoped to a single working tree.
///
/// All operations shell out to `git`; this avoids the libgit2 build dependency and
/// keeps the binary statically linkable. We also avoid building anything on top of
/// the lower-level git plumbing — when something is genuinely a `git` command, we
/// just run that command.
pub struct Git {
    workdir: PathBuf,
}

impl Git {
    pub fn new(workdir: impl Into<PathBuf>) -> Self {
        Self { workdir: workdir.into() }
    }

    /// Discover the repository root containing `start` (walks up via `git rev-parse --show-toplevel`).
    pub fn discover(start: &Path) -> Result<PathBuf> {
        let out = Command::new("git")
            .arg("-C")
            .arg(start)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .context("running `git rev-parse --show-toplevel` (is git installed?)")?;
        check_status(&out, "git rev-parse --show-toplevel")?;
        let path = String::from_utf8(out.stdout)
            .context("non-utf8 output from git rev-parse")?
            .trim()
            .to_string();
        Ok(PathBuf::from(path))
    }

    pub fn current_branch(&self) -> Result<String> {
        let out = self.run(["branch", "--show-current"])?;
        Ok(String::from_utf8(out.stdout)?.trim().to_string())
    }

    /// True if a local branch with this name exists.
    pub fn branch_exists(&self, name: &str) -> Result<bool> {
        let mut cmd = self.command();
        cmd.args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ]);
        let out = cmd
            .output()
            .with_context(|| format!("running `git rev-parse refs/heads/{name}`"))?;
        Ok(out.status.success())
    }

    /// Pick a default branch when one isn't configured: prefer `main`, fall back to `master`.
    pub fn detect_default_branch(&self) -> Result<String> {
        if self.branch_exists("main")? {
            Ok("main".to_string())
        } else if self.branch_exists("master")? {
            Ok("master".to_string())
        } else {
            bail!(
                "no `branch` set in release.conf and could not find a `main` or `master` branch"
            )
        }
    }

    pub fn is_clean_working_tree(&self) -> Result<bool> {
        // Only flag modifications to tracked files; untracked files don't block a release.
        let out = self.run(["status", "--porcelain", "--untracked-files=no"])?;
        Ok(out.stdout.is_empty())
    }

    pub fn add(&self, paths: &[&Path]) -> Result<()> {
        let mut cmd = self.command();
        cmd.arg("add").arg("--");
        for p in paths {
            cmd.arg(p);
        }
        run_inheriting(cmd, "git add")
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        let mut cmd = self.command();
        cmd.args(["commit", "-m", message]);
        run_inheriting(cmd, "git commit")
    }

    pub fn tag_annotated(&self, name: &str, message: &str) -> Result<()> {
        let mut cmd = self.command();
        cmd.args(["tag", "-a", name, "-m", message]);
        run_inheriting(cmd, "git tag")
    }

    pub fn push(&self, remote: &str, refname: &str) -> Result<()> {
        let mut cmd = self.command();
        cmd.args(["push", remote, refname]);
        run_inheriting(cmd, "git push")
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.workdir);
        cmd
    }

    fn run<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = self.command();
        cmd.args(args);
        let argv = format!("{cmd:?}");
        let out = cmd.output().with_context(|| format!("running {argv}"))?;
        check_status(&out, &argv)?;
        Ok(out)
    }
}

fn run_inheriting(mut cmd: Command, label: &str) -> Result<()> {
    let status = cmd.status().with_context(|| format!("running {label}"))?;
    if !status.success() {
        bail!("{label} failed (exit code {:?})", status.code());
    }
    Ok(())
}

fn check_status(out: &Output, argv: &str) -> Result<()> {
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("{argv} failed (exit code {:?}): {}", out.status.code(), stderr.trim());
    }
    Ok(())
}
