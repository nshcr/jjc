# jjc phase 2 development plan

Phase 2 starts from the current implemented state of M0-M6, partial M7, and the
minimal M8 editor-command boundary in `docs/development-plan.md`. Its goal is
to make the existing `jj` diff and merge workflows more useful without turning
`jjc` into a full editor or reimplementing `jj` internals.

## Current baseline

Observed current capabilities:

- `jjc edit`, `jjc diff`, and `jjc merge` are real protocol entry points.
- Diff mode supports whole-hunk selection, line-level selection, function
  toggle, added/deleted file handling, binary accept-side choices, manual output
  editing, and undo/redo.
- Merge mode supports ordinary UTF-8 three-way output editing and binary
  accept-side resolution.
- Tree-sitter highlighting is shared by edit, diff, and merge text views.
- The editor buffer has structured commands and explicit suggestion
  application, but no agent runtime.

Known current limits:

- Visual mode, cross-line motion ranges, broader text objects, macros, and
  multi-register editing are still deferred.
- Some merge cases are bounded by the external `jj` merge-tool protocol, not by
  local UI code alone.
- There is no agent runtime, network call path, or background write path.

Execution status:

- P2.1 implemented: added and deleted files are whole-file diff entries.
- P2.2 implemented: binary diff files support whole-file accept-side choices.
- P2.3 implemented: `%`, `ciw`, `diw`, and `yiw` are available in the shared
  Vim core.
- P2.4 verified: protocol-limited merge cases remain documented and covered by
  negative smoke tests.
- P2.5 verified: suggestions remain explicit in-memory edits with normal save
  semantics.

## Socratic operating loop

Use this loop for every Phase 2 slice:

1. State the goal slice in one sentence.
2. Re-read the current protocol path before editing code.
3. Name the smallest user-visible behavior that proves the slice works.
4. Add or update the smallest test that fails without the behavior.
5. Implement the smallest shared-root change.
6. Run the slice test and then the project validation set.
7. Update README or this plan only when the user-visible contract changes.
8. Stop if the next step would change scope, depend on upstream `jj`, or require
   an irreversible cleanup.

This is not a general questioning step. Ask only when the answer changes the
next implementation action.

## Phase 2 priorities

Implementation order:

1. Added and deleted file support in diff mode.
2. Binary diff accept-side support.
3. Small Vim compatibility improvements.
4. Merge protocol boundary cleanup and documentation.
5. Agent-ready layer preservation checks.

Non-goals:

- Do not add a plugin system.
- Do not add an agent runtime.
- Do not link against `jj_lib`.
- Do not implement a full `scm-record` replacement.
- Do not add a broad TUI automation framework.
- Do not solve merge cases that `jj` does not pass to an external merge tool.

## P2.1: Added and deleted file diff support

Status: implemented.

Problem:

Diff mode currently rejects files that are not present on both sides. That makes
`jj split`, `jj squash -i`, and `jj restore -i` less useful for common changes
that add or delete files.

Scope:

- Represent a changed file as one of `modified`, `added`, `deleted`, or
  `unsupported`.
- For added text files, selected means write the right-side file to `$output`;
  unselected means omit the file from `$output`.
- For deleted text files, selected means delete the file from `$output`;
  unselected means restore the left-side file to `$output`.
- Keep line-level and function-level selection for modified files only.
- Show added/deleted files as whole-file entries first.
- Keep binary added/deleted files unsupported until P2.2.

Design:

- Replace the current "both sides must be files" load branch with a small file
  kind enum local to `src/diff.rs`.
- Keep reconstruction inside `DiffFile::render` or an adjacent helper.
- For deletion, remove the output path when selected instead of writing an empty
  file.
- For addition, create parent directories before writing the selected file.
- Treat `JJ-INSTRUCTIONS` as ignored metadata, same as current diff behavior.

Tests:

- Unit: added text file renders only when selected.
- Unit: deleted text file removes output when selected and restores left when
  unselected.
- Smoke: `jj split --tool jjc` can put a newly added text file into the selected
  commit.
- Smoke: `jj restore -i --tool jjc` can restore a deleted text file.

Acceptance:

- Added and deleted text files are no longer blocked as unsupported diff files.
- Existing modified-file hunk and line selection behavior is unchanged.
- `cargo test` passes.

## P2.2: Binary diff accept-side support

Status: implemented.

Problem:

Binary merge has accept-side behavior, but binary diff currently fails as an
unsupported file. For daily `jj restore -i` and simple split decisions, choosing
left or right bytes is enough.

Scope:

