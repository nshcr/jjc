# jjc

[![CI](https://github.com/nshcr/jjc/actions/workflows/ci.yml/badge.svg)](https://github.com/nshcr/jjc/actions/workflows/ci.yml)
[![Rust 1.93.1+](https://img.shields.io/badge/rust-1.93.1%2B-orange.svg)](https://www.rust-lang.org)
[![Jujutsu](https://img.shields.io/badge/Jujutsu-0.43.0_baseline-blueviolet.svg)](https://jj-vcs.github.io/jj/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**One terminal-native editor for Jujutsu commit messages, interactive diffs, and merge conflicts.**

`jjc` plugs into all three of `jj`'s editing surfaces:

| `jj` setting | What `jjc` provides |
| --- | --- |
| `ui.editor` | A Vim-like commit message editor |
| `ui.diff-editor` | Interactive hunk, line, file, and function selection |
| `ui.merge-editor` | Three-way text conflict editing and binary side selection |

It is a single Rust binary with no GUI runtime. Text, diff, and merge views share
Tree-sitter syntax highlighting and Unicode-aware terminal rendering.

> [!IMPORTANT]
> `jjc` is experimental. The tested protocol baseline is `jj 0.43.0`; see
> [Current limits](#current-limits) before relying on it for unusual conflicts.

## Quick start

Requires Rust 1.93.1 or newer and [`jj`](https://jj-vcs.github.io/jj/latest/install-and-setup/).

```sh
cargo install --locked --git https://github.com/nshcr/jjc
jjc doctor
```

Prebuilt archives for Linux, macOS, and Windows are available from
[GitHub Releases](https://github.com/nshcr/jjc/releases). Each release includes
a `SHA256SUMS` file for artifact verification.

Add the configuration printed by `jjc doctor`, or copy this into your `jj`
config:

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

Try it in a repository:

```sh
jj describe              # commit message editor
jj diffedit --tool jjc   # interactive diff editor
jj resolve --tool jjc    # merge editor
```

`jj restore -i`, `jj split`, and `jj squash -i` are also covered by the real
`jj` integration suite.

## Highlights

- Vim-like editing with motions, operators, undo/redo, yanking, and find motions
- Whole-hunk, line-level, file-level, and Rust function-aware diff selection
- Added/deleted files, executable-bit changes, symlinks, and binary diffs
- Per-conflict-block resolution for ordinary UTF-8 three-way merges
- Binary merge resolution by choosing the left, base, or right side
- Tree-sitter highlighting for C, C++, Go, JavaScript, JSON, Python, Rust,
  TypeScript, TSX, and JSX
- Correct cursor and horizontal scrolling behavior for CJK text, emoji,
  combining marks, tabs, and long lines
- Shared structured edit-command layer for future agent integrations

## Commands

```text
jjc doctor
jjc edit <file>
jjc diff <left> <right> <output>
jjc merge <left> <base> <right> <output> --marker-length <n> --path <repo-path>
```

## Key reference

<details>
<summary>Text editor keys</summary>

- Enter insert mode: `i`, `I`, `a`, `A`, `o`, `O`
- Return to normal mode: `Esc`
- Move: `h`, `j`, `k`, `l`, `0`, `^`, `$`, `g_`, `w`, `W`, `b`, `B`, `e`,
  `E`, `ge`, `gE`, `gg`, `G`
- Find and match: `%`, `f`, `F`, `t`, `T`, `;`, `,`
- Edit: `x`, `X`, `D`, `C`, `dd`, `cc`, `s`, `S`, `r`, `J`
- Yank and paste: `yy`, `Y`, `p`, `P`
- Case: `~`, `guu`, `gUU`, `g~~`
- Operators and text objects include `dw`, `cw`, `yw`, `d$`, `c$`, `y$`,
  `df`, `ct`, `yf`, `ciw`, `diw`, `yiw`, `guw`, `gUw`, and `g~w`
- Undo and redo: `u`, `Ctrl-r`
- Save and quit: `:wq`; discard: `:q!`

</details>

<details>
<summary>Diff editor keys</summary>

- Move between hunks: `j`, `k`
- Move between files: `[`, `]`
- Move inside an expanded hunk: `n`, `p`, `PageUp`, `PageDown`
- Toggle the current hunk: `Space`
- Toggle the current line or replacement pair: `x`
- Select or deselect the current file: `S`, `D`
- Toggle the current Rust function: `f`
- Undo or redo selection changes: `u`, `r`
- Manually edit the current file output: `e`
- Write output: `w`; cancel: `q`

</details>

<details>
<summary>Merge editor keys</summary>

- Move between conflict blocks: `n`, `p`
- Accept left, base, or right: `1`, `2`, `3`
- Write text output: `:wq`; cancel: `q`
- For binary conflicts, choose a side with `1`, `2`, or `3`, then write with `w`

</details>

## Configuration

`jjc` reads `$JJC_CONFIG` when set. Otherwise it checks
`$XDG_CONFIG_HOME/jjc/config.toml`, then `$HOME/.config/jjc/config.toml`.
Missing files use defaults.

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

Colors accept terminal names such as `cyan`, `yellow`, `green`, `magenta`,
`blue`, `gray`, and `dark-gray`, or `#rrggbb` values. Unsupported file
extensions fall back to plain text.

## Compatibility and verification

CI builds the library and binary on Linux, macOS, and Windows. Linux and macOS
also run real `jj` and PTY integration tests. A scheduled advisory job probes
the latest available `jj` without making that version part of the support claim.

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
JJC_REQUIRE_INTEGRATION=1 cargo test --locked \
  --test smoke --test tty --test diff_tree_entries --test merge_markers
```

## Current limits

- The external `jj` merge-tool protocol cannot currently express deletion as
  the merge result through `jjc`.
- `jj` rejects some non-normal-file and unresolved executable-bit conflicts
  before invoking an external merge tool.
- Visual mode, cross-line motion ranges, broader text objects, macros,
  file/directory conflicts, symlink conflicts, multi-side conflict UI, and the
  actual agent runtime are not implemented yet.
- Additional Tree-sitter languages require a grammar crate and registry entry.

For design details and planned work, see the
[development roadmap](docs/development-plan.md). The
[Phase 4 plan](docs/phase-4-development-plan.md) records the current correctness
and release gates.

## Development

```sh
git clone https://github.com/nshcr/jjc.git
cd jjc
cargo test --locked --all-targets --all-features
cargo run -- doctor
```

Contributions and focused bug reports are welcome. If an issue depends on `jj`
behavior, include the output of `jj --version` and `jjc doctor`.

## License

`jjc` is available under the [MIT License](LICENSE).
