use std::fmt;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow, bail};
use chrono::NaiveDate;
use regex::Regex;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Version(pub u32, pub u32, pub u32);

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

impl Version {
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            bail!("invalid version format: {s} (expected X.Y.Z)");
        }
        let parse = |p: &str| p.parse::<u32>().map_err(|_| anyhow!("invalid version component in {s}: {p}"));
        Ok(Version(parse(parts[0])?, parse(parts[1])?, parse(parts[2])?))
    }

    pub fn major(self) -> u32 { self.0 }
}

pub struct Changelog {
    contents: String,
}

pub struct ReleaseOpts {
    pub version: Version,
    pub prev_version: Version,
    pub date: NaiveDate,
}

impl Changelog {
    pub fn from_str(s: &str) -> Self {
        Self { contents: s.to_string() }
    }

    /// The body of the [Unreleased] section — everything between the
    /// `## [Unreleased]` line and the next `## [` line, both exclusive.
    pub fn unreleased_section(&self) -> &str {
        let start = match find_line(&self.contents, |l| l.trim() == "## [Unreleased]") {
            Some((_, end)) => end,
            None => return "",
        };
        let after = &self.contents[start..];
        let next = match find_line(after, |l| l.starts_with("## [") && l.trim() != "## [Unreleased]") {
            Some((s, _)) => s,
            None => after.len(),
        };
        &after[..next]
    }

    /// The most recent released version (the first `## [X.Y.Z]` header in the file).
    pub fn current_version(&self) -> Option<Version> {
        let re = version_header_regex();
        let caps = re.captures(&self.contents)?;
        Version::parse(caps.get(1)?.as_str()).ok()
    }

    /// Base URL for compare links, derived from the existing `[Unreleased]: ...` reference link.
    /// E.g. "https://github.com/cross-platform-actions/openbsd-builder".
    pub fn repo_url(&self) -> Option<String> {
        let re = unreleased_ref_regex();
        let caps = re.captures(&self.contents)?;
        Some(caps.get(1)?.as_str().to_string())
    }

    /// Produce the rewritten changelog for a new release.
    pub fn release(&self, opts: &ReleaseOpts) -> Result<String> {
        if opts.version <= opts.prev_version {
            bail!("new version {} must be greater than previous {}", opts.version, opts.prev_version);
        }

        let base_url = self
            .repo_url()
            .context("could not find an [Unreleased]: reference link to derive the repo URL from")?;

        let new_header = format!(
            "## [Unreleased]\n\n## [{ver}] - {date}",
            ver = opts.version,
            date = opts.date,
        );
        let unreleased_re = Regex::new(r"(?m)^## \[Unreleased\]\s*$").unwrap();
        let after_header = unreleased_re
            .replace(&self.contents, new_header.as_str())
            .into_owned();
        if after_header == self.contents {
            bail!("could not locate the `## [Unreleased]` header in the changelog");
        }

        let new_unreleased_ref = format!("[Unreleased]: {base_url}/compare/v{ver}...HEAD", ver = opts.version);
        let unreleased_ref_re = unreleased_ref_regex();
        let after_unreleased_ref = unreleased_ref_re
            .replace(&after_header, new_unreleased_ref.as_str())
            .into_owned();
        if after_unreleased_ref == after_header {
            bail!("could not locate the `[Unreleased]: ...` reference link in the changelog");
        }

        let new_version_ref = format!(
            "\n\n[{ver}]: {base_url}/compare/v{prev}...v{ver}",
            ver = opts.version,
            prev = opts.prev_version,
        );
        // Insert the new version reference link after the [Unreleased] reference line.
        let unreleased_ref_line_re =
            Regex::new(r"(?m)^\[Unreleased\]:\s*[^\r\n]+$").unwrap();
        let final_contents = unreleased_ref_line_re
            .replace(&after_unreleased_ref, |caps: &regex::Captures<'_>| {
                let mut s = caps[0].to_string();
                s.push_str(&new_version_ref);
                s
            })
            .into_owned();

        Ok(final_contents)
    }
}

fn version_header_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^## \[(\d+\.\d+\.\d+)\]").unwrap())
}

fn unreleased_ref_regex() -> &'static Regex {
    // Captures the base URL (everything before "/compare/")
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?m)^\[Unreleased\]:\s*(\S+?)/compare/v\d+\.\d+\.\d+\.\.\.HEAD\s*$").unwrap()
    })
}

