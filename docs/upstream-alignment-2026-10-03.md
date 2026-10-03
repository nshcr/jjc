# Upstream alignment audit — 2026-10-03

## Versions and evidence

- GitHub's latest stable release is still [jj 0.45.1](https://github.com/jj-vcs/jj/releases/tag/v0.45.1),
  published on 2026-09-03, commit `7c41cdeb16b6b321c64e789a966b6adf723816a5`.
- The audited main snapshot is [`69abfbedcc615bb562c31d488b98abb1ab854089`](https://github.com/jj-vcs/jj/commit/69abfbedcc615bb562c31d488b98abb1ab854089),
  dated 2026-10-03. Main's changes below are **unreleased**, not a 0.46.0 release contract.
- `jjc` retains Rust 1.93.1 and jj 0.45.1 as its blocking baselines. Doctor,
  verification scripts, and CI/release installs already agree on them.
- Main was audited from a fixed source snapshot, not built or run locally.
  The available newest local Rust toolchain is 1.96.0; main now requires 1.97.1.
  This does not require raising jjc's MSRV: jjc uses the external-tool protocol,
  not jj's library crates. Main also raises its Git requirement to 2.42.0.
- The existing `latest-jj` advisory job installs the latest published `jj-cli`
  crate. It does not test upstream main.

## Branch scope

Stable-protocol correctness fixes are committed on `main`: the marker suffix
boundary fix and its regression tests, plus documentation of the existing
file-by-file diff limitation. The `dev` branch starts from that fixed `main`
and adds preparation for unreleased editor variables, combined descriptions,
and native OS path arguments. This full audit record lives on `dev`; it does
not make unreleased features part of main's support contract.

The main-only candidate passed the full local gate with 122 unit tests and
77 strict filesystem/real-jj/PTY tests. The counts at the end of this record
refer to the combined dev candidate, which also includes the main fix.

## Editor changes

### Optional line/path variables

Main's `TextEditor::edit_file()` substitutes `$path` and `$line` in configured
editor arguments. The line number is 1-based. It appends the path only when the
argument list does not already reference `$path`, so the existing
`editor = ["jjc", "edit"]` configuration remains valid.

`jjc edit --line <n> <file>` now supports that line hint. Zero, negative, and
non-numeric values are rejected; an out-of-range line is clamped to the last
line, including an empty file. The default is line 1. Positioning does not
modify file contents or normalize line endings.

For a jj build that implements the new variables, the opt-in configuration is:

```toml
[ui]
editor = ["jjc", "edit", "--line", "$line", "--", "$path"]
```

This configuration is **not for jj 0.45.1**. Doctor continues recommending the
stable configuration. CLI, buffer/editor, scripted binary, and real PTY tests
cover line positioning; substitution by an actual main binary is unverified.

### One split-description session

Main's `jj split` now sends both descriptions through
`edit_multiple_descriptions()` in one `.jjdescription` file. Its `JJ: describe`
headers already exist in stable `jj describe -r <revset>` bulk editing.

No new file profile or jj-internal description parser is needed. jjc preserves
headers and comment lines while editing; jj interprets them after save. A unit
fixture edits both description sections with CRLF endings. Real stable-jj
regressions save both descriptions unchanged and cancel with both commit IDs
and descriptions unchanged. These verify the shared bulk-description format,
not main's split transaction or its editor invocation count.

Source: [description editing](https://github.com/jj-vcs/jj/blob/69abfbedcc615bb562c31d488b98abb1ab854089/cli/src/description_util.rs)
and [split](https://github.com/jj-vcs/jj/blob/69abfbedcc615bb562c31d488b98abb1ab854089/cli/src/commands/split.rs).

## Diff and merge changes

### Native OS paths

Main replaces UTF-8-only temporary paths and string interpolation with native
`OsString` arguments. jjc already uses clap `PathBuf` fields for edit/diff/merge
filesystem paths. New Unix parsing tests pass invalid UTF-8 bytes through all
those arguments and assert exact preservation. This establishes argv parsing
compatibility only, not non-UTF-8 filesystem read/write behavior on every OS.
The merge `--path` remains a string because it is jj's UTF-8 repository path,
not one of the temporary filesystem paths.

### Marker boundary correction

The conflict codec in `lib/src/conflicts.rs` is byte-for-byte unchanged between
0.45.1 and the audited main snapshot. Its marker parser requires a repeated
marker prefix to be followed by ASCII whitespace or the end of the line.

The fix in this section is inherited from jjc's `main` branch, not introduced
as an unreleased-protocol feature on `dev`.

jjc previously accepted body text such as `>>>>>>>literal` as a closing marker.
A real jj 0.45.1 resolution could succeed while dropping that line and leaving
the actual closing marker in the resolved file. `is_marker_line()` now checks
that suffix boundary. Unit tests cover all four marker characters and reject
attached ASCII text, Unicode text, and non-ASCII whitespace; a real-jj regression
asserts the selected right side is preserved byte-for-byte and fully resolved.
Both new regressions failed before the fix and passed afterward.

The existing exact-marker-length policy is unchanged. This audit does not add
support for every marker shape accepted by jj's parser.

### Unchanged protocol limits

- The external merge argument names remain `$left`, `$base`, `$right`,
  `$output`, `$marker_length`, and `$path`.
- Empty or unchanged merge output is still rejected; removing `$output`
  still causes an I/O error, not deletion. Existing regressions remain valid.
- Diff editing still defaults to directory mode. `edit-invocation-mode =
"file-by-file"` sends file paths instead; jjc's diff route requires directories
  and does not support that optional mode. This is not a new main regression.
- No internal jj dependency, new deletion protocol, multi-side UI, or expanded
  Windows terminal support is introduced.

Sources: [stable conflict codec](https://github.com/jj-vcs/jj/blob/v0.45.1/lib/src/conflicts.rs),
[main conflict codec](https://github.com/jj-vcs/jj/blob/69abfbedcc615bb562c31d488b98abb1ab854089/lib/src/conflicts.rs),
[external tools](https://github.com/jj-vcs/jj/blob/69abfbedcc615bb562c31d488b98abb1ab854089/cli/src/merge_tools/external.rs),
and [diff working copies](https://github.com/jj-vcs/jj/blob/69abfbedcc615bb562c31d488b98abb1ab854089/cli/src/merge_tools/diff_working_copies.rs).

## Local validation

On macOS arm64 with Rust 1.93.1 and jj 0.45.1, `./scripts/verify.sh full` passed:

- Formatting, locked all-target/all-feature check and metadata, and Clippy with
  warnings denied.
- 126 unit tests, 8 tree-entry tests, 6 marker tests, 4 release-tag checks,
  33 smoke tests, and 34 PTY tests.
- The 81 filesystem/real-jj/PTY integration tests rerun with prerequisites
  required and the exact jj version enforced; none skipped.
- Locked offline installation, installed binary startup, and installed doctor
  reporting jj 0.45.1 as the tested protocol.

Independent review found no blocking issue in the new CLI/editor changes.
This is `verified-local` for the stable baseline. Linux/Windows, hosted CI, and
main runtime compatibility remain separate evidence that has not been obtained
in this audit. No baseline promotion follows from source inspection alone.