- Support binary files as whole-file entries only.
- For modified binary files, selected means keep right bytes, unselected means
  restore left bytes.
- For added binary files, selected means write right bytes, unselected means omit
  the file.
- For deleted binary files, selected means remove output, unselected means write
  left bytes.
- Do not add binary preview or byte editing.

Design:

- Reuse the file kind from P2.1.
- Store bytes for binary file entries instead of forcing UTF-8 conversion.
- Render a compact binary summary: path, kind, byte counts, selected state.
- Keep manual edit disabled for binary entries.

Tests:

- Unit: binary modified file writes selected side bytes.
- Unit: added and deleted binary files follow whole-file selection.
- Smoke: `jj restore -i --tool jjc` can restore a binary modification.

Acceptance:

- Binary diff decisions are possible without leaving the TUI.
- No binary path is silently treated as UTF-8.
- `cargo test` passes.

## P2.3: Small Vim compatibility improvements

Status: implemented.

Problem:

The current editor covers frequent normal and insert commands, but a few missing
motions block everyday editing more often than advanced features.

Scope:

- Add `%` matching for brackets on the current line or nearby cursor position.
- Add the smallest useful text objects: `ciw`, `diw`, and `yiw`.
- Keep Visual mode deferred.
- Keep cross-line operator ranges deferred unless needed by text objects.
- Do not add macros or multi-register support.

Design:

- Extend the current `Vim` parser only where command state already exists.
- Keep text-object ranges as `TextBuffer` helpers.
- Add tests before changing command dispatch.

Tests:

- Unit: `%` jumps between paired `()`, `[]`, and `{}` on a line.
- Unit: `ciw`, `diw`, and `yiw` work on ASCII and UTF-8 words.
- Regression: existing motion, delete, paste, undo, and redo tests still pass.

Acceptance:

- The added commands work in `edit`, merge output, and diff manual output
  because they share the same Vim core.
- No Visual mode state is introduced.
- `cargo test` passes.

## P2.4: Merge protocol boundary cleanup

Status: verified against the current external `jj` merge-tool boundary.

Problem:

Some M7 items are local implementation work, while others require upstream `jj`
or an internal integration path. The plan should keep those separate so future
work does not chase impossible external-tool behavior.

Scope:

- Keep binary normal-file conflict support.
- Keep delete/modify "keep modified side" support.
- Keep delete side, non-normal files, executable-bit-only conflicts, file mode
  resolution, and multi-side UI documented as protocol-limited unless current
  `jj` behavior changes.
- Add tests only for cases that current `jj` actually passes to `jjc`.

Design:

- Do not add fake UI actions for choices that cannot be faithfully returned via
  the single `$output` file.
- Keep negative smoke tests for protocol-limited cases.
- If upstream `jj` changes, update this section before implementing new UI.

Tests:

- Existing negative smoke tests must keep passing.
- Add new positive tests only when `jj` invokes the external tool for the case.

Acceptance:

- README and plan clearly distinguish implemented behavior from protocol
  limits.
- No test asserts impossible behavior for the current external merge protocol.

## P2.5: Agent-ready layer preservation

Status: verified. No agent runtime was added.

Problem:

The current editor command layer is enough for explicit suggestions. Adding a
runtime too early would create a larger trust and UX problem than it solves.

Scope:

- Preserve structured buffer commands.
- Preserve explicit suggestion application.
- Keep "no write until user save" as a tested invariant.
- Do not add background agents, network calls, model calls, or auto-writes.

Design:

- Treat future commit-message assistance as a separate design after P2.1-P2.3.
- If a suggestion API expands, keep it as explicit edit commands on the buffer.

Tests:

- Existing suggestion test remains required.
- Add regression tests only when the command API changes.

Acceptance:

- Suggestions can change the in-memory buffer.
- Files are written only through normal save paths.

## Validation set

Run this before marking a Phase 2 slice complete:

```sh
cargo fmt --check
cargo test
```

For protocol-affecting slices, the relevant smoke test must exercise real `jj`
commands. Prefer a small `JJC_KEYS` scripted flow over a broad UI harness.

## Completion rules

A Phase 2 slice is complete only when:

- The user-visible behavior is implemented.
- The smallest relevant unit or smoke test fails without the behavior.
- `cargo fmt --check` and `cargo test` pass.
- README or docs are updated if commands, keys, limits, or setup change.
- Protocol-limited cases are labeled as limits, not as missing local work.

Stop and checkpoint before:

- Adding a new dependency.
- Linking against `jj_lib`.
- Introducing an agent runtime.
- Rewriting the shared editor model.
- Implementing merge behavior that current `jj` does not expose to external
  tools.
