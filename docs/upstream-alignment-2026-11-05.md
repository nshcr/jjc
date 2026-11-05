# Upstream alignment — 2026-11-05

The current stable target is [jj 0.47.1](https://github.com/jj-vcs/jj/releases/tag/v0.47.1),
published on 2026-11-05 at 00:34:13 UTC, commit
`d16ee1fc88a72c54142241b079a34bbf3b033bc0`. This patch fixes the Linux build
dependency error in 0.47.0. The shared Rust minimum remains 1.97.1.

## Editor contract

The [0.47 changelog](https://github.com/jj-vcs/jj/blob/v0.47.1/CHANGELOG.md)
introduces `jj resolve --tool :editor`. This route invokes the configured
`ui.editor`, supplies the first conflict's 1-based line number, and lets users
manually edit materialized conflict markers, including multi-side conflicts.
jjc's existing `edit --line` entry point implements this route without a new
command or internal jj dependency.

The named external `merge-tools.jjc` route still requires two sides. Its
left/base/right selection UI does not become a multi-side UI. Both routes reject
empty or unchanged output; removing the output file remains an I/O error.
The external diff directory contract, description editor substitutions, combined
split descriptions, and file-edit save/cancel contract are unchanged.

Fixed sources:

- [Merge tool selection and text editing](https://github.com/jj-vcs/jj/blob/v0.47.1/cli/src/merge_tools/mod.rs)
- [External merge protocol](https://github.com/jj-vcs/jj/blob/v0.47.1/cli/src/merge_tools/external.rs)
- [Description editor](https://github.com/jj-vcs/jj/blob/v0.47.1/cli/src/description_util.rs)
- [Rust minimum](https://github.com/jj-vcs/jj/blob/v0.47.1/Cargo.toml)

Doctor, the strict local gate, blocking CI and release installs, and the current
README/support contract now target 0.47.1. Historical acceptance records retain
their original versions. Runtime validation is recorded separately from source
inspection; the unchanged external merge limitations continue to have explicit
regression coverage.

## Local acceptance

On macOS arm64, Rust 1.97.1 and the exact jj 0.47.1 binary passed
`./scripts/verify.sh full`: 126 unit tests, 8 tree-entry tests, 6 marker tests,
4 release-tag checks, 38 smoke tests, and 36 PTY tests (218 total). All 88
integrations also passed with prerequisites and the exact protocol version
required. Locked offline installation and installed doctor succeeded.

New real-jj tests exercise three-side resolution via `:editor`, cancellation
preserving the commit ID and file bytes, and rejection of unchanged output.
A real PTY scenario opens at the first conflict on line 31, saves or cancels,
and checks terminal cleanup. Zero-byte output rejection is confirmed from
upstream source; no artificial wrapper is used to claim runtime coverage.

The complete local log is `target/jj-0.47.1-full.log`. Focused results and
save/cancel terminal transcripts are retained in `target/protocol-evidence/`
as `text-editor-focused-0.47.1.log` and `resolve-text-editor-{save,cancel}.log`.
Actionlint 1.7.12 passed both workflows with external shell/Python linters
disabled. Independent review found no actionable defect. These are local
results; hosted CI must bind to the resulting revision.

## Pending pull requests

On 2026-11-05, two open dependency PRs were identified:

- [#14](https://github.com/nshcr/jjc/pull/14): Rust toolchain Action SHA update.
- [#15](https://github.com/nshcr/jjc/pull/15): similar, Tree-sitter/language, and
  unicode-width patch updates.

Both were mergeable and their existing head checks passed. Those historical
checks used the earlier protocol baseline; they do not establish 0.47.1 runtime
acceptance. Neither PR is included in this alignment change.
