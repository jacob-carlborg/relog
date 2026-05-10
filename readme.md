# relog

A small CLI that cuts a release driven by your `[Unreleased]` section in a
[Keep a Changelog](https://keepachangelog.com/) formatted file. Project- and
language-agnostic — it works for any project with a KaC-style changelog and a
GitHub-style remote.

The name is short for *release log* — and `relog` literally re-logs the
`[Unreleased]` work as a versioned entry.

Given a clean working tree, `relog`:

1. Detects the bump type (major/minor/patch) from the subheaders under `[Unreleased]`.
2. Rewrites the changelog: inserts a `## [X.Y.Z] - YYYY-MM-DD` header, updates
   the `[Unreleased]` reference link, adds a new compare link.
3. Commits the changelog and creates an annotated git tag.
4. Runs configurable `pre_commit` hooks before staging and committing.
5. Prompts before pushing to the remote.

## Install

Download a static binary for your platform from the
[releases page](https://github.com/OWNER/relog/releases) and put it on your `$PATH`.

Or build from source (requires Rust 1.85+):

```
cargo install --git https://github.com/OWNER/relog
```

## Usage

```
relog [VERSION] [--dry-run] [--chdir DIR]
```

Auto-detect the bump from the changelog:

```
$ relog
Releasing 0.13.0 (previous: 0.12.0)
  tag:  v0.13.0
  date: 2026-05-03
...
Push master and tag to origin? [y/N]
```

Or pass an explicit version:

```
$ relog 1.0.0
```

Use `--dry-run` to preview without making changes.

## Bump detection

The bump kind is decided by the `###` subheaders under `## [Unreleased]`:

| Trigger | Bump |
|---|---|
| `### Removed` (or the word "Breaking" anywhere) | Major |
| `### Added`, `### Changed`, `### Deprecated` | Minor |
| `### Fixed` | Patch |

Higher-precedence rules win: a section with both `### Added` and `### Removed`
is a major bump.

If the `[Unreleased]` section is empty, `relog` refuses to release.

## Configuration: `release.conf`

Optional file at the repository root. Every field has a sensible default:

```ini
# falls back to changelog.md / CHANGELOG.md / Changelog.md if not set
changelog = changelog.md
# defaults to `main` if it exists, otherwise `master`
branch    = main
remote    = origin

# `pre_commit` may repeat; each occurrence is one command run before
# staging the changelog and committing.
# pre_commit = ...
```

Format: one `key = value` per line. Blank lines and `#` comments are ignored.
Values are not quoted — everything after the first `=` (trimmed) is the value.

The repository URL used in the new compare link is taken from the existing
`[Unreleased]: ...` reference in the changelog, so any forge (GitHub, GitLab,
Codeberg, sourcehut, …) works without configuration.

## Hooks

Each `pre_commit` hook is a shell command run via `sh -c` in the repository
root, after the changelog has been rewritten and before it is staged. These
environment variables are exported to every hook:

| Variable | Example |
|---|---|
| `RELEASE_VERSION` | `1.2.3` |
| `RELEASE_PREV_VERSION` | `1.1.0` |
| `RELEASE_TAG` | `v1.2.3` |
| `RELEASE_MAJOR` | `1` |

A hook that exits non-zero aborts the release. Anything the hook modifies
should be staged by the hook itself (`git add ...`) — only the changelog is
staged automatically.

### Example: rewrite a version reference in the README

```ini
pre_commit = sed -i "" "s|action@v$RELEASE_PREV_VERSION|action@v$RELEASE_VERSION|g" readme.md
pre_commit = git add readme.md
```

For multi-step commands, chain with `&&` — each hook is passed to `sh -c`.
Multi-line values aren't supported; for anything complex, put the steps in a
script file and reference it.

## Source layout

Parsing and decision logic are kept separate from side effects:

| File | Responsibility |
|---|---|
| `src/changelog.rs` | Read-only KaC parser + pure rewriter |
| `src/bump.rs` | Pure bump detection |
| `src/config.rs` | `release.conf` loader |
| `src/git.rs` | Thin wrapper around the `git` CLI |
| `src/hooks.rs` | Phase-based hook runner |
| `src/release.rs` | Orchestrator |
| `src/cli.rs` | Argument parser + entry point |

Run the tests:

```
cargo test
```

## License

MIT.
