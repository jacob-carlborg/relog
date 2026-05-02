use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::changelog::Version;
use crate::git::Git;
use crate::release::Plan;

#[derive(Debug, Parser)]
#[command(
    name = "kacr",
    version,
    about = "Keep A Changelog Release — automate changelog-driven releases",
    long_about = None,
)]
struct Cli {
    /// Explicit version to release (X.Y.Z). If omitted, the bump is detected from
    /// the [Unreleased] section of the changelog.
    version: Option<String>,

    /// Show what would happen without making any changes.
    #[arg(long)]
    dry_run: bool,

    /// Override the working directory (defaults to the repository root).
    #[arg(long, value_name = "DIR")]
    chdir: Option<PathBuf>,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    let start = match cli.chdir {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let root = Git::discover(&start)?;

    let explicit_version = cli.version.as_deref().map(Version::parse).transpose()?;
    let plan = Plan::build(&root, explicit_version)?;

    plan.print_summary();

    if cli.dry_run {
        plan.dry_run();
        return Ok(());
    }

    plan.execute()
}
