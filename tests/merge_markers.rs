mod support;

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use support::jj_available;

#[test]
fn jj_resolve_can_choose_different_sides_for_two_prefilled_blocks() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = two_block_conflict_repo("different-sides")?;

    let output = jj(repo.path())
        .env("JJC_KEYS", "31:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "root:file.txt"])
        .output()?;
    assert_success(output);

    assert_eq!(
        fs::read_to_string(repo.path().join("file.txt"))?,
        conflict_content("right", "left")
    );
    assert_no_conflicts(repo.path())?;
    Ok(())
}

#[test]
fn jj_resolve_round_trips_a_partially_resolved_marker_file() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = two_block_conflict_repo("partial-round-trip")?;

    let output = jj(repo.path())
        .env("JJC_KEYS", "3:wq<Enter><Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "root:file.txt"])
        .output()?;
    assert_success(output);

    let output = jj(repo.path()).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("file.txt"));
    assert!(fs::read_to_string(repo.path().join("file.txt"))?.contains("first = right"));

    let output = jj(repo.path())
        .env("JJC_KEYS", "1:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "root:file.txt"])
        .output()?;
    assert_success(output);

    assert_eq!(
        fs::read_to_string(repo.path().join("file.txt"))?,
        conflict_content("right", "left")
    );
    assert_no_conflicts(repo.path())?;
    Ok(())
}

#[test]
fn jj_resolve_preserves_literal_when_jj_lengthens_conflict_markers() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = marker_literal_conflict_repo("auto-marker-length")?;
    let expected = "right-before\n>>>>>>>\nright-after\n";

    let output = jj(repo.path())
        .env("JJC_KEYS", "3:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "root:file.txt"])
        .output()?;
    assert_success(output);

    assert_eq!(fs::read_to_string(repo.path().join("file.txt"))?, expected);
    assert_no_conflicts(repo.path())?;
    Ok(())
}

#[test]
fn jj_resolve_preserves_right_side_literal_with_attached_marker_suffix() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let expected = b"right-before\n>>>>>>>literal\nright-after\n";
    let repo = content_conflict_repo("attached-marker-suffix", b"base\n", b"left\n", expected)?;

    let output = jj(repo.path())
        .env("JJC_KEYS", "3:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "root:file.txt"])
        .output()?;
    assert_success(output);

    assert_eq!(fs::read(repo.path().join("file.txt"))?, expected);
    assert_no_conflicts(repo.path())?;
    Ok(())
}

#[test]
fn jj_resolve_round_trips_line_endings_before_accepting_each_side() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    // The first two cases mirror jj's CRLF conflict-parser regression inputs:
    // https://github.com/jj-vcs/jj/blob/v0.45.1/lib/src/conflicts.rs
    let cases = [
        ("bare-cr", &b"base"[..], &b"left\r"[..], &b"right"[..]),
        (
            "mixed-crlf-final-cr",
            &b"base\r\n"[..],
            &b"left\r\nmore\r"[..],
            &b"right\r\n"[..],
        ),
        (
            "crlf",
            &b"base\r\n"[..],
            &b"left\r\n"[..],
            &b"right\r\n"[..],
        ),
        ("no-final-lf", &b"base"[..], &b"left"[..], &b"right"[..]),
    ];

    for (name, base, left, right) in cases {
        for (side, key, expected) in [
            ("left", '1', left),
            ("base", '2', base),
            ("right", '3', right),
        ] {
            let repo = content_conflict_repo(&format!("{name}-{side}"), base, left, right)?;
            let original_markers = fs::read(repo.path().join("file.txt"))?;

            let unchanged = jj(repo.path())
                .env("JJC_KEYS", ":wq<Enter><Enter>")
                .args(merge_editor_config())
                .args(["resolve", "--tool", "jjc", "root:file.txt"])
                .output()?;
            assert!(!unchanged.status.success());
            assert!(
                String::from_utf8_lossy(&unchanged.stderr)
                    .contains("The output file is either unchanged or empty")
            );
            assert_eq!(fs::read(repo.path().join("file.txt"))?, original_markers);

            // jj rejects an unchanged output file. Editing only the opening
            // marker's label makes it parse and materialize the unchanged sides.
            // Insert before the label so its original CRLF/LF ending is intact.
            assert_success(
                jj(repo.path())
                    .env("JJC_KEYS", "wiX<Esc>:wq<Enter><Enter>")
                    .args(merge_editor_config())
                    .args(["resolve", "--tool", "jjc", "root:file.txt"])
                    .output()?,
            );
            let unresolved = jj(repo.path()).args(["resolve", "--list"]).output()?;
            assert_success_ref(&unresolved);
            assert!(String::from_utf8_lossy(&unresolved.stdout).contains("file.txt"));

            assert_success(
                jj(repo.path())
                    .env("JJC_KEYS", format!("{key}:wq<Enter>"))
                    .args(merge_editor_config())
                    .args(["resolve", "--tool", "jjc", "root:file.txt"])
                    .output()?,
            );

            assert_eq!(
                fs::read(repo.path().join("file.txt"))?,
                expected,
                "{name}: accepting {side} after a marker round trip changed its bytes"
            );
            assert_no_conflicts(repo.path())?;
        }
    }
    Ok(())
}

