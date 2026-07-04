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
```

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
- `Space`: toggle hunk
- `S`, `D`: select or deselect the current file
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

`jjc edit` dims `JJ:` comment lines and warns before saving an empty commit
message. Save again to intentionally write the empty message.

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
cargo check
cargo test
cargo install --path . --root /tmp/jjc-install-check --force
```

## Current limits

- Diff mode supports whole-hunk and line-level selection, added/deleted files,
  file-level navigation and selection, binary accept-side choices, plus manual
  editing of modified UTF-8 file output.
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
