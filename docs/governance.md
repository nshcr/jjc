# Engineering governance

This document defines the controls expected for changes to `jjc`. Repository
files encode part of the policy; GitHub settings are a separate enforcement
surface and must be verified directly.

## Change classes and acceptance

| Change class | Required evidence |
| --- | --- |
| Documentation-only | link and consistency review; fast gate when commands or configuration change |
| Product code | fast gate plus focused regression tests |
| `jj` protocol, TUI, or filesystem writeback | full non-skipping gate and exact-baseline CI |
| Dependency | full dependency policy, lockfile review, license/source rationale |
| CI or release | immutable Action references, workflow lint, least permissions, dry-run where possible |

Claims use four distinct evidence levels: implemented, verified-local,
verified-ci on an exact revision, and verified-release for a tagged artifact.
A test count, historical plan, or green run on another SHA cannot substitute for
the required level.

## Repository controls

- `rust-toolchain.toml` pins formatter, linter, and compiler behavior.
- `scripts/verify.sh` owns the fast and full local gates.
- `JJC_REQUIRE_INTEGRATION=1` prevents missing prerequisites from becoming a
  successful strict test run; `JJC_EXPECT_JJ_VERSION` binds it to the tested
  protocol baseline.
- `JJ_CONFIG` is empty in governed CI/release lanes so user configuration cannot
  alter integration results.
- `deny.toml` blocks unapproved registries, Git dependencies, wildcard direct
  versions, disallowed licenses, known vulnerabilities, and directly used
  unmaintained crates. Unmaintained transitive crates require upstream tracking
  when no safe upgrade is available.
- Workflow Actions use reviewed full commit SHAs. Dependabot proposes updates,
  but a human still reviews the upstream diff and release notes.
- `Cargo.toml` disables accidental crates.io publication. GitHub Releases are
  the only currently governed publication path.

## Required GitHub enforcement

The following settings cannot be guaranteed by committed files. Before treating
the repository as governed, verify them in GitHub and record the observation
date and exact repository:

1. An active `main` Ruleset blocks force pushes and deletion, requires pull
   requests, requires conversations to be resolved, and requires these checks:
   `Quality (ubuntu-latest)`, `Quality (macos-latest)`,
   `Quality (windows-latest)`, `Dependency policy`,
   `jj integration (ubuntu-latest)`, and `jj integration (macos-latest)`.
   Require `Dependency review` for pull requests.
2. An active tag Ruleset named `Immutable release tags` targets
   `refs/tags/v*`, has no exclusions, and restricts creation, update, and
   deletion. Limit its bypass actors to the repository owner. The release
   workflow reads this Ruleset and the current peeled tag target immediately
   before publication; absence, drift, or a target other than `GITHUB_SHA`
   fails closed.
3. Actions allow only approved sources and require full-SHA pinning.
4. The `release` environment requires an explicit approval before publication.
5. Dependency graph, Dependabot alerts/security updates, private vulnerability
   reporting, secret scanning/push protection, and code scanning are enabled as
   available.

If a setting is absent, report it as a governance gap; do not describe the
corresponding repository file as enforcement. Changes to these remote settings
are external state changes and require explicit owner authorization.

## Review and exceptions

A temporary exception must state the exact control, reason, expiry or removal
condition, and compensating evidence. Silent skips, `continue-on-error` on a
blocking lane, and moving a published tag are not acceptable exceptions.

The scheduled latest-`jj` job is intentionally advisory. All release gates,
artifact checksums, and attestations are blocking for the tagged revision.
