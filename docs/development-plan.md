# jjc development plan

This document is the canonical product roadmap for `jjc`. Detailed historical
work and acceptance evidence live in the phase documents linked below.

Status as of 2026-10-13:

- Foundation milestones and Phase 2 are complete within their recorded scope.
- Phase 3 features are implemented, but later review found integration and
  release-gate gaps that its original completion wording did not capture.
- [Phase 4](phase-4-development-plan.md) is complete within its supported local
  boundaries. The hosted CI result remains pending external evidence.
- [Phase 5](phase-5-terminal-ux-plan.md) is implemented and locally verified;
  its hosted CI result remains pending external evidence.
- The current protocol baseline is `jj 0.46.0`; the upstream alignments below
  supersede historical phase versions without changing the three CLI routes.

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
jjc edit <file> [--line <n>]
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
- Global and “only current” diff presets, changed-line navigation, selection-mode
  horizontal panning, partial-selection indicators, and in-app help.
- Ordinary text and binary normal-file merge choices, exact dynamic Git diff3
  marker parsing, safe accept-and-advance, batch side choices, conflict progress,
  empty-output confirmation, and partial resolution.
- Grapheme-safe editing, terminal-cell horizontal viewports for text-editing
  surfaces, fingerprint-cached highlighting, and a `512 KiB` plain fallback.
- Description, convergence-description, sparse, and generic `ui.editor` profiles.
- Scripted real-`jj` smoke tests, tree-entry and dynamic-marker integrations,
  and replayed PTY tests covering fixed and resized terminals, real control
  keys, guarded saves, and cleanup on cancel.
- A read-only `jjc doctor` whose generated config is exercised through all three
  routes and whose version status distinguishes the tested protocol from drift.
- Rust 1.97.1 metadata and CI matrices for formatting, clippy, tests, pinned
  `jj 0.46.0`, installation, and latest-`jj` advisory probing.

The Phase 5 local gate passed from the converged working state. After the branch
is published, the hosted CI result remains separate external evidence.

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

[Phase 4](phase-4-development-plan.md) remains the correctness and release-gate
source for its recorded `jj 0.43.0` snapshot:

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

### Phase 5: terminal interaction convergence

[Phase 5](phase-5-terminal-ux-plan.md) is the current UX acceptance source. It
adds a shared help/status shell and consistent global save/cancel actions,
reduces diff selection to global and “only current” operations, makes long diff
lines inspectable without manual editing, and makes merge block choices safe,
progressive, and batchable. It also moves the live protocol gate to `jj 0.44.0`
while preserving Phase 4 as historical evidence.

### Upstream alignment: jj 0.45.1

