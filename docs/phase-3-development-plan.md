# jjc phase 3 development plan

Phase 3 starts from the implemented Phase 2 baseline: `jjc edit`, `jjc diff`,
and `jjc merge` already work as real `jj` external editor entry points. The
goal is to make `jjc` safe and convenient enough to set as the default daily
`jj` editor without turning it into a full editor, agent host, or `jj`
internal clone.

## Historical status and Phase 4 relationship

Phase 3 records implemented feature slices, but its original “default daily
editor” closure was broader than the evidence ultimately supported. A later
code/config review found three gaps:

- `doctor` did not emit the merge marker settings required by the implemented
  conflict-block parser.
- `jjc edit` treated the general `ui.editor` route as if every input were a
  commit description.
- The release command list was not an enforced CI, clippy, pinned-`jj`, and
  platform compatibility gate.

[Phase 4](phase-4-development-plan.md) supersedes those closure claims while
preserving the Phase 3 implementation history. Its implementation now closes
the three local gaps; the final local full gate and hosted CI result remain
pending evidence. P3.2 navigation and the P3.3/P3.4 mechanisms remain
regression baselines.

## Goal contract

Outcome:

- Make the three existing `jj` editor surfaces easier to install, verify, and
  use every day.

Scope:

- Keep one binary with explicit `edit`, `diff`, and `merge` protocol routes.
- Prefer small local improvements over new dependencies.
- Keep real `jj` smoke tests for protocol-affecting behavior.
- Label external `jj` merge-tool limits as protocol limits, not local TODOs.

Non-goals:

- Do not add an agent runtime.
- Do not add a plugin system.
- Do not link against `jj_lib`.
- Do not replace `jj`'s built-in `scm-record` editor.
- Do not implement merge behavior that current `jj` does not expose to an
  external merge tool.

## Operating loop

Use the Socratic Codex loop for every slice:

1. State the smallest user-visible behavior.
2. Re-read the protocol path before editing code.
3. Add or update the smallest test that catches the behavior.
4. Implement the smallest shared-root change.
5. Run the slice test and then the validation set.
6. Update docs only when commands, keys, limits, or setup change.
7. Stop before new dependencies, `jj_lib`, agent runtime, or protocol-boundary
   changes.

## Phase 3 priorities

Implementation order:

1. Default usability checks.
2. Diff file navigation.
3. Merge conflict-block workflow.
4. Commit message editor hardening.
5. Release and compatibility guardrails.

## P3.1: Default usability checks

Status: implemented in its original scope; Phase 4 P4.2 and P4.4 now provide the
complete configuration contract and real generated-config evidence.

Problem:

The editor protocols work, but a user still has to manually assemble the jj
configuration and trust that the binary on PATH can be called by `jj`.

Scope:

- Add `jjc doctor`.
- Print the current `jjc` executable path.
- Check whether `jj` is available.
- Print the recommended `jj` configuration for all three editor surfaces.
- Do not modify user configuration.

Tests:

- Unit: the generated TOML snippet escapes paths correctly.
- Unit: a missing `jj` check returns a failing doctor report.
- CLI: `jjc doctor` parses as a real command.

Acceptance:

- `jjc doctor` gives enough information to configure `ui.editor`,
  `ui.diff-editor`, and `ui.merge-editor`.
- Missing `jj` is reported clearly and returns non-zero.
- No new dependency is added.

Later finding:

- The generated snippet omitted
  `merge-tool-edits-conflict-markers = true` and
  `conflict-marker-style = "git"`. Phase 4 fixed the snippet, standardized the
  tested baseline at `jj 0.43.0`, and added a real test that uses the
  doctor-generated config for edit, diff, and merge routes.

## P3.2: Diff file navigation

Status: implemented.

Problem:

Diff mode has hunk, line, function, binary, added-file, and deleted-file
selection, but larger changes still require too much linear scrolling.

Scope:

- Add file-level navigation between changed files.
- Add current file selection summary.
- Add whole-file select and unselect actions.
- Keep the existing hunk and line selection model.

Deferred:

- A full side panel.
- Search UI.
- Reimplementing `scm-record`.

Acceptance:

- A multi-file `jj split --tool jjc` flow can jump between files and select or
  unselect one file without touching unrelated hunks.

Evidence:

- Unit coverage verifies file navigation and current-file selection.
- Smoke coverage verifies `jj split --tool jjc` with `]Dw`.

## P3.3: Merge conflict-block workflow

Status: local mechanism implemented; the documented integration path, exact
dynamic marker parsing, and real multi-block acceptance are completed by Phase
4 P4.2.

