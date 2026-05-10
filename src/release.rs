use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::bump;
use crate::changelog::{Changelog, ReleaseOpts, Version};
use crate::config::Config;
use crate::date::Date;
use crate::git::Git;
use crate::hooks::{self, ReleaseEnv};

pub struct Plan {
    pub config: Config,
    pub git: Git,
    pub changelog: Changelog,
    pub prev_version: Version,
    pub version: Version,
    pub tag: String,
    pub date: Date,
}

impl Plan {
    /// Build a release plan from the repository at `root`. Validates that:
    ///   - the changelog exists and has a current version
    ///   - the [Unreleased] section is non-empty (or `version` was passed explicitly)
    ///   - the version is greater than the previous one
    pub fn build(root: &Path, explicit_version: Option<Version>) -> Result<Self> {
        let git = Git::new(root);
        let config = Config::load(root, || git.detect_default_branch())?;

        let changelog_text = std::fs::read_to_string(&config.changelog)
            .with_context(|| format!("reading {}", config.changelog.display()))?;
        let changelog = Changelog::from_str(&changelog_text);

        let prev_version = changelog
            .current_version()
            .context("could not determine the current version from the changelog")?;

        let version = match explicit_version {
            Some(v) => v,
            None => {
                let section = changelog.unreleased_section();
                bump::next_version(prev_version, section)?
            }
        };

        if version <= prev_version {
            bail!("new version {} must be greater than previous {}", version, prev_version);
        }

        Ok(Self {
            tag: format!("v{version}"),
            date: Date::today_utc(),
            config,
            git,
            changelog,
            prev_version,
            version,
        })
    }

    pub fn release_env(&self) -> ReleaseEnv {
        ReleaseEnv {
            version: self.version,
            prev_version: self.prev_version,
            tag: self.tag.clone(),
        }
    }

    pub fn print_summary(&self) {
        println!(
            "Releasing {} (previous: {})",
            self.version, self.prev_version
        );
        println!("  tag:  {}", self.tag);
        println!("  date: {}", self.date);
        println!();
    }

    /// Print what would be done without making any changes.
    pub fn dry_run(&self) {
        println!("[dry-run] Would update {}:", self.config.changelog.display());
        println!("  - Add header: ## [{}] - {}", self.version, self.date);
        println!("  - Add reference link for {}", self.version);
        println!("  - Update [Unreleased] reference link");
        hooks::describe(&self.config.pre_commit);
        println!("[dry-run] Would commit: Release {}", self.version);
        println!("[dry-run] Would create annotated tag: {}", self.tag);
        println!("[dry-run] Would push {} and tag: {}", self.config.branch, self.tag);
    }

    /// Execute the release: validate branch + worktree, rewrite changelog, commit, tag,
    /// run pre_commit hooks, prompt to push.
    pub fn execute(&self) -> Result<()> {
        // --- Validate branch and working tree -------------------------------
        let current = self.git.current_branch()?;
        if current != self.config.branch {
            bail!(
                "must be on the {} branch to release (currently on {})",
                self.config.branch,
                current
            );
        }
        if !self.git.is_clean_working_tree()? {
            bail!("working tree is not clean — commit or stash your changes first");
        }

        let env = self.release_env();

        // --- Rewrite changelog ----------------------------------------------
        let opts = ReleaseOpts {
            version: self.version,
            prev_version: self.prev_version,
            date: self.date,
        };
        let new_text = self.changelog.release(&opts)?;
        std::fs::write(&self.config.changelog, &new_text)
            .with_context(|| format!("writing {}", self.config.changelog.display()))?;

        // --- pre_commit hooks (e.g. README rewrite) ------------------------
        hooks::run(&self.config.pre_commit, &self.config.root, &env)?;

        // --- Stage + commit + tag ------------------------------------------
        let changelog_rel = relative_to(&self.config.changelog, &self.config.root);
        self.git.add(&[&changelog_rel])?;
        self.git.commit(&format!("Release {}", self.version))?;
        self.git.tag_annotated(&self.tag, &format!("Release {}", self.version))?;

        println!();
        println!("Release {} committed and tagged ({}).", self.version, self.tag);
        println!();

        // --- Prompt before push --------------------------------------------
        let push = confirm_default_no(&format!(
            "Push {} and tag to {}?",
            self.config.branch, self.config.remote
        ));

        if !push {
            println!("Not pushed. When ready, run:");
            println!("  git push {} {}", self.config.remote, self.config.branch);
            println!("  git push {} {}", self.config.remote, self.tag);
            return Ok(());
        }

        // --- Push -----------------------------------------------------------
        self.git.push(&self.config.remote, &self.config.branch)?;
        self.git.push(&self.config.remote, &self.tag)?;
        println!(
            "Pushed {} and tag {}.",
            self.config.branch, self.tag
        );

        Ok(())
    }
}

fn relative_to(path: &Path, base: &Path) -> std::path::PathBuf {
    path.strip_prefix(base)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Prompt the user with `<message> [y/N] ` and return true only if the first
/// non-whitespace character of the reply is `y` or `Y`. Empty input, EOF, or
/// I/O errors all return false.
fn confirm_default_no(message: &str) -> bool {
    print!("{message} [y/N] ");
    if std::io::stdout().flush().is_err() {
        return false;
    }

    let mut line = String::new();
    let stdin = std::io::stdin();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }

    matches!(line.trim_start().chars().next(), Some('y') | Some('Y'))
}
