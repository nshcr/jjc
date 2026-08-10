# Release runbook

Releases are published deliberately by the repository owner as GitHub archives
built by the tracked workflow; crates.io publication is disabled.

## 1. Prepare the candidate

1. Choose a Semantic Versioning version and update `Cargo.toml` plus
   `Cargo.lock` without changing the tested Rust or `jj` baseline implicitly.
2. Prepare concise release notes from the commits and user-visible changes.
3. Update README, support policy, compatibility limits, and migration notes.
4. Confirm the `main` Ruleset, the active `Immutable release tags` Ruleset,
   required checks, Actions restrictions, and protected `release` environment
   described in `governance.md`.
5. Review the complete candidate diff and confirm the worktree is clean.

## 2. Verify the exact revision

From the candidate revision on Linux or macOS:

```sh
./scripts/verify.sh full
cargo deny check
git status --short
git rev-parse HEAD
```

Record the revision, Rust/`jj` versions, command results, and any unavailable
platform. Required hosted CI must pass on that same revision before tagging.

## 3. Tag and publish

Create an annotated tag matching the Cargo version; use a signed tag when the
owner's signing identity is available:

```sh
git tag -s vX.Y.Z -m "jjc vX.Y.Z"
git push origin vX.Y.Z
```

An unsigned tag requires an explicit documented exception. The Release workflow
repeats the full gate on the tag SHA, builds all five archives, attests each
archive, generates `SHA256SUMS`, and pauses at the protected `release`
environment before publication.

Immediately before creating the release, the workflow fails closed unless the
active `Immutable release tags` Ruleset restricts `v*` creation, update, and
deletion and the current peeled tag target is exactly the event SHA. This
remote immutability check is required; `gh release create --verify-tag` alone
only proves that a tag exists.

Do not create a release manually while the workflow is running. Confirm that
the workflow's `headSha` equals the tag target.

## 4. Verify published artifacts

Download every archive and `SHA256SUMS`, then verify checksums and provenance:

```sh
sha256sum --check SHA256SUMS
gh attestation verify jjc-vX.Y.Z-<target>.<archive> --repo nshcr/jjc
```

On each supported platform, unpack the archive and run `jjc --version` and
`jjc doctor`. Record platform failures separately; one successful archive does
not validate the others. Check that release notes include compatibility changes,
known limits, and security acknowledgements where applicable.

## 5. Respond to a bad release

Never move or reuse a published tag. Stop promotion, preserve logs and artifact
digests, mark affected assets/release notes clearly, and publish a new patch
version after the corrected exact revision passes the full process. Removing a
compromised artifact or release is a destructive external action and requires a
documented incident decision and explicit owner authorization.
