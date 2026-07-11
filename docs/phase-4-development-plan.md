# jjc phase 4 development plan

Status: **complete within the supported local boundary**. All five slices are
implemented and the final local gate passed from the converged working tree.
The hosted GitHub Actions run is still external evidence and must not be claimed
until it has actually completed.

Started: 2026-07-10.

Phase 4 is the correctness-and-release-gate phase. It closes the local gaps that
previously prevented `jjc` from being trusted as one daily terminal editor for
the three Jujutsu (`jj`) editing surfaces. It does not turn `jjc` into a full
editor, an agent host, or an internal clone of `jj`.

## Goal contract

Phase 4 delivers:

- Safe diff decisions for regular files, missing entries, executable metadata,
  symlinks, and regular-file/symlink transitions.
- Reachable Git diff3 conflict-block editing from the recommended merge-tool
  config, including dynamic marker lengths and partial resolution.
- Grapheme-safe editing, terminal-cell-correct horizontal viewing, cached syntax
  rendering, and an explicit large-content fallback.
- Correct semantic profiles for description, sparse-pattern, and generic
  `ui.editor` files.
- A complete read-only `doctor` config path and a declared Rust/platform/`jj`
  compatibility gate.

Invariants:

- A selected supported diff decision materializes the right-side state in
  `$output`; an unselected decision materializes the left-side state.
- Directory-involving and special diff entries are rejected before any output
  mutation instead of being represented misleadingly.
- Diff output materialization never follows an output symlink or symlink
  ancestor.
- A cancel path never mutates protocol output.
- Rendering and viewport operations never change buffer bytes.
- Description-only policy applies only to `.jjdescription` inputs.
- Unsupported merge shapes remain explicit upstream protocol boundaries.

## Supported and deferred boundaries

Phase 4 does not:

- Link against `jj_lib` or reimplement `jj`'s built-in `scm-record` editor.
- Add an agent runtime, plugin system, network path, or background write path.
- Add Visual mode, macros, multiple registers, or a general search UI.
- Materialize directory-involving or special-file transitions in diff mode.
- Add horizontal panning to diff selection mode. Long diff content can be
  inspected through the existing rows and edited through manual-output mode.
- Introduce visible-range rendering or a buffer-revision cache. The implemented
  cache uses a full-content fingerprint; visible-range and revision-keyed caches
  are possible later optimizations, not Phase 4 completion requirements.
- Add support for merge conflict shapes that `jj` rejects before invoking an
  external tool or cannot return through one `$output` file.

The external merge non-goals include deletion as the result, non-normal entries,
unresolved executable-bit conflicts, file/directory conflicts, symlink
conflicts, and conflicts with more than two sides.

## Execution and evidence rules

The implementation order was:

1. P4.1 diff tree-entry correctness.
2. P4.2 merge marker configuration and real multi-block acceptance.
3. P4.3 terminal-cell rendering, horizontal viewport, and render caching.
4. P4.4 `ui.editor` semantic profiles and `doctor` convergence.
5. P4.5 CI, clippy, compatibility, and install gates.

Each protocol-affecting behavior requires a real-`jj` test where `jj` owns the
invocation. A skipped prerequisite is not passing evidence in the strict CI
job. Slice evidence and the final full gate are tracked separately: implemented
and focused-tested slices do not make the whole phase complete before the final
commands pass from the same working state.

## P4.1: Diff tree-entry correctness

Status: **implemented within the supported tree-entry boundary**.

### Implemented behavior

Diff discovery now snapshots each changed path once with `symlink_metadata` and
represents:

- a missing entry;
- a regular file with bytes and executable state;
- a symlink with its raw target;
- a directory classification;
- an unsupported special-entry classification.

The snapshot is shared by comparison, display, selection, and materialization,
so a path is not re-read later under different metadata. The scan does not
follow symlinked directories.

Supported decisions are:

- regular text and binary content;
- added/deleted regular files;
- executable-bit-only changes, independently from content hunks;
- added/deleted symlinks and symlink-target changes;
- regular-file to symlink and symlink to regular-file transitions.

Line, hunk, function, and manual editing remain regular UTF-8 content features.
Executable and entry-kind choices are explicit metadata/whole-entry hunks.

### Write safety

Before the first mutation, `write_output`:

1. Rejects every unsupported directory-involving or special entry.
2. Materializes all supported choices in memory.
3. Validates all output paths and rejects any symlink ancestor.

Only after full validation does it write supported file, symlink, or missing
states. Existing output symlinks are replaced without writing through their
targets. Directory transitions are deliberately unsupported; no recursive
delete/create ordering is claimed.

On Unix, executable state is the repository-relevant executable bit. On other
platforms, Unix executable and symlink behavior is not manufactured. The
platform matrix keeps the full tree-entry acceptance on Linux and macOS.

### Evidence

`tests/diff_tree_entries.rs` contains eight focused integration tests:

- select/unselect executable-only metadata;
- choose content and executable metadata independently;
- select/unselect a dangling symlink target;
- replace an output symlink without changing its target;
- reject an output symlink ancestor before any write;
- reject a file/directory transition before any write;
- real `jj split --tool jjc` leaves a mode-only change in the remainder;
- real `jj split --tool jjc` leaves a symlink-target change in the remainder.