#[test]
fn jj_resolve_crlf_marker_label_append_preserves_each_side() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    // Keep the opening marker's CRLF intact when appending to its label with A.
    // These sides mirror jj's mixed-CRLF/final-CR conflict-parser regression.
    let base = &b"base\r\n"[..];
    let left = &b"left\r\nmore\r"[..];
    let right = &b"right\r\n"[..];
    for (side, key, expected) in [
        ("left", '1', left),
        ("base", '2', base),
        ("right", '3', right),
    ] {
        let repo = content_conflict_repo(&format!("crlf-label-append-{side}"), base, left, right)?;
        assert_success(
            jj(repo.path())
                .env("JJC_KEYS", "A label<Esc>:wq<Enter><Enter>")
                .args(merge_editor_config())
                .args(["--config", "ui.conflict-marker-style=\"git\""])
                .args(["resolve", "--tool", "jjc", "root:file.txt"])
                .output()?,
        );
        let unresolved = jj(repo.path()).args(["resolve", "--list"]).output()?;
        assert_success_ref(&unresolved);
        assert!(String::from_utf8_lossy(&unresolved.stdout).contains("file.txt"));
        let markers = fs::read(repo.path().join("file.txt"))?;
        let opening = markers
            .split_inclusive(|byte| *byte == b'\n')
            .next()
            .unwrap();
        assert!(opening.starts_with(b"<<<<<<< "));
        assert!(
            opening.ends_with(b"\r\n"),
            "appending a marker label must preserve its CRLF: {opening:?}"
        );

        assert_success(
            jj(repo.path())
                .env("JJC_KEYS", format!("{key}:wq<Enter>"))
                .args(merge_editor_config())
                .args(["resolve", "--tool", "jjc", "root:file.txt"])
                .output()?,
        );
        assert_eq!(
            fs::read(repo.path().join("file.txt"))?,
            expected,
            "accepting {side} after appending to a marker label changed its bytes"
        );
        assert_no_conflicts(repo.path())?;
    }
    Ok(())
}

struct TestRepo {
    root: PathBuf,
    repo: PathBuf,
}

impl TestRepo {
    fn path(&self) -> &Path {
        &self.repo
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn two_block_conflict_repo(name: &str) -> io::Result<TestRepo> {
    content_conflict_repo(
        name,
        conflict_content("base", "base").as_bytes(),
        conflict_content("left", "left").as_bytes(),
        conflict_content("right", "right").as_bytes(),
    )
}

fn marker_literal_conflict_repo(name: &str) -> io::Result<TestRepo> {
    content_conflict_repo(
        name,
        b"base\n",
        b"left\n",
        b"right-before\n>>>>>>>\nright-after\n",
    )
}

fn content_conflict_repo(
    name: &str,
    base_content: &[u8],
    left_content: &[u8],
    right_content: &[u8],
) -> io::Result<TestRepo> {
    let repo = init_repo(name)?;
    fs::write(repo.path().join("file.txt"), base_content)?;
    assert_success(jj(repo.path()).args(["describe", "-m", "base"]).output()?);

    assert_success(jj(repo.path()).args(["new", "-m", "left"]).output()?);
    fs::write(repo.path().join("file.txt"), left_content)?;
    let left = rev(repo.path())?;

    assert_success(
        jj(repo.path())
            .args(["new", "@-", "-m", "right"])
            .output()?,
    );
    fs::write(repo.path().join("file.txt"), right_content)?;
    let right = rev(repo.path())?;

    assert_success(
        jj(repo.path())
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );
    Ok(repo)
}

fn conflict_content(first: &str, second: &str) -> String {
    format!(
        "header\n\
         first = {first}\n\
         common-01\n\
         common-02\n\
         common-03\n\
         common-04\n\
         common-05\n\
         common-06\n\
         common-07\n\
         common-08\n\
         common-09\n\
         common-10\n\
         common-11\n\
         common-12\n\
         second = {second}\n\
         footer\n"
    )
}

fn init_repo(name: &str) -> io::Result<TestRepo> {
    let root = std::env::temp_dir().join(format!(
        "jjc-merge-markers-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let repo = root.join("repo");
    fs::create_dir_all(&root)?;
    assert_success(
        Command::new("jj")
            .args(["git", "init"])
            .arg(&repo)
            .output()?,
    );
    Ok(TestRepo { root, repo })
}

fn jj(repo: &Path) -> Command {
    let mut command = Command::new("jj");
    command.current_dir(repo).arg("--no-pager");
    command
}

fn rev(repo: &Path) -> io::Result<String> {
    let output = jj(repo)
        .args(["log", "-r", "@", "--no-graph", "-T", "change_id.short()"])
        .output()?;
    assert_success_ref(&output);
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn merge_editor_config() -> Vec<String> {
    vec![
        "--config".into(),
        "ui.merge-editor=\"jjc\"".into(),
        "--config".into(),
        format!("merge-tools.jjc.program={}", toml_string(jjc())),
        "--config".into(),
        "merge-tools.jjc.merge-args=[\"merge\",\"$left\",\"$base\",\"$right\",\"$output\",\"--marker-length\",\"$marker_length\",\"--path\",\"$path\"]".into(),
        "--config".into(),
        "merge-tools.jjc.merge-tool-edits-conflict-markers=true".into(),
        "--config".into(),
        "merge-tools.jjc.conflict-marker-style=\"git\"".into(),
    ]
}

fn assert_no_conflicts(repo: &Path) -> io::Result<()> {
    let output = jj(repo).args(["resolve", "--list"]).output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No conflicts found"));
    Ok(())
}

fn jjc() -> &'static str {
    env!("CARGO_BIN_EXE_jjc")
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn assert_success(output: Output) {
    assert_success_ref(&output);
}

fn assert_success_ref(output: &Output) {
    assert!(
        output.status.success(),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