Problem:

Merge mode can edit output or accept a whole side, but ordinary text conflicts
need a smaller workflow around conflict blocks.

Scope:

- Detect conflict-marker blocks in the output buffer.
- Jump between conflict blocks.
- Accept left/base/right for the current block when marker structure is clear.
- Warn when saving output that still contains conflict markers.

Boundary:

- Deletion results, non-normal files, unresolved executable-bit conflicts, and
  multi-side conflicts remain limited by the current external `jj` merge-tool
  protocol unless upstream behavior changes.

Acceptance:

- `jj resolve --tool jjc <path>` can resolve a normal text conflict block by
  accepting one side per block or manually editing the output.

Evidence:

- Unit coverage verifies current conflict-block side acceptance.
- Unit coverage verifies saving with conflict markers warns before writing.

Later finding:

- These unit tests construct Git diff3 markers directly. Under the previously
  recommended `jj` config, `$output` starts empty, so a normal real
  `jj resolve --tool jjc` invocation did not expose those blocks. Phase 4 added
  marker prefill, a per-tool Git marker style, exact `$marker_length` parsing,
  and three real marker fixtures covering complete, partial, and automatically
  lengthened conflicts.

## P3.4: Commit message editor hardening

Status: description behavior implemented; Phase 4 P4.4 completes generic and
sparse `ui.editor` semantics.

Problem:

`ui.editor` is the highest-frequency entry point. The current built-in editor
works, but commit messages need a few jj-specific guardrails.

Scope:

- Visually mute `JJ:` comment lines without rewriting them.
- Warn on empty commit messages before save.
- Keep title/body editing as plain text.
- Preserve templates exactly unless edited by the user.

Deferred:

- Full commit-message linting.
- Agent-generated messages.
- Project-specific templates.

Acceptance:

- `jj describe --config 'ui.editor=["jjc","edit"]'` remains a plain temp-file
  edit, but common accidental empty-message saves are caught.

Evidence:

- Existing render coverage verifies `JJ:` lines are visually dimmed.
- Unit coverage verifies empty messages warn before save.
- Smoke coverage verifies `jj describe --editor` does not complete on the first
  empty `:wq`.

Later finding:

- The guard is valid for `.jjdescription`, but `ui.editor` also edits inputs
  such as `.jjsparse`. Phase 4 preserves the description warning and now makes
  empty sparse and generic saves follow their own semantics, including a real
  empty-sparse smoke test.

## P3.5: Release and compatibility guardrails

Status: historical checklist implemented; Phase 4 P4.5 implements the enforced
release/compatibility workflow. The final local gate passed; the hosted CI run
remains pending external evidence.

Problem:

The test suite proves local behavior, but daily installation needs repeatable
release checks and visible compatibility expectations.

Scope:

- Document supported `jj` command entry points.
- Keep smoke coverage for `describe`, `diffedit`, `restore -i`, `split`,
  `squash -i`, and `resolve`.
- Add a release checklist using existing cargo and smoke commands.
- Avoid broad TUI automation beyond the current PTY checks.

Acceptance:

- A release candidate can be checked with one documented command sequence and
  the README states the supported `jj` surfaces clearly.

Evidence:

- README lists supported `jj` entry points.
- README includes the release check command sequence.

Later finding:

- A documented local command sequence does not ensure warnings-as-errors
  clippy, clean-checkout CI, pinned `jj`, PTY prerequisites, platform tiers, or
  non-skipping protocol tests. Phase 4 P4.5 now encodes those gates for Rust
  1.93.1 and pinned `jj 0.43.0`, with a non-blocking latest-`jj` advisory job.

## Validation set

Run before marking any Phase 3 slice complete:

```sh
cargo fmt --check
cargo check
cargo test
```

For protocol-affecting slices, the relevant smoke test must run through real
`jj`, not only a unit-level protocol mock.

## Phase 4 handoff

Phase 4 retained all Phase 3 behavior and closed the later local findings:

- P4.2 makes the P3.3 conflict-block workflow reachable and proves it through
  complete, partial, and marker-lengthened `jj resolve` flows.
- P4.4 narrows P3.4's description policy to the correct profiles and proves the
  generated doctor config across all three routes.
- P4.5 turns P3.5's release checklist into an enforced matrix where real-`jj`
  and PTY requirements cannot silently skip.

Do not mark Phase 3 “failed” or erase its evidence. Its feature work is the
baseline; Phase 4 corrects the integration and completion boundary.