Focused evidence: **8/8 passed locally**, including the two real-`jj 0.43.0`
flows. Existing regular text, binary, added/deleted, manual-edit, and cancel
coverage remains part of the full regression gate.

### Acceptance

- Supported output kind, content/target, and executable state match the chosen
  side.
- Mode and symlink decisions are proven through real `jj split`.
- Unsupported directory/special entries and unsafe output ancestry fail before
  partial output mutation.

## P4.2: Merge marker configuration and multi-block acceptance

Status: **implemented and accepted through real `jj` flows**.

### Implemented behavior

Every recommended `[merge-tools.jjc]` snippet now includes:

```toml
merge-tool-edits-conflict-markers = true
conflict-marker-style = "git"
```

This makes `jj` prefill `$output` with the Git diff3 markers used by `jjc`'s
block workflow. The parser uses the supplied `$marker_length` as an exact
delimiter length, rejects zero length, and requires a structurally valid
start/base/separator/end sequence. Shorter marker-like content remains literal
when `jj` lengthens its markers to avoid ambiguity.

The merge editor supports:

- `n`/`p` navigation across recognized blocks;
- left/base/right acceptance for the current block;
- different choices in different blocks;
- whole-side acceptance when no block is selected;
- a warning and explicit second confirmation when valid markers remain;
- returning remaining markers to `jj` as a partial resolution.

Snapshot/diff marker styles are not parsed because the per-tool supported style
is explicitly `git`.

### Evidence

Unit coverage verifies:

- dynamic marker lengths;
- exact-length matching that preserves shorter literal marker lines;
- malformed and incomplete sequences are not conflict blocks;
- zero marker length is rejected;
- block acceptance and remaining-marker confirmation.

`tests/merge_markers.rs` contains three real-`jj 0.43.0` acceptance tests, all
passing locally:

- two prefilled blocks accept different sides;
- a partial resolution round-trips through `jj resolve --list` and a second
  resolution pass;
- automatic marker lengthening preserves literal marker-like content.

The PTY suite also exercises the configured `jj resolve --tool jjc` route with
marker prefill and Git marker style.

### Acceptance

- Copying the recommended config makes conflict-block behavior reachable.
- Complete and partial multi-block resolution are proven by real `jj`.
- Marker lengthening cannot consume a shorter literal marker line.
- Upstream-rejected conflict shapes remain documented as upstream limits.

## P4.3: Terminal-cell rendering, horizontal viewport, and caching

Status: **implemented within the text-editing viewport boundary**.

### Implemented behavior

The shared buffer and renderer now provide:

- grapheme-safe left/right movement and deletion;
- grapheme-aware word boundaries;
- byte-offset to terminal-cell metrics for ASCII, CJK, combining sequences,
  emoji/ZWJ graphemes, and tabs;
- a fixed display-only tab width of four cells;
- horizontal offsets clamped to valid grapheme display boundaries;
- one shared `ViewScroll` horizontal viewport model.

Horizontal cursor-following is implemented for:

- `jjc edit`;
- diff manual-output editing;
- merge output, with the same comparison offset applied to left/base/right and
  output panes.

Diff selection mode intentionally does not pan horizontally. This is a product
boundary, not missing Phase 4 acceptance; long content becomes fully editable
after entering manual-output mode.

### Rendering cache and large-content fallback

`StyledTextCache` keys rendered lines by a fingerprint of path, complete
content, configuration, and instruction-dimming policy. It prevents repeated
Tree-sitter parsing while those inputs are unchanged and invalidates after
content or relevant config changes.

The cache still hashes complete content and clones cached rendered lines for a
redraw. It is not a visible-range or monotonic buffer-revision cache. Those are
future performance optimizations and are not represented as current behavior.

Syntax highlighting falls back to plain text above `512 KiB`. The fallback is
visible in status text where applicable and preserves the original bytes on
save.

### Evidence

Unit coverage verifies grapheme movement/deletion, CJK/combining/emoji/tab cell
metrics, tab expansion, grapheme-safe offsets, cache reuse/invalidation, and the
`512 KiB` threshold.

The PTY harness now:

- fixes the terminal at `24 x 100` cells;
- turns timeout and early EOF into explicit failures;
- replays the terminal stream before asserting visible screen content;
- verifies alternate-screen entry/exit and cursor-shape transitions.

The **19/19 PTY tests pass locally**. Phase 4-specific PTY evidence covers:

- Unicode long-line horizontal visibility in `edit`;
- horizontal viewing in diff manual-output editing;
- horizontal viewing in merge output;
- large-file plain fallback and byte-for-byte preservation;
- the existing edit/diff/merge and real-`jj` protocol routes.

### Acceptance

- Cursor-following uses terminal cells without splitting a grapheme.
- Edit, diff manual output, and merge long lines remain usable at a fixed
  terminal size.
- Unchanged content does not trigger repeated Tree-sitter parsing.
- Large-content fallback is explicit and preserves saved bytes.

