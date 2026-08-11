# jjc phase 5 terminal UX plan

Status: **implemented and locally verified**. The converged local gate passes
against `jj 0.44.0`. Hosted GitHub Actions remains separate external evidence
until the workflow runs on the final revision.

Started: 2026-08-10.

Phase 5 reduces repeated terminal work without weakening the protocol and
filesystem safety established in Phase 4. It converges the three editing
surfaces around discoverable help, compact status, consistent finish actions,
and fewer keys for common diff and merge decisions.

## Goal contract

Phase 5 delivers:

- In-app `?` / `F1` help for edit, diff, and merge views.
- Consistent `Ctrl-S` save and `Ctrl-C` cancel actions on every surface while
  retaining the existing Vim-style and direct keys.
- Resize-driven redraw and release-event filtering for stable interactive use.
- Direct global, file, hunk, and changed-line diff presets with one-step undo.
- Selection-mode horizontal panning and changed-line navigation that skips
  context and duplicate rows from one replacement pair.
- Safe accept-and-advance and batch-side operations for merge conflict blocks.
- Explicit conflict progress, unavailable-base feedback, and empty-output
  confirmation.
- A declared and truthfully reported `jj 0.44.0` protocol baseline.

Invariants:

- Cancel never writes editor, diff, or merge protocol output.
- Help, resize, navigation, and horizontal panning never change a selection or
  output buffer.
- A global or “only current” diff operation creates one undo snapshot.
- Selecting an unavailable merge base never falls back to replacing the whole
  output with the base input.
- Batch merge choices preflight every remaining block and make no partial
  change when the chosen side is unavailable anywhere.
- A whole-file deletion side cannot bypass the empty-output confirmation,
  including through a batch command.
- Empty merge output means an empty regular file on the external-tool protocol;
  it is never presented as deletion support.

## Shared terminal shell

`src/ui.rs` owns the compact mode badge, context/action status line, and centered
rounded help popup used by all three surfaces. Help remains inside the alternate
screen and closes with `?`, `F1`, or `Esc`.

The input layer now treats resize as an application event so each TUI redraws
at the new dimensions. Key-release events are ignored; press and repeat events
remain active. Scripted tests support `F1`, `Ctrl-S`, and `Ctrl-C` tokens.

The existing commands remain valid. The shared finish actions are additions,
not breaking replacements:

- edit: `Ctrl-S` or `:wq`; `Ctrl-C` or `:q!`;
- diff: `Ctrl-S` or `w`; `Ctrl-C` or `q`;
- merge: `Ctrl-S` or `:wq`; `Ctrl-C`, `q`, or `:q!`.

## Diff selection convergence

The diff view adds:

- `s` / `d`: select or deselect the complete diff;
- `S` / `D`: select or deselect the current file;
- `o`: keep only the current hunk;
- `O`: keep only the current changed line or replacement pair;
- `Enter`: toggle the current hunk and advance;
- `h` / `l` or arrow keys: pan long selection rows horizontally.

Changed-line navigation now lands only on changed groups. Context rows are
skipped, and the old/new rows of one replacement are treated as one decision.
The first cursor position in a hunk is its first changed group rather than the
first context row.

Full, partial, and unselected hunks use distinct markers. The status line shows
current hunk position, full and partial selection counts, and horizontal pan.
Global and “only current” operations clear incompatible manual file outputs and
remain one-step undoable; undo also restores any displaced manual output.
When a newer hand edit is completed, stale outer selection undo/redo history is
discarded so it cannot overwrite that manual result.

## Merge convergence and safety

Choosing `1`, `2`, or `3` for a parsed conflict block now accepts that side and
focuses the next remaining block automatically. The status line reports the
current conflict and remaining count, and explicitly marks base as unavailable
for two-way marker blocks.

Batch commands resolve all remaining blocks with one side:

- `:al` / `:all-left`;
- `:ab` / `:all-base`;
- `:ar` / `:all-right`.

The batch path computes every replacement before mutation. If any requested
base section is absent, the output stays unchanged. Pressing `Enter` saves only
after all parsed blocks are resolved; explicit `Ctrl-S` / `:wq` retains the
existing partial-resolution confirmation.

`jj 0.44.0` accepts empty external-tool output as an empty regular file. `jjc`
therefore requires a second save before returning zero bytes. Selecting a
whole-file empty side is canonicalized to zero bytes before this check, so a
preserved trailing newline cannot bypass the warning. The same rule applies to
batch resolution.

## Compatibility reporting

The tested baseline and blocking CI installation are pinned to `jj 0.44.0`.
`jjc doctor` reports exact baseline matches as tested and emits a warning for
older, newer, or unparseable versions. A present but drifted `jj` still permits
diagnostics and config output; the warning prevents it from being described as
verified.

Phase 4 remains the historical source for its `jj 0.43.0` acceptance snapshot.
The baseline change does not rewrite that earlier evidence.

## Local evidence

The converged working state passed:

```sh
cargo fmt --check
cargo metadata --locked --no-deps --format-version 1
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
JJC_REQUIRE_INTEGRATION=1 cargo test --locked \
  --test smoke --test tty --test diff_tree_entries --test merge_markers
cargo install --locked --path . --root target/install-check --force
target/install-check/bin/jjc doctor
```

Observed local results:

- 106 unit tests passed.
- 8 diff tree-entry integrations passed.
- 3 dynamic merge-marker integrations passed.
- 24 real-`jj` smoke tests passed.
- 21 fixed-size replayed PTY tests passed, including in-app help and
  selection-mode horizontal panning.
- Strict integration mode passed all 56 integration tests without prerequisite
  skips.
- The locked release install succeeded, and its `doctor` output identified
  local `jj 0.44.0` as the tested protocol.

This is verified-local evidence. It is not hosted CI, another operating system,
or production field evidence.

## Terminal test maintenance (2026-08-12)

The replayed PTY suite now contains 29 tests. Eight added scenarios exercise
real `Ctrl-S` and `Ctrl-C` bytes across edit, diff, and merge flows, no-write
cancellation after in-memory changes, both empty-output confirmation paths,
resize-triggered redraw, and the interleaved diff selection/manual-edit history
regression. The shared harness now also requires the default cursor-shape escape
before leaving the alternate screen, including expected non-zero cancellation
exits.

The focused `cargo test --locked --test tty` gate passed all 29 tests locally,
and `./scripts/verify.sh full` passed the complete formatting, Clippy, unit,
real-`jj`, PTY, offline-install, and installed-doctor gate. Hosted CI remains
separate evidence.

## Deferred work

Phase 5 does not add Visual mode, search, mouse input, adaptive merge-pane
layouts, multi-side conflicts, deletion results, or an agent runtime. Merge
shapes that the external `jj` tool protocol cannot invoke or faithfully return
remain upstream boundaries.
