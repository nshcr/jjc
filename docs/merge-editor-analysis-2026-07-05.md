# jjc merge editor analysis - 2026-07-05

## Scope

This analysis checks the current `jjc` merge editor against:

- current `jjc` implementation and tests in this repository
- local Jujutsu (`jj`) source at `/Users/juniusc/Source/jj`
- historical plans in `docs/development-plan.md`,
  `docs/phase-2-development-plan.md`, and `docs/phase-3-development-plan.md`

The result does not create a new development plan because no local merge-editor
feature gap remains that can be fixed independently of the current `jj`
external merge-tool protocol.

## Upstream jj protocol facts

The current `jj` external merge-tool protocol is file-by-file:

- `jj resolve` builds a list of conflicted paths and invokes the merge editor
  for each path.
- `$output` is required for merge tools; `jj` reads it after the tool exits.
- `$left`, `$base`, `$right`, `$marker_length`, and `$path` are available
  substitutions.
- by default, `$output` starts empty; a zero exit code plus non-empty changed
  output means the conflict is fully resolved.
- empty or unchanged output is rejected by `jj`.
- if `merge-tool-edits-conflict-markers` or `merge-conflict-exit-codes` is used,
  `jj` reparses conflict markers as partial resolution.
- `jj` only invokes external merge tools for materializable normal-file
  two-sided conflicts with resolved executable bit.
- non-normal files, unresolved executable-bit conflicts, and conflicts with more
  than two sides are rejected before external tool invocation.

## Current jjc behavior

`jjc merge <left> <base> <right> <output> --marker-length <n> --path <path>`
matches the protocol shape used by `jj`.

Implemented behavior:

- ordinary UTF-8 three-way conflicts can be resolved by editing output or
  accepting left/base/right.
- conflict-marker blocks in the output buffer can be navigated and resolved one
  block at a time.
- saving with remaining conflict markers warns once before writing.
- binary normal-file conflicts support left/base/right byte selection.
- delete/modify can keep the modified side when `jj` can materialize it.
- delete-as-result, non-normal files, executable-bit conflicts, and multi-side
  conflicts remain protocol-limited, not local UI gaps.

## Gaps found and fixed

Two integration coverage gaps were found:

- `jj resolve --tool jjc` without an explicit path was not covered, so the
  sequential multi-file invocation path from upstream `jj` was untested.
- real TTY execution through `jj resolve --tool jjc` was not covered; existing
  TTY tests covered direct `jjc merge` and diff-editor `jj` entry points.

Both gaps were test coverage gaps, not implementation gaps.

Added tests:

- `tests/smoke.rs`: `jj_resolve_without_path_invokes_jjc_for_each_conflict`
- `tests/tty.rs`: `jj_resolve_uses_merge_editor_tty`

## Usability matrix

| Scenario | Status | Evidence |
| --- | --- | --- |
| simple text conflict with explicit path | supported | existing smoke test |
| cancel merge editor | supported | existing smoke test |
| binary normal-file conflict | supported | existing smoke test |
| delete/modify keep modified side | supported | existing smoke test |
| delete/modify choose deleted side | protocol-limited | existing negative smoke test |
| executable-bit conflict | protocol-limited | existing negative smoke test |
| symlink conflict | protocol-limited | existing negative smoke test |
| file/directory conflict | protocol-limited | existing negative smoke test |
| multi-side conflict | protocol-limited | existing negative smoke test |
| multiple conflicted files via `jj resolve` | supported | new smoke test |
| real TTY path via `jj resolve --tool jjc` | supported | new TTY test |

## Validation

Commands run:

```sh
cargo fmt
cargo fmt --check
cargo check
cargo test
```

Observed result:

- 72 unit tests passed.
- 22 smoke tests passed.
- 15 TTY tests passed.
- doc tests passed.

## Conclusion

Do not open a new merge-editor development plan now. The remaining limitations
are upstream protocol boundaries or explicitly deferred product scope, not
missing local implementation work.

The next useful work is ordinary maintenance: keep these smoke and TTY tests
green when upgrading `jj`, and revisit the protocol-limited cases only if
upstream `jj` changes what it passes to external merge tools.
