# jjc development plan

This document is the canonical product roadmap for `jjc`. Detailed historical
work and acceptance evidence live in the phase documents linked below.

Status as of 2026-07-10:

- Foundation milestones and Phase 2 are complete within their recorded scope.
- Phase 3 features are implemented, but later review found integration and
  release-gate gaps that its original completion wording did not capture.
- [Phase 4](phase-4-development-plan.md) is complete within its supported local
  boundaries. The hosted CI result remains pending external evidence.

## Product contract

`jjc` is one terminal-native Rust TUI for the three Jujutsu (`jj`) editing
surfaces:

- text editor: `ui.editor`
- diff editor: `ui.diff-editor`
- merge editor: `ui.merge-editor`

Commit descriptions are the highest-frequency `ui.editor` use case, but
`ui.editor` is a general text-editor contract. `jj` also uses it for content
such as sparse patterns. Commit-description-only behavior must therefore be
selected from the file contract, not applied to every `jjc edit` invocation.

The project is terminal-first and protocol-first. It is not intended to become
a full IDE or an internal clone of `jj`.

## Stable CLI boundary

```sh
jjc doctor
jjc edit <file>
jjc diff <left> <right> <output>
jjc merge <left> <base> <right> <output> --marker-length <n> --path <repo-path>
```

The diff route intentionally uses the experimental three-pane directory
protocol: `$output` starts as a copy of `$right`, and `jjc` writes the selected
tree there. The two-pane `$left`/`$right` protocol is not a local target.

The supported Phase 4 configuration is:

```toml
[ui]
editor = ["jjc", "edit"]
diff-editor = "jjc"
merge-editor = "jjc"

[merge-tools.jjc]
program = "jjc"
edit-args = ["diff", "$left", "$right", "$output"]
merge-args = [
  "merge",
  "$left",
  "$base",
  "$right",
  "$output",
  "--marker-length",
  "$marker_length",
  "--path",
  "$path",
]
merge-tool-edits-conflict-markers = true
conflict-marker-style = "git"
```

`jjc doctor` emits the resolved executable path for `ui.editor` and
`merge-tools.jjc.program`; the named `jjc` diff/merge routes resolve through that
tool entry. The two merge-tool options make `jj` prefill `$output` with the Git
diff3 markers that `jjc` recognizes.

## Product priorities

Maintain the three surfaces in this order:

1. `ui.editor` correctness and daily usability.
2. Diff tree and selection correctness.
3. Merge usability within the external merge-tool boundary.

All three remain release-critical. A failure that can silently select the wrong
tree entry is more severe than a missing convenience command.

## Interaction model

The UI should feel like a small terminal tool, not a full editor suite.

- Text editing uses one shared minimal Vim model.
- Diff/list navigation uses direct, discoverable single-key actions.
- Terminal cell width, not Unicode scalar count or UTF-8 byte count, determines
  cursor placement and horizontal visibility.
- Save/cancel semantics follow the calling `jj` protocol and must never silently
  write on cancel.

Implemented Vim-compatible commands include high-frequency normal and insert
motions, find motions, line and word operators, a single yank buffer, case
conversion, undo/redo, `:wq`, and `:q!`.

Deferred editor breadth:

- Visual mode.
- Cross-line operator ranges and broader text objects.
- Macros and multiple registers.
- Search and replace UI.

These are not Phase 4 work unless a correctness fix requires a shared primitive.

## Dependency policy

Prefer the standard library for protocol and filesystem work, but use focused
libraries for terminal, parsing, and Unicode behavior that should not be
hand-rolled.

Current dependency roles:

- `clap`: CLI parsing.
- `crossterm` and `ratatui`: terminal lifecycle, input, and rendering.
- `similar`: text diff and hunk generation.
- Tree-sitter grammar crates: function-aware hunks and shared syntax
  highlighting.
- `serde` and `toml`: `jjc` configuration.

Phase 4 added focused Unicode width and grapheme segmentation dependencies for
terminal-cell metrics and safe editing boundaries. They add no background
runtime or network path.

## Current architecture

The current source tree is:

```text
src/
  main.rs       process entry point
  lib.rs        library module boundary
  cli.rs        clap commands and arguments
  app.rs        command dispatch, path validation, terminal cleanup guard
  editor.rs     ui.editor TUI and save/cancel policy
  buffer.rs     UTF-8 text buffer and structured edits
  vim.rs        shared normal/insert command model
  input.rs      terminal and scripted key input
  diff.rs       three-pane diff protocol, selection, output materialization
  merge.rs      external merge protocol and output editing
  render.rs     shared styled text and terminal cursor rendering
  scroll.rs     viewport and scrollbar state
  syntax.rs     Tree-sitter language registry, functions, highlighting
  theme.rs      render theme model
  config.rs     jjc config loading
  doctor.rs     non-mutating environment/config diagnostics

tests/
  smoke.rs      scripted real-jj protocol tests
  tty.rs        PTY and terminal-behavior tests
  diff_tree_entries.rs
                executable, symlink, and fail-before-write integration
  merge_markers.rs
                dynamic-marker real-jj integration
```

Keep shared behavior at these existing roots. Add a new module only when a
concept has at least two real consumers or when isolation materially improves
filesystem or rendering correctness.

## Verified implementation baseline

The checkout already contains:

