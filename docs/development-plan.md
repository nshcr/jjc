# jjc development plan

`jjc` is a Rust TUI editor for Jujutsu (`jj`). Its long-term goal is to become
one terminal-native editor that can serve all three `jj` editing surfaces:

- commit message editor: `ui.editor`
- diff editor: `ui.diff-editor`
- merge editor: `ui.merge-editor`

The early goal is personal daily usability. After the editor is stable enough
for regular use, the project should be prepared as a public general-purpose
tool for `jj`. The long-term design should keep the door open for possible
upstream integration into `jj` as a single canonical editor.

## Non-goals for the first pass

- Do not link against `jj_lib` yet.
- Do not reimplement `jj`'s built-in `scm-record` editor.
- Do not support every conflict shape in the first merge editor.
- Do not add a plugin system, agent runtime, or Tree-sitter layer before the
  basic editor protocols are reliable.

## External protocol

The first stable CLI shape is:

```sh
jjc edit <file>
jjc diff <left> <right> <output>
jjc merge <left> <base> <right> <output> --marker-length <n> --path <repo-path>
```

Recommended `jj` config:

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
```

The diff editor intentionally requires the 3-pane `$output` protocol. The
2-pane `$left`/`$right` mode is skipped until there is a concrete reason to
support it.

## Product priorities

Implementation priority:

1. Commit message editor
2. Diff editor
3. Merge editor

Acceptance priority for the first usable milestone is still all three entry
points: each mode must have a thin working version before the project is treated
as usable.

## Interaction model

The UI should feel like a small terminal tool, not a full IDE.

- Text editing follows a minimal Vim model.
- Panel/list navigation follows a lazygit-style model.
- First-pass Vim support prioritizes high-frequency `normal` and `insert`
  commands.
- `visual` mode is planned as a later layer after the shared text editor is
  stable in all three entry points.
- If a good Rust library can provide mature Vim editing behavior without
  forcing a large framework, use it instead of hand-rolling Vim semantics.

Current Vim-compatible commands:

- `Esc`: return to normal mode
- `i`, `I`, `a`, `A`, `o`, `O`: enter insert mode
- `h`, `j`, `k`, `l`, `0`, `^`, `$`, `g_`, `w`, `W`, `b`, `B`, `e`, `E`, `ge`, `gE`, `gg`, `G`: move
- `f`, `F`, `t`, `T`, `;`, `,`: find on the current line
- `x`, `X`, `D`, `C`, `dd`, `cc`, `s`, `S`, `r`, `J`: delete/change/replace/join
- `yy`, `Y`, `p`, `P`: line yank and paste
- `~`, `guu`, `gUU`, `g~~`: case conversion
- `dw`, `cw`, `yw`, `d$`, `c$`, `y$`, `df`, `ct`, `yf`, `guw`, `gUw`, `g~w`, `gUf`, `g~t`: motion ranges
- `u`, `Ctrl-r`: undo and redo
- Insert mode supports ordinary text, `Enter`, `Backspace`, `Delete`,
  `Ctrl-h`, `Ctrl-w`, `Ctrl-u`, `Ctrl-[`, and `Ctrl-c`
- `:wq`: save and quit
- `:q!`: cancel the editor and exit non-zero

Deferred Vim-compatible commands:

- Visual mode
- `%` matching
- cross-line motion ranges
- text objects such as `ciw`, `di(`, and `yap`
- macros and registers beyond the single built-in yank buffer

## Dependency policy

Prefer standard library code where it is enough, but do not rebuild solved
pieces.

Allowed early dependencies:

- `clap` for CLI parsing
- `similar` for text diff and hunk generation
- current TUI stack: `ratatui` and `crossterm`

Tree-sitter is now used for function-aware hunks and shared syntax
highlighting. Keep grammar support explicit: each bundled language needs a
grammar crate and one registry entry.

Defer heavier dependencies until they unlock a concrete feature:

- agent/runtime dependencies: commit-message assistance and later guided edits

## Architecture

Keep modules boring and protocol-first:

```text
src/
  main.rs          CLI dispatch and top-level error handling
  cli.rs           clap definitions
  app.rs           shared TUI loop and terminal cleanup guard
  editor.rs        text buffer, cursor, normal/insert modes
  edit.rs          commit message editor mode
  diff.rs          diff editor mode
  merge.rs         merge editor mode
  fs.rs            small file/path helpers
```

Do not add abstractions before a second mode actually needs them. Shared code
should start as small helpers, then move only when duplication becomes real.

## Milestones

### M0: Protocol shell

Scope:

- Add `clap` subcommands for `edit`, `diff`, and `merge`.
- Validate paths and required arguments.
- Enter the TUI for each mode.
- Ensure terminal cleanup always runs after normal exit or error.

Acceptance:

- `jjc edit <file>` opens the editor mode.
- `jjc diff <left> <right> <output>` opens the diff mode.
- `jjc merge <left> <base> <right> <output> --marker-length 7 --path file.rs`
  opens the merge mode.
- Invalid arguments return non-zero with a useful error.
- `cargo check` passes.

### M1: Commit message editor

Scope:

- Built-in text editing for UTF-8 files.
- Shared Vim-like `normal`/`insert` behavior.
- `:wq` writes the file and exits.
- `:q!` exits without writing and returns non-zero so `jj` cancels the edit.
- `JJ:` comment lines may be visually muted, but the file should be preserved
  exactly unless the user edits it. Let `jj` decide how to interpret comments.

Acceptance:

- `jj describe --config 'ui.editor=["jjc","edit"]'` can edit a commit message.
- Saving writes the exact buffer to disk.
- Discarding leaves the file unchanged.
- The same text editing core is used by commit messages, merge text output, and
  diff manual output editing.

### M2: Diff editor with hunk selection

Scope:

- Require `$left`, `$right`, and `$output`.
- Treat `$output` as initially equal to `$right`.
- Generate line-based hunks with `similar`.
- Toggle whole hunks between selected and unselected.
- Selected hunk means keep the `$right` version in `$output`.
- Unselected hunk means restore the `$left` version in `$output`.
- Ignore `JJ-INSTRUCTIONS`.
- Start with UTF-8 text files; binary files get a clear unsupported message.

Acceptance:

- `jj split` or `jj squash -i` can invoke `jjc`.
- Hunk toggles are reflected in `$output`.
- `jj` reads `$output` and applies the selected hunks correctly.

Deferred:

- Line-level selection
- Function-level selection
- Syntax-aware grouping
- Manual editing inside a diff hunk
- 2-pane diff editor mode

### M3: Merge editor for simple text conflicts

Scope:

- Support ordinary UTF-8 three-way text conflicts.
- Show left/base/right/output panes.
- Allow accept-left, accept-base, accept-right, and manual output editing.
- Save only to `$output`.
- Preserve `--marker-length` and `--path` in the model for later conflict-marker
  compatibility.

Acceptance:

- `jj resolve --tool jjc <path>` can resolve a simple text conflict.
- Saving writes a complete `$output` file.
- Discarding exits non-zero so `jj` cancels the resolution.

First-pass unsupported cases:

- multi-side conflicts
- binary or non-UTF-8 conflicts
- file/directory conflicts
- symlink or submodule conflicts
- executable-bit-only conflicts
- delete/modify conflicts

Unsupported cases must fail clearly and must not write misleading output.

### M4: Daily-use hardening

Scope:

- Add focused tests for CLI parsing, file writes, hunk application, and terminal
  cleanup.
- Add minimal integration smoke tests using temporary `jj` repos.
- Improve error messages for unsupported files and malformed inputs.
- Add README setup instructions.

Acceptance:

- `cargo test` passes.
- Smoke tests cover `describe`, `split` or `squash -i`, and `resolve`.
- The tool can be installed with `cargo install --path .` and configured in
  `jj`.

### M5: Fine-grained diff editing

Scope:

- Add line-level selection inside hunks.
- Add optional manual editing of `$output` for a selected file.
- Add undo/redo for diff selection operations.

Acceptance:

- The user can split a change at line granularity without leaving the TUI.

### M5.5: Vim hardening before Visual mode

Scope:

- Keep one shared text editing core for `edit`, merge text output, and diff
  manual output.
- Add only high-frequency `normal`/`insert` commands before Visual mode.
- Keep command tests small and scriptable with `JJC_KEYS`.

Acceptance:

- Shared Vim command tests cover movement, delete/change, yank/paste, undo/redo,
  and insert-mode control deletes.
- Smoke tests cover `jj describe`, diff editing, and text/binary merge paths.
- `cargo test` passes.

### M6: Tree-sitter function hunks and syntax highlighting

Scope:

- Add Tree-sitter parsers through an explicit language registry.
- Group hunks by enclosing function when possible.
- Fall back to line hunks for unknown languages or parse failures.
- Highlight edit, diff, and merge text views through one shared renderer.
- Add jjc config for enabling/disabling syntax highlighting and overriding
  per-class theme styles.

Acceptance:

- A complete function can be selected as one logical hunk.
- Parse failures never block line-based editing.
- Rust, Python, Go, JavaScript, TypeScript/TSX, JSON, C, and C++ are supported
  by bundled grammar crates.
- `jjc edit`, `jjc diff`, and `jjc merge` all use the same configured
  highlighter for recognized file extensions.
- Non-Rust highlighting and theme parsing have focused unit tests.

### M7: Complex merge compatibility

Handle unsupported conflict shapes one by one:

- delete/modify conflicts: choose delete or keep modified output
- binary conflicts: accept left/base/right only
- file mode conflicts: expose explicit mode choices
- file/directory conflicts: choose path-level resolution
- symlink conflicts: accept side or edit target as text only when safe
- multi-side conflicts: design a side list UI instead of pretending it is a
  normal three-way conflict

Important external-tool boundary:

- The current `jj` external merge-tool protocol always consumes a single
  `$output` file. `jj` treats empty or unchanged output as an error, so deletion
  cannot be represented faithfully by this external binary alone.
- For delete/modify conflicts, this external binary can keep the modified side,
  but choosing the deleted side still needs upstream/internal integration.
- `jj` rejects non-normal-file conflicts, conflicts with more than two sides,
  and unresolved executable-bit conflicts before invoking the external merge
  tool. Full support for those cases requires either upstream `jj` changes or a
  future internal integration path.
- The external binary can still support binary normal-file conflicts by accepting
  left/base/right bytes and writing the selected side to `$output`.

Acceptance:

- Each added conflict shape has a test fixture and a clear UI action.

### M8: Agent-ready editor layer

Scope:

- Keep the editor buffer and command model independent from the terminal UI.
- Add a structured command API that an agent can call later.
- Start with commit message assistance before code/diff mutation.

Acceptance:

- Agent suggestions can be applied as explicit buffer edits.
- No background agent action writes files without user confirmation.

Implementation boundary:

- The current layer should stop at structured buffer commands and explicit
  suggestion application. Do not add an agent runtime, network calls, or
  background file writes until commit-message assistance has a concrete UX.

## Testing strategy

Use the smallest tests that catch real regressions:

- Unit tests for path validation and command parsing.
- Unit tests for text buffer operations.
- Unit tests for hunk toggle to `$output` reconstruction.
- Integration tests with temporary directories for diff/merge file protocols.
- Smoke tests with real `jj` commands after the protocol modes exist.

Do not build a large TUI automation suite until the UI stabilizes.

## First implementation slice

Start with M0 and the smallest useful piece of M1:

1. Add `clap`.
2. Split `main.rs` into CLI dispatch plus app loop.
3. Implement `jjc edit <file>` with load/save/discard.
4. Keep `diff` and `merge` as protocol-validated placeholder screens.
5. Run `cargo check`.

This gives all three entry points a real shape while making the highest-priority
mode useful first.