/// Find the byte range of the first line in `text` for which `pred(line)` is true.
/// Returns `(line_start, line_end_inclusive_of_newline)`.
fn find_line<F: Fn(&str) -> bool>(text: &str, pred: F) -> Option<(usize, usize)> {
    let mut start = 0usize;
    for line in text.split_inclusive('\n') {
        let line_no_nl = line.strip_suffix('\n').unwrap_or(line);
        if pred(line_no_nl) {
            return Some((start, start + line.len()));
        }
        start += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Changelog

## [Unreleased]
### Added
- New thing

## [0.12.0] - 2026-04-29
### Changed
- Old thing

## [0.11.1] - 2025-12-13
### Fixed
- Bug

[Unreleased]: https://github.com/example/repo/compare/v0.12.0...HEAD

[0.12.0]: https://github.com/example/repo/compare/v0.11.1...v0.12.0

[0.11.1]: https://github.com/example/repo/compare/v0.11.0...v0.11.1
";

    const EMPTY_UNRELEASED: &str = "\
# Changelog

## [Unreleased]

## [0.12.0] - 2026-04-29
### Changed
- Old thing

[Unreleased]: https://github.com/example/repo/compare/v0.12.0...HEAD

[0.12.0]: https://github.com/example/repo/compare/v0.11.0...v0.12.0
";

    #[test]
    fn current_version_returns_first_release_header() {
        let cl = Changelog::from_str(SAMPLE);
        assert_eq!(cl.current_version(), Some(Version(0, 12, 0)));
    }

    #[test]
    fn current_version_none_when_no_releases() {
        let cl = Changelog::from_str("# Changelog\n\n## [Unreleased]\n");
        assert_eq!(cl.current_version(), None);
    }

    #[test]
    fn unreleased_section_includes_subheaders_and_entries() {
        let cl = Changelog::from_str(SAMPLE);
        let section = cl.unreleased_section();
        assert!(section.contains("### Added"), "section was: {section:?}");
        assert!(section.contains("New thing"));
        assert!(!section.contains("[0.12.0]"), "should stop before next version header");
    }

    #[test]
    fn unreleased_section_empty_when_section_is_empty() {
        let cl = Changelog::from_str(EMPTY_UNRELEASED);
        assert_eq!(cl.unreleased_section().trim(), "");
    }

    #[test]
    fn repo_url_extracted_from_unreleased_reference() {
        let cl = Changelog::from_str(SAMPLE);
        assert_eq!(cl.repo_url().as_deref(), Some("https://github.com/example/repo"));
    }

    #[test]
    fn release_inserts_version_header_and_reference_link() {
        let cl = Changelog::from_str(SAMPLE);
        let opts = ReleaseOpts {
            version: Version(0, 13, 0),
            prev_version: Version(0, 12, 0),
            date: NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        };
        let out = cl.release(&opts).unwrap();

        // Version header inserted after [Unreleased].
        assert!(out.contains("## [Unreleased]\n\n## [0.13.0] - 2026-05-02"));
        // Old version header still present.
        assert!(out.contains("## [0.12.0] - 2026-04-29"));
        // [Unreleased] reference link updated.
        assert!(out.contains("[Unreleased]: https://github.com/example/repo/compare/v0.13.0...HEAD"));
        // New compare link added.
        assert!(out.contains("[0.13.0]: https://github.com/example/repo/compare/v0.12.0...v0.13.0"));
        // Old [Unreleased] line is gone.
        assert!(!out.contains("v0.12.0...HEAD"));
        // Existing reference links preserved.
        assert!(out.contains("[0.12.0]: https://github.com/example/repo/compare/v0.11.1...v0.12.0"));
    }

    #[test]
    fn release_works_with_empty_unreleased_section() {
        let cl = Changelog::from_str(EMPTY_UNRELEASED);
        let opts = ReleaseOpts {
            version: Version(0, 13, 0),
            prev_version: Version(0, 12, 0),
            date: NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        };
        let out = cl.release(&opts).unwrap();
        assert!(out.contains("## [0.13.0] - 2026-05-02"));
        assert!(out.contains("[0.13.0]: https://github.com/example/repo/compare/v0.12.0...v0.13.0"));
    }

    #[test]
    fn release_rejects_version_not_greater_than_prev() {
        let cl = Changelog::from_str(SAMPLE);
        let opts = ReleaseOpts {
            version: Version(0, 12, 0),
            prev_version: Version(0, 12, 0),
            date: NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        };
        assert!(cl.release(&opts).is_err());
    }

    #[test]
    fn version_parse_rejects_invalid() {
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.x").is_err());
        assert_eq!(Version::parse("1.2.3").unwrap(), Version(1, 2, 3));
    }

    #[test]
    fn version_ordering_is_lexicographic_on_components() {
        assert!(Version(1, 0, 0) > Version(0, 99, 99));
        assert!(Version(0, 2, 0) > Version(0, 1, 99));
        assert!(Version(0, 0, 2) > Version(0, 0, 1));
    }
}