The stable target is [jj 0.45.1](https://github.com/jj-vcs/jj/releases/tag/v0.45.1)
(tag commit `7c41cdeb16b6b321c64e789a966b6adf723816a5`). The Rust baseline remains
1.93.1. The local verification script, CI/release installs, doctor, and current
support text name this same jj version. Development/prerelease versions must
not be labelled as the tested release even when their numeric version matches.

This alignment covers three upstream behaviors:

- `jj absorb --interactive` and `--tool`, introduced in 0.44.0, use the existing
  three-pane diff route. All, partial, empty, and canceled selection tests check
  the destination and source trees. Empty selection is an upstream error that
  preserves the commits, not a successful absorption.
- `jj converge`, introduced in 0.45.0, opens `ui.editor` with a
  `.jj-converge-description` file when the user chooses to edit descriptions
  that cannot be merged automatically. Unlike `.jjdescription`, its content
  is returned verbatim by jj: `JJ:` lines are ordinary text. The editor applies
  empty-description confirmation without treating those lines as instructions.
  Real terminal tests exercise description save and cancellation.
- The 0.45.0 conflict-parser fix preserves a side ending in a bare carriage
  return. Real-jj regressions check CRLF, bare CR, and missing-final-LF bytes
  through marker round trips and side selection. jjc now removes exactly one
  framing CR when a final, unterminated conflict uses CRLF markers, preserving
  any CR that belongs to the selected content. Single, partial, and batch
  choices share this decoding. Marker fixtures come from jj, whose line endings
  encode whether a side had a terminating LF.

The external-tool arguments and Git-marker configuration remain the same.
No `jj_lib` dependency or unreleased main-branch protocol is introduced. Local
results do not establish Linux/Windows behavior or hosted CI for this revision;
those require the configured platform jobs to run on the same commit.

Local validation on 2026-09-12 (macOS arm64, Rust 1.93.1, jj 0.45.1) passed
`./scripts/verify.sh full`: 111 unit tests, 8 tree-entry tests, 4 marker tests,
4 release-tag checks, 28 smoke tests, and 31 PTY tests. The 71 real-jj/filesystem/
PTY integration tests also passed in strict mode without skipped prerequisites.
The locked offline installation and installed `doctor` succeeded. Actionlint
1.7.12 checked both modified workflows (external shell/Python linters disabled),
and independent review found no remaining issue in this candidate. This is
`verified-local`; hosted checks on the resulting commit remain pending.

Upstream sources: [absorb selection](https://github.com/jj-vcs/jj/blob/v0.45.1/cli/src/commands/absorb.rs),
[converge description editing](https://github.com/jj-vcs/jj/blob/v0.45.1/cli/src/commands/converge.rs#L421),
and [CR preservation](https://github.com/jj-vcs/jj/blob/v0.45.1/lib/src/conflicts.rs#L858).

### Follow-up: CRLF-aware manual editing

The shared text buffer now separates editable line content from its CRLF
terminator while retaining raw lines for diff and merge serialization. Line-end
motions, insertion, deletion, replacement, splitting, joining, and linewise
paste preserve existing line endings. New lines use the current line's ending,
or the preceding line's ending at an unterminated EOF. A bare CR at EOF and
additional content CR bytes remain content. Mixed files are not normalized.

Vim `cc` changes the current line in place, insert-mode Right reaches the
insertion point before the terminator, and linewise paste can copy empty
lines. Unit regressions cover mixed endings, Unicode insertion boundaries,
and undo/redo. Real PTY tests cover CRLF append/newline/save and cancellation.
A real-jj marker test appends to the opening marker label with `A`, then
reparses and selects each side while preserving CRLF and EOF bare-CR bytes.

The follow-up candidate, including the empty-output correction below, passed
`./scripts/verify.sh full` on macOS arm64 with Rust 1.93.1 and jj 0.45.1:
121 unit tests, 8 tree-entry tests, 5 marker tests, 4 release-tag checks,
30 smoke tests, and 33 PTY tests. All 76 real-jj/filesystem/PTY tests also
passed in strict mode. The locked offline installation and installed `doctor`
passed, and independent review found no remaining issue after the empty-line
paste and Unicode-join regressions were fixed. This is `verified-local`;
Linux/Windows and hosted CI evidence remain separate.

### Follow-up: conflict marker suffix boundary

Conflict markers must be followed by ASCII whitespace or the end of the line.
Previously, body text such as `>>>>>>>literal` could be treated as a closing
marker, dropping that line and retaining the actual closing marker in a
successfully resolved file. The parser now checks this suffix boundary while
keeping the existing exact-marker-length policy.

Unit tests cover all four marker characters, rejecting attached text and
non-ASCII whitespace. A real `jj 0.45.1` regression checks byte-for-byte right
side selection and the absence of remaining conflicts. Both regressions failed
before the fix and passed afterward. This is a stable-protocol correctness fix;
no unreleased upstream feature or baseline change is involved.

The main-only candidate passed `./scripts/verify.sh full` on 2026-10-03
(macOS arm64, Rust 1.93.1, jj 0.45.1): 122 unit tests, 8 tree-entry tests,
6 marker tests, 4 release-tag checks, 30 smoke tests, and 33 PTY tests.
All 77 filesystem/real-jj/PTY tests also passed in strict mode. Locked offline
installation and installed doctor verification passed. This is `verified-local`;
Linux/Windows and hosted CI evidence remain separate.

### Later product work

After Phase 5 closes, separately design and prioritize:

- Visual mode and broader Vim compatibility.
- Search and larger navigation surfaces.
- More adaptive merge layouts.
- Commit-description assistance built on explicit structured edits.
- A possible internal/upstream integration path for conflict shapes that the
  external tool protocol cannot represent.

No agent runtime, background write path, plugin system, or `jj_lib` dependency
is implied by Phase 5.

## Upstream merge boundary

The external merge route consumes one `$output` file and is invoked only for
conflicts that `jj` can materialize for an external tool. Current upstream
limitations include deletion as the chosen result, non-normal tree entries,
unresolved executable-bit conflicts, file/directory conflicts, symlink
conflicts, and conflicts with more than two sides.

An empty `$output` is not a deletion signal. The `jj 0.45.1` protocol rejects
empty or unchanged output, and a missing output file produces an I/O error.
Earlier roadmap text claiming that empty output resolves to an empty regular
file was incorrect. Direct `jjc` use retains a second confirmation before
writing an empty file, with a warning that `jj resolve` will reject it. Users
can edit the content or cancel to keep the conflict.

Real-jj regressions choose an empty-file side and a deletion side, perform
both saves, and check the upstream rejection plus unchanged conflict bytes.
A separate successful tool removes its temporary output file and verifies the
I/O error and preserved conflict. These restrictions were rechecked against
the stable release and upstream main `a4a61b3916f2a46eb3fb9f49b4e9021f26fdc893`
on 2026-09-12. See the [external merge result checks](https://github.com/jj-vcs/jj/blob/v0.45.1/cli/src/merge_tools/external.rs#L283).

Phase 4 does not promise local support for shapes that `jj` rejects before
invoking `jjc` or that cannot be faithfully returned through one `$output`
file. Revisit those only after an upstream protocol change or a separately
approved internal integration design.

### Upstream alignment: jj 0.46.0

The stable target is [jj 0.46.0](https://github.com/jj-vcs/jj/releases/tag/v0.46.0).
Doctor, the strict local gate, blocking CI/release installs, and current support
documentation now name this version. Historical acceptance records above retain
their original versions. jjc and upstream jj now share Rust 1.97.1 as the
minimum and blocking compiler for local checks, CI, and release builds. The advisory job uses stable Rust for the latest upstream binary.

This alignment covers the changed text-editor contract:

- `ui.editor` supports `$path` and 1-based `$line` substitution. `jjc edit`
  accepts `--line`, rejects zero, and clamps a line past EOF to the last line.
  Doctor recommends explicit substitutions with `--` before the file path.
  The original `["jjc", "edit"]` config remains usable from line 1.
- `jj split` now edits both descriptions in one `.jjdescription` session.
  The `JJ: describe` separators remain intact for jj to parse. A real-terminal
  test selects part of a diff, edits both descriptions, and checks both trees
  and descriptions; cancellation at the description step preserves the original
  commit and working-copy content.
- `jj file edit` uses the text route for an existing or new file, including
  files in ancestor revisions. A missing file with an existing parent opens as
  an empty buffer and is created only on save. Missing parents, directories,
  and dangling symlinks remain errors. Ordinary `JJ:` content and empty-file
  saves retain generic text semantics. Integration tests check ancestor edits,
  descendant rebasing, paths with spaces, creation, empty output, and cancel.

The existing diff/merge argument configuration remains in use. Before this
change, all 77 existing real-jj/filesystem/PTY tests passed against 0.46.0;
new regression tests then demonstrated rejection of `--line` before the fix.
Terminal transcripts for split and file-edit save/cancel scenarios are retained
under `target/protocol-evidence/`.

Initial validation on 2026-10-13 (macOS arm64, Rust 1.93.1, jj 0.46.0), before
aligning the shared toolchain, passed
`./scripts/verify.sh full`: 124 unit tests, 8 tree-entry tests, 6 marker tests,
4 release-tag checks, 33 smoke/CLI tests, and 35 PTY tests. The 82 integration
tests also passed with required prerequisites and the exact jj version enforced.
Locked offline installation and the installed doctor succeeded. Actionlint
1.7.12 checked both workflows with external shell/Python linters disabled.
The complete local gate log is `target/jj-0.46.0-full.log`. These results are
`verified-local`; hosted CI on the resulting revision remains pending.

The final shared-toolchain candidate also passed `./scripts/verify.sh full` on
2026-10-13 with Rust 1.97.1 and jj 0.46.0: the same 210 tests, including all 82
strict integrations, locked offline installation, and installed doctor. The
log is `target/jj-0.46.0-rust-1.97.1-full.log`. Both workflows passed Actionlint
1.7.12. Blocking integration, latest-jj advisory, and release verification now
retain logs and terminal transcripts as seven-day Actions artifacts. Release
builds check binary startup on each native runner before packaging; a manual
workflow run verifies all five archives without publishing. Hosted acceptance
must identify the exact resulting commit and its CI/release workflow runs.

Upstream sources: [text-editor substitutions and bulk descriptions](https://github.com/jj-vcs/jj/blob/v0.46.0/cli/src/description_util.rs),
[split description editing](https://github.com/jj-vcs/jj/blob/v0.46.0/cli/src/commands/split.rs),
[file editing](https://github.com/jj-vcs/jj/blob/v0.46.0/cli/src/commands/file/edit.rs),
and [upstream Rust requirement](https://github.com/jj-vcs/jj/blob/v0.46.0/Cargo.toml).

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
