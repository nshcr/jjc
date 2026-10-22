# Upstream alignment audit — 2026-10-23

## Published baseline

The latest stable release remains [jj v0.46.0](https://github.com/jj-vcs/jj/releases/tag/v0.46.0),
published on 2026-10-07 at 21:28:17 UTC, targeting commit
`7d382314f9420b4b8d932245b49c99492c812b9f`.
Its [Rust requirement](https://github.com/jj-vcs/jj/blob/v0.46.0/Cargo.toml)
is 1.97.1. The existing jjc protocol baseline and toolchain remain unchanged.

## Unreleased upstream snapshot

The inspected main snapshot is
[`3f3bddbd1c649a8d37075876688a4e9cec6f26fa`](https://github.com/jj-vcs/jj/commit/3f3bddbd1c649a8d37075876688a4e9cec6f26fa),
dated 2026-10-22 at 16:16:50 UTC and
[125 commits ahead of v0.46.0](https://github.com/jj-vcs/jj/compare/v0.46.0...3f3bddbd1c649a8d37075876688a4e9cec6f26fa).
It was inspected from official source, not compiled or run. This audit does
not extend jjc's runtime support claim.

| Surface | Change from v0.46.0 | Disposition |
| --- | --- | --- |
| External diff editor | Checkout uses asynchronous internal APIs; no argument or output protocol change identified | Keep existing configuration |
| Text/description editor | `TextEditor` adds equality derives; line/path substitution and combined split descriptions remain unchanged | Keep existing `edit --line` route |
| External merge editor | Shared per-file processing and tree-update helpers; no changed arguments or output checks identified | Keep existing two-side merge route |
| Built-in `:editor` merge tool | New route invokes `ui.editor` on conflict markers, starting at the first conflict, and accepts multiple sides | Unreleased and not tested with jjc |

The multi-side capability belongs to the new built-in `:editor` route.
External tools, including `merge-tools.jjc`, still require two sides. Both
routes reject empty or unchanged output. Existing jjc text-editor arguments
are a plausible entry point for `jj resolve --tool :editor`; runtime evidence
is required before documenting it as supported.

Sources pinned to the inspected snapshot:

- [Unreleased changelog](https://github.com/jj-vcs/jj/blob/3f3bddbd1c649a8d37075876688a4e9cec6f26fa/CHANGELOG.md)
- [Merge-tool selection, side restrictions, and text editing](https://github.com/jj-vcs/jj/blob/3f3bddbd1c649a8d37075876688a4e9cec6f26fa/cli/src/merge_tools/mod.rs)
- [External tools](https://github.com/jj-vcs/jj/blob/3f3bddbd1c649a8d37075876688a4e9cec6f26fa/cli/src/merge_tools/external.rs)
- [Description editor](https://github.com/jj-vcs/jj/blob/3f3bddbd1c649a8d37075876688a4e9cec6f26fa/cli/src/description_util.rs)
- [Main Rust requirement](https://github.com/jj-vcs/jj/blob/3f3bddbd1c649a8d37075876688a4e9cec6f26fa/Cargo.toml)

## Local branch selection

The checked main is `7e919e3`; dev's unique commit is `21b9e85`. Main already
implements dev's line positioning and combined-description preparation, with
additional real jj split/file-edit coverage and new-file handling. Consolidation
keeps main's implementation and current support contracts while retaining
unique tests for native paths, line hints, and bulk descriptions. The
[2026-10-03 dev audit](upstream-alignment-2026-10-03.md) remains a historical record.

The older terminal branches contain changes already integrated through rebase:

| Local commit | Equivalent main commit |
| --- | --- |
| `7314cb3` | `9411933` |
| `c22d4f3` | `d377337` |
| `948bc19` | `e0e4306` |

Each pair has an identical stable patch-id, and `git cherry main <branch>`
marks all commits on both terminal branches as equivalent. No second merge is
needed. Source branches are retained; remote publication is outside this local
integration.

## Local acceptance

The consolidated candidate passed `./scripts/verify.sh full` on 2026-10-23
on macOS arm64 with Rust 1.97.1 and jj 0.46.0. Formatting, locked checks,
Clippy, and all 214 tests passed: 126 unit tests, 8 tree-entry tests, 6 marker
tests, 4 release-tag checks, 35 smoke tests, and 35 PTY tests. The 84
filesystem/real-jj/PTY integration tests also passed with required prerequisites
and the exact jj version enforced. Locked offline installation, binary startup,
and the installed `jjc doctor` passed.

The retained additions check invalid line hints, Unix native-path argv bytes,
CRLF/Unicode and empty-file line positioning, and actual two-commit bulk
description edits. Canceling bulk description editing preserves both commit
IDs and descriptions. Main's existing real split/file-edit tests remain intact.

The complete gate log is
`target/local-branch-consolidation-2026-10-23-full.log`; focused logs are under
`target/integration-tests-evidence/`. Save/cancel terminal transcripts are under
`target/protocol-evidence/`. All 23 local Markdown file targets in the four
changed documents exist, and an independent review found no actionable defect.
This is `verified-local`; hosted CI on the resulting merge revision and runtime
verification of unreleased upstream main remain outside this acceptance.
