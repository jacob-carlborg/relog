use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::changelog::Version;
use crate::git::Git;
use crate::release::Plan;

const USAGE: &str = "\
Cut releases from a Keep a Changelog `[Unreleased]` section

Usage: relog [VERSION] [OPTIONS]

Arguments:
  [VERSION]  Explicit version to release (X.Y.Z). If omitted, the bump is
             detected from the [Unreleased] section of the changelog.

Options:
      --dry-run      Show what would happen without making any changes
      --chdir <DIR>  Override the working directory (defaults to the repo root)
  -h, --help         Print help
  -V, --version      Print version
";

#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    version: Option<String>,
    dry_run: bool,
    chdir: Option<PathBuf>,
}

#[derive(Debug)]
enum Action {
    Help,
    PrintVersion,
    Run(Args),
}

pub fn run() -> Result<()> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    match parse(raw)? {
        Action::Help => {
            print!("{USAGE}");
            Ok(())
        }
        Action::PrintVersion => {
            println!(
                "{} {}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        Action::Run(args) => run_release(args),
    }
}

fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Action> {
    let mut iter = args.into_iter();
    let mut out = Args::default();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Action::Help),
            "-V" | "--version" => return Ok(Action::PrintVersion),
            "--dry-run" => out.dry_run = true,
            "--chdir" => {
                let value = match iter.next() {
                    Some(v) => v,
                    None => bail!("--chdir requires a value"),
                };
                out.chdir = Some(PathBuf::from(value));
            }
            s if s.starts_with("--chdir=") => {
                let value = s.strip_prefix("--chdir=").unwrap();
                out.chdir = Some(PathBuf::from(value));
            }
            s if s.starts_with("--") => bail!("unknown option: {s}"),
            _ => {
                if out.version.is_some() {
                    bail!("unexpected argument: {arg}");
                }
                out.version = Some(arg);
            }
        }
    }
    Ok(Action::Run(out))
}

fn run_release(cli: Args) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn parse_run(items: &[&str]) -> Args {
        match parse(args(items)).unwrap() {
            Action::Run(a) => a,
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn no_args_yields_defaults() {
        let a = parse_run(&[]);
        assert_eq!(a, Args::default());
    }

    #[test]
    fn positional_version_is_captured() {
        let a = parse_run(&["1.2.3"]);
        assert_eq!(a.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn dry_run_flag() {
        let a = parse_run(&["--dry-run"]);
        assert!(a.dry_run);
    }

    #[test]
    fn chdir_with_separate_value() {
        let a = parse_run(&["--chdir", "/tmp/x"]);
        assert_eq!(a.chdir, Some(PathBuf::from("/tmp/x")));
    }

    #[test]
    fn chdir_with_equals() {
        let a = parse_run(&["--chdir=/tmp/x"]);
        assert_eq!(a.chdir, Some(PathBuf::from("/tmp/x")));
    }

    #[test]
    fn flags_can_appear_before_or_after_positional() {
        let a = parse_run(&["1.2.3", "--dry-run"]);
        assert_eq!(a.version.as_deref(), Some("1.2.3"));
        assert!(a.dry_run);
        let b = parse_run(&["--dry-run", "1.2.3"]);
        assert_eq!(b.version.as_deref(), Some("1.2.3"));
        assert!(b.dry_run);
    }

    #[test]
    fn help_short_and_long() {
        assert!(matches!(parse(args(&["-h"])).unwrap(), Action::Help));
        assert!(matches!(parse(args(&["--help"])).unwrap(), Action::Help));
    }

    #[test]
    fn version_short_and_long() {
        assert!(matches!(parse(args(&["-V"])).unwrap(), Action::PrintVersion));
        assert!(matches!(parse(args(&["--version"])).unwrap(), Action::PrintVersion));
    }

    #[test]
    fn unknown_option_errors() {
        let err = parse(args(&["--nope"])).unwrap_err();
        assert!(err.to_string().contains("unknown option"));
    }

    #[test]
    fn chdir_without_value_errors() {
        let err = parse(args(&["--chdir"])).unwrap_err();
        assert!(err.to_string().contains("--chdir requires a value"));
    }

    #[test]
    fn second_positional_errors() {
        let err = parse(args(&["1.2.3", "4.5.6"])).unwrap_err();
        assert!(err.to_string().contains("unexpected argument"));
    }
}