- Real `edit`, three-pane `diff`, and external `merge` entry points.
- Shared normal/insert text editing, structured buffer edits, undo/redo, and
  explicit save/cancel paths.
- Whole-hunk, line, function, file, added/deleted, binary, executable, symlink,
  and manual-output diff choices. Regular-file/symlink transitions are
  supported; directory/special transitions fail before output mutation.
- Ordinary text and binary normal-file merge choices, exact dynamic Git diff3
  marker parsing, multi-block choices, and partial resolution.
- Grapheme-safe editing, terminal-cell horizontal viewports for text-editing
  surfaces, fingerprint-cached highlighting, and a `512 KiB` plain fallback.
- Description, sparse, and generic `ui.editor` profiles.
- Scripted real-`jj` smoke tests, eight tree-entry integrations, three dynamic
  marker integrations, and a fixed-size replayed 19-test PTY suite.
- A read-only `jjc doctor` whose generated config is exercised through all three
  routes.
- Rust 1.93.1 metadata and CI matrices for formatting, clippy, tests, pinned
  `jj 0.43.0`, installation, and latest-`jj` advisory probing.

The final Phase 4 local gate passed from the converged working state. After the
branch is published, the hosted CI result remains separate external evidence.

## Roadmap

### Foundation milestones: complete within recorded scope

The initial milestones established:

- M0: protocol shell and terminal cleanup.
- M1: built-in text editing and `ui.editor` save/cancel.
- M2/M5: hunk- and line-level diff selection plus manual output editing.
- M3: ordinary three-way text merge output.
- M4/M5.5: tests, smoke coverage, and shared Vim hardening.
- M6: function-aware hunks and shared syntax highlighting.
- M8 boundary: structured, explicit in-memory suggestion edits without an agent
  runtime.

The old M7 “complex merge compatibility” list mixed local work with conflict
shapes that `jj` does not pass to external tools. Its valid local behavior and
upstream limits are now recorded in the phase plans; it is not an open promise
to bypass the upstream protocol.

### Phase 2: complete, with Phase 4 corrections

[Phase 2](phase-2-development-plan.md) added regular-file added/deleted and
binary diff choices, small Vim improvements, merge-protocol boundary tests, and
preserved the agent-ready buffer boundary. Phase 4 P4.1 supersedes its
content-only tree model with executable and symlink-aware snapshots while
keeping directory/special transitions explicitly unsupported.

### Phase 3: historically implemented, closure expanded by Phase 4

[Phase 3](phase-3-development-plan.md) added `doctor`, multi-file diff
navigation, conflict-block operations, description warnings, and a release
checklist. Later review found that the recommended merge config did not make the
conflict blocks available and that the release checklist was not an enforced
compatibility gate. Phase 4 P4.2, P4.4, and P4.5 supersede those completion
claims without erasing the implementation history.

### Phase 4: complete within the supported local boundary

[Phase 4](phase-4-development-plan.md) is the current acceptance source:

1. P4.1 implements safe executable/symlink diff entries and fail-before-write
   rejection for directory/special/unsafe-output cases.
2. P4.2 makes exact dynamic Git-marker block behavior reachable and proves
   complete, partial, and automatically lengthened real-`jj` flows.
3. P4.3 implements grapheme/cell-correct text viewports, fingerprint caching,
   a `512 KiB` fallback, and fixed-size replayed PTY coverage.
4. P4.4 separates description/sparse/generic semantics and proves the generated
   doctor configuration through all three routes.
5. P4.5 defines Rust 1.93.1, pinned `jj 0.43.0`, strict integration, install,
   platform, and advisory latest-`jj` CI gates.

The documented final local gate passed from the converged working state. Hosted
CI remains separate external evidence until the workflow actually runs.

### Later product work

After Phase 4 closes, separately design and prioritize:

- Visual mode and broader Vim compatibility.
- Search and larger navigation surfaces.
- More adaptive merge layouts.
- Commit-description assistance built on explicit structured edits.
- A possible internal/upstream integration path for conflict shapes that the
  external tool protocol cannot represent.

No agent runtime, background write path, plugin system, or `jj_lib` dependency
is implied by Phase 4.

## Upstream merge boundary

The external merge route consumes one `$output` file and is invoked only for
conflicts that `jj` can materialize for an external tool. Current upstream
limitations include deletion as the chosen result, non-normal tree entries,
unresolved executable-bit conflicts, file/directory conflicts, symlink
conflicts, and conflicts with more than two sides.

Phase 4 does not promise local support for shapes that `jj` rejects before
invoking `jjc` or that cannot be faithfully returned through one `$output`
file. Revisit those only after an upstream protocol change or a separately
approved internal integration design.

## Validation policy

Use layered evidence:

- Unit tests for buffer, rendering, tree-entry classification, materialization,
  marker parsing, and config generation.
- Filesystem integration tests for output type, link target, executable bit,
  unsafe ancestry, unsupported-entry fail-before-write behavior, and cancel
  safety.
- Real-`jj` smoke tests for every protocol-affecting behavior.
- PTY tests for cursor placement, viewport behavior, and terminal cleanup.
- CI gates for formatting, clippy, tests, install/package checks, and the
  declared platform/`jj` compatibility matrix.

Tests that require `jj` or a PTY helper must not silently skip in the CI job that
claims to exercise them. Unsupported platform cases should be explicitly gated
and documented rather than counted as passing evidence.
