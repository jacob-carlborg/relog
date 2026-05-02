# kacr — Keep A Changelog Release

A small CLI that cuts a release driven by your `[Unreleased]` section in a
[Keep a Changelog](https://keepachangelog.com/) formatted file. Project- and
language-agnostic — it works for any project with a KaC-style changelog and a
GitHub-style remote.

Given a clean working tree, `kacr`:

1. Detects the bump type (major/minor/patch) from the subheaders under `[Unreleased]`.
2. Rewrites the changelog: inserts a `## [X.Y.Z] - YYYY-MM-DD` header, updates
   the `[Unreleased]` reference link, adds a new compare link.
3. Commits the changelog and creates an annotated git tag.
4. Runs configurable hooks at three phases (`pre_commit`, `post_tag`, `pre_push`).
5. Prompts before pushing to the remote.

## Install

Download a static binary for your platform from the
[releases page](https://github.com/OWNER/kacr/releases) and put it on your `$PATH`.

Or build from source (requires Rust 1.85+):

```
cargo install --git https://github.com/OWNER/kacr
```

## Usage

```
kacr [VERSION] [--dry-run] [--chdir DIR]
```

Auto-detect the bump from the changelog:

```
$ kacr
Releasing 0.13.0 (previous: 0.12.0)
  tag:  v0.13.0
  date: 2026-05-02
...
Push master and tag to origin? [y/N]
```

Or pass an explicit version:

```
$ kacr 1.0.0
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

If the `[Unreleased]` section is empty, `kacr` refuses to release.

## Configuration: `.release.toml`

Optional file at the repository root. Every field has a sensible default:

```toml
changelog = "changelog.md"   # falls back to CHANGELOG.md if not set
branch    = "master"
remote    = "origin"

[hooks]
pre_commit = []   # before staging the changelog and committing
post_tag   = []   # after the annotated tag is created
pre_push   = []   # before pushing branch + tag to the remote
```

The repository URL used in the new compare link is taken from the existing
`[Unreleased]: ...` reference in the changelog, so any forge (GitHub, GitLab,
Codeberg, sourcehut, …) works without configuration.

## Hooks

Each hook is a shell command run via `sh -c` in the repository root. These
environment variables are exported to every hook:

| Variable | Example |
|---|---|
| `RELEASE_VERSION` | `1.2.3` |
| `RELEASE_PREV_VERSION` | `1.1.0` |
| `RELEASE_TAG` | `v1.2.3` |
| `RELEASE_MAJOR` | `1` |

A hook that exits non-zero aborts the release.

### Example: keep a major-version tag in sync (GitHub Actions style)

For projects where users reference `action@v1`, this `.release.toml` rewrites
`readme.md` and force-moves the `v1` tag on each release:

```toml
[hooks]
pre_commit = [
  'sed -i "" "s|action@v$RELEASE_PREV_VERSION|action@v$RELEASE_VERSION|g" readme.md',
  "git add readme.md",
]
post_tag = [
  'git tag -f -a v$RELEASE_MAJOR -m "Release $RELEASE_VERSION"',
]
pre_push = [
  "git push -f origin v$RELEASE_MAJOR",
]
```

## Source layout

Parsing and decision logic are kept separate from side effects:

| File | Responsibility |
|---|---|
| `src/changelog.rs` | Read-only KaC parser + pure rewriter |
| `src/bump.rs` | Pure bump detection |
| `src/config.rs` | `.release.toml` loader |
| `src/git.rs` | Thin wrapper around the `git` CLI |
| `src/hooks.rs` | Phase-based hook runner |
| `src/release.rs` | Orchestrator |
| `src/cli.rs` | clap-based CLI |

Run the tests:

```
cargo test
```

## License

MIT.
