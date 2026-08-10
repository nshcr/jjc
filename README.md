# jjc

[![CI](https://github.com/nshcr/jjc/actions/workflows/ci.yml/badge.svg)](https://github.com/nshcr/jjc/actions/workflows/ci.yml)
[![Rust 1.93.1+](https://img.shields.io/badge/rust-1.93.1%2B-orange.svg)](https://www.rust-lang.org)
[![Jujutsu](https://img.shields.io/badge/Jujutsu-0.44.0_baseline-blueviolet.svg)](https://docs.jj-vcs.dev/)
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
> `jjc` is experimental. The tested protocol baseline is `jj 0.44.0`; see
> [Current limits](#current-limits) before relying on it for unusual conflicts.

## Quick start

Requires Rust 1.93.1 or newer and [`jj`](https://docs.jj-vcs.dev/latest/install-and-setup/).

```sh
cargo install --locked --git https://github.com/nshcr/jjc
jjc doctor
```

Tagged releases publish prebuilt archives for Linux, macOS, and Windows on
[GitHub Releases](https://github.com/nshcr/jjc/releases). Each release includes
a `SHA256SUMS` file and build-provenance attestations. If no tagged release is
listed yet, install the locked source build shown above.

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
- One-key “only this hunk/line” choices, global selection presets, and long-line
  panning without entering manual edit mode
- Added/deleted files, executable-bit changes, symlinks, and binary diffs
- Per-conflict-block resolution for ordinary UTF-8 three-way merges
- Automatic next-conflict focus, conflict progress, and safe all-left/base/right
  commands for repetitive resolutions
- Binary merge resolution by choosing the left, base, or right side
- Tree-sitter highlighting for C, C++, Go, JavaScript, JSON, Python, Rust,
  TypeScript, TSX, and JSX
- Correct cursor and horizontal scrolling behavior for CJK text, emoji,
  combining marks, tabs, and long lines
- In-app `?` / `F1` help plus consistent `Ctrl-S` save and `Ctrl-C` cancel
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
- Save and quit: `:wq` or `Ctrl-S`; discard: `:q!` or `Ctrl-C`
- In-app help: `?` or `F1`

</details>

<details>
<summary>Diff editor keys</summary>

- Move between hunks: `j`, `k`
- Move between files: `[`, `]`
- Move between changed lines inside an expanded hunk: `n`, `p`, `PageUp`,
  `PageDown`
- Pan a long diff line: `h`, `l`, `Left`, `Right`
- Toggle the current hunk: `Space`; toggle and advance: `Enter`
- Toggle the current line or replacement pair: `x`
- Select or deselect the whole diff: `s`, `d`
- Select or deselect the current file: `S`, `D`
- Keep only the current hunk or changed line: `o`, `O`
- Toggle the current Rust function: `f`
- Undo or redo selection changes: `u`, `r`
- Manually edit the current file output: `e`
- Write output: `w` or `Ctrl-S`; cancel: `q` or `Ctrl-C`
- In-app help: `?` or `F1`

</details>

<details>
<summary>Merge editor keys</summary>

- Move between conflict blocks: `n`, `p`
- Accept left, base, or right and focus the next conflict: `1`, `2`, `3`
- Accept one side for all remaining blocks: `:al` / `:all-left`, `:ab` /
  `:all-base`, `:ar` / `:all-right`
- Write text output: `:wq`, `Ctrl-S`, or `Enter` once all parsed conflicts
  are resolved; cancel: `q` or `Ctrl-C`
- For binary conflicts, choose a side with `1`, `2`, or `3`, then write with `w`
- In-app help: `?` or `F1`

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
The current version and platform contract is in the
[support policy](docs/support.md).

```sh
./scripts/verify.sh fast
./scripts/verify.sh full
```

The full gate is Linux/macOS-only and requires Rust 1.93.1, `jj 0.44.0`, and
Expect. It rejects missing prerequisites and protocol-version drift instead of
silently skipping integration coverage.

## Current limits

- The external `jj` merge-tool protocol cannot currently express deletion as
  the merge result through `jjc`. Empty output therefore requires a second
  confirmation and means “empty regular file,” not “delete this path.”
- `jj` rejects some non-normal-file and unresolved executable-bit conflicts
  before invoking an external merge tool.
- Visual mode, cross-line motion ranges, broader text objects, macros,
  file/directory conflicts, symlink conflicts, multi-side conflict UI, and the
  actual agent runtime are not implemented yet.
- Additional Tree-sitter languages require a grammar crate and registry entry.

For design details and planned work, see the
[development roadmap](docs/development-plan.md). The
[Phase 5 plan](docs/phase-5-terminal-ux-plan.md) records the current terminal UX
and local acceptance gates; [Phase 4](docs/phase-4-development-plan.md) remains
the historical correctness baseline.

## Development

```sh
git clone https://github.com/nshcr/jjc.git
cd jjc
./scripts/verify.sh fast
cargo run -- doctor
```

Contributions and focused bug reports are welcome. If an issue depends on `jj`
behavior, include the output of `jj --version` and `jjc doctor`.

See the [documentation index](docs/README.md) for support, engineering
governance, roadmap, and release procedures.

## License

`jjc` is available under the [MIT License](LICENSE).
