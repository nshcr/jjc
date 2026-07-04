# jjc

`jjc` is a Rust terminal editor for Jujutsu (`jj`). It is being built as one
binary for three `jj` editing surfaces:

- commit message editor: `ui.editor`
- diff editor: `ui.diff-editor`
- merge editor: `ui.merge-editor`

This is still early. The current build has a Vim-like built-in text editor,
whole-hunk and line-level diff selection for UTF-8 files present on both sides,
Rust function-aware diff grouping, simple three-way UTF-8 merge output editing,
binary merge accept-side resolution, and an agent-ready structured edit command
layer for the built-in text buffer. Edit, diff, and merge text views share
Tree-sitter syntax highlighting.

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
jjc edit <file>
jjc diff <left> <right> <output>
jjc merge <left> <base> <right> <output> --marker-length <n> --path <repo-path>
```

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
- `f`, `F`, `t`, `T`, `;`, `,`: find on the current line
- `x`, `X`, `D`, `C`, `dd`, `cc`, `s`, `S`, `r`, `J`: delete/change/replace/join
- `yy`, `Y`, `p`, `P`: line yank and paste
- `~`, `guu`, `gUU`, `g~~`: case conversion
- `dw`, `cw`, `yw`, `d$`, `c$`, `y$`, `df`, `ct`, `yf`, `guw`, `gUw`, `g~w`, `gUf`, `g~t`: motion ranges
- `u`, `Ctrl-r`: undo and redo
- Insert mode: `Backspace`, `Delete`, `Ctrl-h`, `Ctrl-w`, `Ctrl-u`, `Ctrl-[`, `Ctrl-c`
- `:wq`: save and quit
- `:q!`: quit without saving

Diff mode:

- `j`, `k`: move between hunks
- `Space`: toggle hunk
- expanded hunks show standard `space`, `+`, and `-` diff rows
- `e`: manually edit the current file output with the same Vim-like text editor
- `w`: write `$output` and quit
- `q`: quit without writing

Merge mode:

- `1`, `2`, `3`: copy left/base/right into output
- text output uses the same Vim-like text editor
- `:wq`: write `$output` and quit
- `q`: cancel merge

For binary conflicts, manual editing is disabled. Use `1`, `2`, or `3` to pick
left/base/right, then `w` to write the selected side.

## Current limits

- Diff mode supports whole-hunk and line-level selection, plus manual editing
  of the current file output.
- Diff mode supports UTF-8 files that exist on both left and right sides.
- Merge mode supports ordinary UTF-8 three-way text conflicts and binary
  accept-side resolution. For delete/modify conflicts, it can keep the modified
  side.
- Syntax highlighting is limited to bundled grammar crates; adding another
  Tree-sitter language requires adding its grammar crate and language registry
  entry.
- The current external `jj` merge-tool protocol does not let `jjc` correctly
  express deletion as the merge result; `jj` also rejects some non-normal-file
  and unresolved executable-bit conflicts before invoking the external tool.
  Those require upstream/internal `jj` integration rather than only this external
  binary.
- `%` matching, Visual mode, cross-line motion ranges, text objects, macros,
  file/directory conflicts, symlink conflicts, multi-side conflict UI, and the
  actual agent runtime are planned later.
