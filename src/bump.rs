use anyhow::{Result, bail};

use crate::changelog::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    Major,
    Minor,
    Patch,
}

/// Detect the bump kind from a Keep a Changelog `[Unreleased]` section body.
/// Returns `None` if there are no qualifying entries (caller should treat as error).
///
/// Mapping (matching the existing bash script's behavior):
///   ### Removed              -> Major
///   "Breaking" anywhere       -> Major
///   ### Added/Changed/Deprecated -> Minor
///   ### Fixed                -> Patch
pub fn detect(section: &str) -> Option<BumpKind> {
    if section.trim().is_empty() {
        return None;
    }

    let has_subheader = |name: &str| has_subheader(section, name);

    if has_subheader("Removed") {
        return Some(BumpKind::Major);
    }
    if section.to_ascii_lowercase().contains("breaking") {
        return Some(BumpKind::Major);
    }
    if has_subheader("Added") || has_subheader("Changed") || has_subheader("Deprecated") {
        return Some(BumpKind::Minor);
    }
    if has_subheader("Fixed") {
        return Some(BumpKind::Patch);
    }
    None
}

pub fn apply(prev: Version, kind: BumpKind) -> Version {
    match kind {
        BumpKind::Major => Version(prev.0 + 1, 0, 0),
        BumpKind::Minor => Version(prev.0, prev.1 + 1, 0),
        BumpKind::Patch => Version(prev.0, prev.1, prev.2 + 1),
    }
}

pub fn next_version(prev: Version, section: &str) -> Result<Version> {
    match detect(section) {
        Some(kind) => Ok(apply(prev, kind)),
        None => bail!("nothing to release — the [Unreleased] section is empty"),
    }
}

/// Whether `text` contains a Keep a Changelog subheader line of the form
/// `### <Name>` (case-insensitive, optional surrounding whitespace, one or
/// more spaces/tabs between `###` and the name).
fn has_subheader(text: &str, name: &str) -> bool {
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("###") else {
            continue;
        };
        // Require at least one whitespace character after `###`.
        if !rest.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        if rest.trim().eq_ignore_ascii_case(name) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_section_yields_none() {
        assert_eq!(detect(""), None);
        assert_eq!(detect("   \n  \n"), None);
    }

    #[test]
    fn removed_yields_major() {
        assert_eq!(detect("### Removed\n- gone"), Some(BumpKind::Major));
    }

    #[test]
    fn breaking_anywhere_yields_major() {
        assert_eq!(
            detect("### Changed\n- BREAKING: rewrite the API"),
            Some(BumpKind::Major)
        );
    }

    #[test]
    fn added_yields_minor() {
        assert_eq!(detect("### Added\n- thing"), Some(BumpKind::Minor));
    }

    #[test]
    fn changed_yields_minor() {
        assert_eq!(detect("### Changed\n- thing"), Some(BumpKind::Minor));
    }

    #[test]
    fn deprecated_yields_minor() {
        assert_eq!(detect("### Deprecated\n- thing"), Some(BumpKind::Minor));
    }

    #[test]
    fn fixed_only_yields_patch() {
        assert_eq!(detect("### Fixed\n- bug"), Some(BumpKind::Patch));
    }

    #[test]
    fn major_takes_precedence_over_minor() {
        let s = "### Added\n- new\n### Removed\n- gone";
        assert_eq!(detect(s), Some(BumpKind::Major));
    }

    #[test]
    fn minor_takes_precedence_over_patch() {
        let s = "### Added\n- new\n### Fixed\n- bug";
        assert_eq!(detect(s), Some(BumpKind::Minor));
    }

    #[test]
    fn apply_major_zeroes_minor_and_patch() {
        assert_eq!(apply(Version(1, 2, 3), BumpKind::Major), Version(2, 0, 0));
    }

    #[test]
    fn apply_minor_zeroes_patch() {
        assert_eq!(apply(Version(1, 2, 3), BumpKind::Minor), Version(1, 3, 0));
    }

    #[test]
    fn apply_patch_increments_patch() {
        assert_eq!(apply(Version(1, 2, 3), BumpKind::Patch), Version(1, 2, 4));
    }

    #[test]
    fn next_version_errors_on_empty_section() {
        let err = next_version(Version(1, 2, 3), "").unwrap_err();
        assert!(err.to_string().contains("nothing to release"));
    }
}