## P4.4: `ui.editor` semantic convergence and `doctor`

Status: **implemented and accepted through real `jj` flows**.

### Implemented behavior

`jjc edit` infers one of three profiles from the temporary-file suffix:

- `.jjdescription`: dim `JJ:` instruction lines and require a second `:wq`
  before saving an empty description;
- `.jjsparse`: dim `JJ:` instructions if present, but allow an empty save;
- generic text: no description-only empty guard and no special treatment of an
  ordinary `JJ:` prefix.

All profiles preserve file bytes unless edited and keep non-zero `:q!` cancel
semantics.

`jjc doctor` remains read-only. It reports:

- the detected `jj` version;
- the current `jjc` executable path;
- the exact tested protocol baseline, `jj 0.43.0`;
- parseable recommended TOML for `ui.editor`, the named diff/merge tool routes,
  one resolved `merge-tools.jjc.program`, edit/merge arguments, marker prefill,
  and Git marker style.

### Evidence

Unit coverage verifies all profiles, description/sparse/generic empty-save
policy, generic `JJ:` rendering, template preservation, program-path escaping,
and the generated TOML structure.

Real-`jj` smoke coverage verifies:

- description edit/cancel/empty-warning behavior;
- intentionally saving an empty sparse-pattern set;
- extracting the doctor-generated TOML and using that same configuration to
  reach edit, diff, and merge routes in a temporary repository.

### Acceptance

- Generic and sparse editing are not blocked by commit-description policy.
- Commit descriptions retain the safety warning.
- The generated configuration is sufficient for all three supported editor
  routes.
- `doctor` never modifies user or repository configuration.

## P4.5: CI, clippy, compatibility, and install gates

Status: **workflow and local acceptance complete; hosted CI evidence pending**.

### Implemented compatibility contract

- Rust compatibility floor and CI toolchain: `1.93.1`.
- Pinned `jj` protocol baseline: `0.43.0`.
- Tier 1: Linux and macOS, including real `jj`, PTY, executable, and symlink
  tests.
- Tier 2: Windows build, clippy, portable unit/integration tests, and install
  verification; Unix tree-entry semantics are not claimed.

`Cargo.toml` now records description, repository, README, keywords, categories,
and `rust-version`. Public registry publication and license selection remain a
separate owner decision; Phase 4 does not infer legal authorization.

### Implemented workflow

`.github/workflows/ci.yml` contains:

- a Linux/macOS/Windows quality matrix running formatting, locked metadata,
  warnings-as-errors clippy, all-target tests, locked install, and installed
  binary checks;
- a Linux/macOS integration matrix installing `jj 0.43.0`, installing `jjc`,
  running installed `jjc doctor`, and running strict smoke/TTY/tree/marker tests;
- `JJC_REQUIRE_INTEGRATION=1`, which makes missing `jj` or `expect` a failure
  instead of a silent skip;
- a scheduled/manual non-blocking latest-`jj` advisory job.

The workflow definition is implemented, but a hosted GitHub Actions pass is an
external result and remains pending until observed.

### Local final gate

Run from the final working state:

```sh
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo metadata --locked --no-deps --format-version 1
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
JJC_REQUIRE_INTEGRATION=1 cargo test --locked \
  --test smoke --test tty --test diff_tree_entries --test merge_markers
cargo install --locked --path . --root /tmp/jjc-install-check --force
/tmp/jjc-install-check/bin/jjc --version
/tmp/jjc-install-check/bin/jjc doctor
```

The complete gate passed on macOS with Rust `1.93.1`, `jj 0.43.0`, and Expect
`5.45`. Results from the converged working tree were:

- formatting, locked all-target check, metadata, and warnings-as-errors clippy:
  passed;
- unit tests: **90/90**;
- diff tree-entry integrations: **8/8**;
- dynamic merge-marker integrations: **3/3**;
- real-`jj` smoke tests: **24/24**;
- fixed-size replayed PTY tests: **19/19**, plus two concurrent 19-test stress
  runs;
- locked clean-root install, installed `jjc --version`, and installed
  `jjc doctor`: passed.

## Evidence ledger

| Slice | State | Focused/local evidence | Remaining evidence |
| --- | --- | --- | --- |
| P4.1 tree entries | complete | 8/8 tree-entry integrations, including two real `jj 0.43.0` split flows; final gate passed | none |
| P4.2 merge markers | complete | dynamic/malformed unit coverage; 3/3 real `jj 0.43.0` marker flows; merge PTY; final gate passed | none |
| P4.3 rendering/cache | complete | Unicode/cache/fallback units; 19/19 fixed-size replayed PTY tests; final gate passed | none |
| P4.4 editor/doctor | complete | profile/TOML units; real sparse and doctor-generated three-route smoke; final gate passed | none |
| P4.5 CI/compatibility | locally complete | Rust 1.93.1 and jj 0.43.0 matrix encoded; strict local gate and installed doctor passed | hosted CI run |

Phase 4 is **complete within its supported local boundary**. Hosted CI remains
separately labeled external evidence until the branch is pushed and the
workflow succeeds.
