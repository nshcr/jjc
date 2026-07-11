# jjc

`jjc` is a Rust terminal editor for Jujutsu (`jj`). It is being built as one
binary for three `jj` editing surfaces:

- commit message editor: `ui.editor`
- diff editor: `ui.diff-editor`
- merge editor: `ui.merge-editor`

This is still early. The current build has a Vim-like built-in text editor,
whole-hunk and line-level diff selection, added/deleted file handling, binary
diff accept-side choices, Rust function-aware diff grouping, simple three-way
UTF-8 merge output editing, binary merge accept-side resolution, and an
agent-ready structured edit command layer for the built-in text buffer. Edit,
diff, and merge text views share Tree-sitter syntax highlighting.

## Install

```sh
cargo install --path .
```

## Configure jj

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

The last two settings make `jj` prefill merge output with Git diff3 conflict
markers, which enables `jjc`'s per-conflict-block navigation and partial
resolution flow.

## Commands

```sh
jjc doctor
jjc edit <file>
jjc diff <left> <right> <output>
jjc merge <left> <base> <right> <output> --marker-length <n> --path <repo-path>
```

Run `jjc doctor` after installation to check that `jj` is available and print a
ready-to-copy `jj` configuration for all three editor surfaces.

## Syntax highlighting

Syntax highlighting is enabled by default in:

- `jjc edit <file>`
- `jjc diff <left> <right> <output>` hunk rows
- diff mode manual output editing via `e`
- `jjc merge <left> <base> <right> <output>` left/base/right/output panes

Supported bundled Tree-sitter languages:

- C: `.c`, `.h`
- C++: `.cc`, `.cpp`, `.cxx`, `.hpp`, `.hh`, `.hxx`
- Go: `.go`
- JavaScript: `.js`, `.mjs`, `.cjs`
- JSON: `.json`
- Python: `.py`, `.pyw`
- Rust: `.rs`
- TypeScript: `.ts`, `.mts`, `.cts`
- TSX/JSX: `.tsx`, `.jsx`

Unsupported extensions fall back to plain text.

## Configure jjc

`jjc` reads configuration from `JJC_CONFIG` when set. Otherwise it checks
`$XDG_CONFIG_HOME/jjc/config.toml`, then `$HOME/.config/jjc/config.toml`.
Missing config files use defaults.

```toml
[syntax]
enabled = true

[theme.keyword]
fg = "cyan"
bold = true

[theme.function]
fg = "yellow"

[theme.string]
fg = "green"

[theme.comment]
fg = "dark-gray"
dim = true

[theme.number]
fg = "magenta"

[theme.type-name]
fg = "blue"
```

Colors can be named terminal colors such as `cyan`, `yellow`, `green`,
`magenta`, `blue`, `gray`, `dark-gray`, or `#rrggbb`.

## Keys

Text editing:

- `i`, `I`, `a`, `A`, `o`, `O`: insert
- `Esc`: normal mode
- `h`, `j`, `k`, `l`, `0`, `^`, `$`, `g_`, `w`, `W`, `b`, `B`, `e`, `E`, `ge`, `gE`, `gg`, `G`: move
- `%`: jump between paired brackets on the current line
- `f`, `F`, `t`, `T`, `;`, `,`: find on the current line
- `x`, `X`, `D`, `C`, `dd`, `cc`, `s`, `S`, `r`, `J`: delete/change/replace/join
- `yy`, `Y`, `p`, `P`: line yank and paste
- `~`, `guu`, `gUU`, `g~~`: case conversion
- `dw`, `cw`, `yw`, `d$`, `c$`, `y$`, `df`, `ct`, `yf`, `ciw`, `diw`, `yiw`, `guw`, `gUw`, `g~w`, `gUf`, `g~t`: motion ranges and inner-word text objects
- `u`, `Ctrl-r`: undo and redo
- Insert mode: `Backspace`, `Delete`, `Ctrl-h`, `Ctrl-w`, `Ctrl-u`, `Ctrl-[`, `Ctrl-c`
- `:wq`: save and quit
- `:q!`: quit without saving

Diff mode:

- `j`, `k`: move between hunks
- `[`, `]`: move between changed files
- `n`, `p`, `PageUp`, `PageDown`: move inside the expanded hunk
- `Space`: toggle hunk
- `x`: toggle the current changed line or replacement pair
- `S`, `D`: select or deselect the current file
- `f`: toggle all hunks associated with the current function
- `u`, `r`: undo or redo selection changes
- expanded hunks show standard `space`, `+`, and `-` diff rows
- `e`: manually edit the current file output with the same Vim-like text editor
- `w`: write `$output` and quit
- `q`: quit without writing

Merge mode:

- `n`, `p`: move between conflict-marker blocks in the output
- `1`, `2`, `3`: accept left/base/right for the current conflict block, or copy
  the whole side when the cursor is not inside a recognized block
- text output uses the same Vim-like text editor
- `:wq`: write `$output` and quit
- `q`: cancel merge

For binary conflicts, manual editing is disabled. Use `1`, `2`, or `3` to pick
left/base/right, then `w` to write the selected side.

For `.jjdescription` inputs, `jjc edit` dims `JJ:` instruction lines and warns
before saving an empty commit description. `.jjsparse` inputs retain the
instruction styling but allow an empty pattern set; generic `ui.editor` files
receive neither description-only behavior. Save a warned description again to
intentionally write it empty.

Text cursors use grapheme boundaries and terminal cell widths. Long lines,
wide CJK characters, emoji, combining marks, and tabs keep the cursor visible
through a shared horizontal viewport without changing the underlying bytes.

## Supported jj flows

The smoke and TTY tests cover these `jj` entry points:

- `jj describe --editor` through `ui.editor`
- `jj diffedit --tool jjc`
- `jj restore -i --tool jjc`
- `jj split --tool jjc`
- `jj squash -i --tool jjc`
- `jj resolve --tool jjc <path>`

Release check:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
JJC_REQUIRE_INTEGRATION=1 cargo test --locked --test smoke --test tty --test diff_tree_entries --test merge_markers
cargo install --locked --path . --root /tmp/jjc-install-check --force
```

The current compatibility floor is Rust 1.93.1. CI compiles the library and
binary on Linux, macOS, and Windows, and runs the real `jj` plus PTY suite on
Linux and macOS. The current local protocol baseline is `jj 0.43.0`; older jj
versions are not claimed until they are added to the compatibility matrix.

## Current limits

- Diff mode supports whole-hunk and line-level selection, added/deleted files,
  executable-bit and symlink entry choices, file-level navigation and
  selection, binary accept-side choices, plus manual editing of modified UTF-8
  file output.
- Merge mode supports ordinary UTF-8 three-way text conflicts and binary
  accept-side resolution. Recognized conflict-marker blocks can be resolved one
  block at a time. For delete/modify conflicts, it can keep the modified side.
- Syntax highlighting is limited to bundled grammar crates; adding another
  Tree-sitter language requires adding its grammar crate and language registry
  entry.
- The current external `jj` merge-tool protocol does not let `jjc` correctly
  express deletion as the merge result; `jj` also rejects some non-normal-file
  and unresolved executable-bit conflicts before invoking the external tool.
  Those require upstream/internal `jj` integration rather than only this external
  binary.
- Visual mode, cross-line motion ranges, broader text objects, macros,
  file/directory conflicts, symlink conflicts, multi-side conflict UI, and the
  actual agent runtime are planned later.

See [`docs/development-plan.md`](docs/development-plan.md) for the converged
roadmap and [`docs/phase-4-development-plan.md`](docs/phase-4-development-plan.md)
for the current correctness and release-gate plan.
