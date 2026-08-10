# Support policy

`jjc` is pre-1.0 and experimental. Support means that the named lane is tested
and regressions are accepted for triage; it is not a guarantee for every
terminal, filesystem, or conflict shape.

## Current compatibility matrix

| Surface | Current contract | Evidence tier |
| --- | --- | --- |
| Rust | 1.93.1 is the minimum and blocking toolchain | local and CI quality gates |
| `jj` | 0.44.0 is the blocking protocol baseline | real-`jj` smoke, tree, marker, and PTY tests |
| newer `jj` | probed weekly without expanding the support claim | advisory CI only |
| Linux | build, unit, real-`jj`, PTY, executable, and symlink behavior | Tier 1 |
| macOS | build, unit, real-`jj`, PTY, executable, and symlink behavior | Tier 1 |
| Windows | build, Clippy, portable tests, install, and binary startup | Tier 2 |

Tier 2 does not claim Windows ConPTY interaction or Unix filesystem semantics.
Unsupported merge and tree-entry shapes remain listed in the README's current
limits.

## Version maintenance

Until the first tag, `main` is the only maintained line. After releases begin,
the latest tagged pre-1.0 release and `main` receive fixes on a best-effort
basis. Older pre-1.0 versions are not routinely backported; a severe security
issue may justify an exception.

A version is end-of-life when a newer release is published unless release notes
say otherwise. Support and security claims must name the exact version or commit
and must not be inferred from a later green run.

## Updating a baseline

A Rust or `jj` baseline change must update every authoritative reference in the
same pull request: `rust-toolchain.toml`, `Cargo.toml`, workflows, verification
scripts, doctor behavior, README/support text, and affected tests. The change
must pass:

1. the fast gate on all CI platforms;
2. the strict integration gate on Linux and macOS;
3. installed `jjc doctor` verification;
4. an explicitly labelled latest-version advisory probe when applicable.

An advisory pass alone never promotes a new baseline. A failure is triaged as a
local regression, upstream protocol change, runner/tooling issue, or unsupported
shape before the support contract changes.
