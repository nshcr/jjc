#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const EXPECTED_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TAG_OBJECT_SHA: &str = "cccccccccccccccccccccccccccccccccccccccc";
static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "jjc-release-tag-guard-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn install_fake_gh(root: &Path) {
    let script = format!(
        r#"#!/bin/sh
set -eu

case "$*" in
  *'/rulesets?'*)
    if [ "${{FAKE_RULESET_STATE:-valid}}" = missing ]; then
      exit 0
    fi
    printf '%s\n' 42
    ;;
  *'/rulesets/42?'*)
    printf '%s\n' "${{FAKE_RULESET_STATE:-valid}}" | sed 's/^valid$/ok/'
    ;;
  *'/git/ref/tags/v1.2.3'*)
    case "${{FAKE_TAG_KIND:-lightweight}}" in
      lightweight) printf 'commit %s\n' "${{FAKE_TAG_SHA:-{EXPECTED_SHA}}}" ;;
      annotated) printf 'tag {TAG_OBJECT_SHA}\n' ;;
      *) exit 2 ;;
    esac
    ;;
  *'/git/tags/{TAG_OBJECT_SHA}'*)
    printf 'commit %s\n' "${{FAKE_TAG_SHA:-{EXPECTED_SHA}}}"
    ;;
  *)
    printf 'unexpected fake gh call: %s\n' "$*" >&2
    exit 2
    ;;
esac
"#
    );

    let path = root.join("gh");
    fs::write(&path, script).expect("write fake gh");
    let mut permissions = fs::metadata(&path).expect("stat fake gh").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fake gh executable");
}

fn run_guard(tag_kind: &str, tag_sha: &str, ruleset_state: &str) -> std::process::Output {
    let root = TestRoot::new();
    install_fake_gh(root.path());

    let system_path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(root.path().to_path_buf()).chain(std::env::split_paths(&system_path)),
    )
    .expect("construct PATH");

    Command::new("sh")
        .arg(format!(
            "{}/scripts/verify-release-tag.sh",
            env!("CARGO_MANIFEST_DIR")
        ))
        .args(["example/jjc", "v1.2.3", EXPECTED_SHA])
        .env("PATH", path)
        .env("FAKE_TAG_KIND", tag_kind)
        .env("FAKE_TAG_SHA", tag_sha)
        .env("FAKE_RULESET_STATE", ruleset_state)
        .output()
        .expect("run release tag guard")
}

#[test]
fn accepts_lightweight_tag_at_expected_commit() {
    let output = run_guard("lightweight", EXPECTED_SHA, "valid");
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn peels_annotated_tag_to_expected_commit() {
    let output = run_guard("annotated", EXPECTED_SHA, "valid");
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn rejects_tag_that_moved_after_build() {
    let output = run_guard("lightweight", OTHER_SHA, "valid");
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("expected aaaaaaaaaa"),
        "{output:?}"
    );
}

#[test]
fn rejects_missing_or_invalid_ruleset() {
    for state in ["missing", "invalid"] {
        let output = run_guard("lightweight", EXPECTED_SHA, state);
        assert!(!output.status.success(), "state {state}: {output:?}");
    }
}
