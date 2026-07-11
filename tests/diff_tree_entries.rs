#![cfg(unix)]

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn diff_can_select_or_unselect_an_executable_bit_change() -> io::Result<()> {
    let root = temp_root("mode-direct")?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    create_diff_dirs(&left, &right, &output)?;
    fs::write(left.join("script.sh"), "#!/bin/sh\n")?;
    fs::write(right.join("script.sh"), "#!/bin/sh\n")?;
    fs::write(output.join("script.sh"), "#!/bin/sh\n")?;
    set_mode(&left.join("script.sh"), 0o644)?;
    set_mode(&right.join("script.sh"), 0o755)?;
    set_mode(&output.join("script.sh"), 0o755)?;

    run_diff(&left, &right, &output, "Dw")?;
    assert!(!is_executable(&output.join("script.sh"))?);

    run_diff(&left, &right, &output, "Sw")?;
    assert!(is_executable(&output.join("script.sh"))?);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diff_selects_file_content_and_executable_bit_independently() -> io::Result<()> {
    let root = temp_root("content-and-mode")?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    create_diff_dirs(&left, &right, &output)?;
    fs::write(left.join("script.sh"), "old\n")?;
    fs::write(right.join("script.sh"), "new\n")?;
    fs::write(output.join("script.sh"), "new\n")?;
    set_mode(&left.join("script.sh"), 0o644)?;
    set_mode(&right.join("script.sh"), 0o755)?;
    set_mode(&output.join("script.sh"), 0o755)?;

    run_diff(&left, &right, &output, " w")?;
    assert_eq!(fs::read_to_string(output.join("script.sh"))?, "old\n");
    assert!(is_executable(&output.join("script.sh"))?);

    run_diff(&left, &right, &output, "j w")?;
    assert_eq!(fs::read_to_string(output.join("script.sh"))?, "new\n");
    assert!(!is_executable(&output.join("script.sh"))?);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diff_can_select_or_unselect_a_dangling_symlink_target() -> io::Result<()> {
    let root = temp_root("symlink-direct")?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    create_diff_dirs(&left, &right, &output)?;
    symlink("old-missing-target", left.join("link"))?;
    symlink("new-missing-target", right.join("link"))?;
    symlink("new-missing-target", output.join("link"))?;

    run_diff(&left, &right, &output, "Dw")?;
    assert_eq!(
        fs::read_link(output.join("link"))?,
        Path::new("old-missing-target")
    );

    run_diff(&left, &right, &output, "Sw")?;
    assert_eq!(
        fs::read_link(output.join("link"))?,
        Path::new("new-missing-target")
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diff_replaces_an_output_symlink_without_writing_through_it() -> io::Result<()> {
    let root = temp_root("symlink-no-follow")?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    let target = root.join("outside.txt");
    create_diff_dirs(&left, &right, &output)?;
    fs::write(&target, "sentinel\n")?;
    fs::write(left.join("path"), "left file\n")?;
    symlink(&target, right.join("path"))?;
    symlink(&target, output.join("path"))?;

    run_diff(&left, &right, &output, "Dw")?;

    assert_eq!(fs::read_to_string(&target)?, "sentinel\n");
    assert!(
        fs::symlink_metadata(output.join("path"))?
            .file_type()
            .is_file()
    );
    assert_eq!(fs::read_to_string(output.join("path"))?, "left file\n");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diff_rejects_an_output_symlink_ancestor_before_writing_through_it() -> io::Result<()> {
    let root = temp_root("symlink-ancestor")?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    let outside = root.join("outside");
    create_diff_dirs(&left, &right, &output)?;
    fs::create_dir_all(left.join("nested"))?;
    fs::create_dir_all(right.join("nested"))?;
    fs::create_dir_all(&outside)?;
    fs::write(left.join("a-good.txt"), "left\n")?;
    fs::write(right.join("a-good.txt"), "right\n")?;
    fs::write(output.join("a-good.txt"), "right\n")?;
    fs::write(left.join("nested/file.txt"), "left\n")?;
    fs::write(right.join("nested/file.txt"), "right\n")?;
    fs::write(outside.join("file.txt"), "sentinel\n")?;
    symlink(&outside, output.join("nested"))?;

    let result = Command::new(jjc())
        .env("JJC_KEYS", "Dw")
        .args(["diff"])
        .arg(&left)
        .arg(&right)
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("output ancestor"));
    assert_eq!(fs::read_to_string(output.join("a-good.txt"))?, "right\n");
    assert_eq!(fs::read_to_string(outside.join("file.txt"))?, "sentinel\n");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diff_rejects_file_directory_changes_before_mutating_output() -> io::Result<()> {
    let root = temp_root("file-directory")?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    create_diff_dirs(&left, &right, &output)?;
    fs::write(left.join("a-good.txt"), "old\n")?;
    fs::write(right.join("a-good.txt"), "new\n")?;
    fs::write(output.join("a-good.txt"), "new\n")?;
    fs::create_dir(left.join("z-path"))?;
    fs::write(left.join("z-path/child.txt"), "child\n")?;
    fs::write(right.join("z-path"), "file\n")?;
    fs::write(output.join("z-path"), "file\n")?;

    let result = Command::new(jjc())
        .env("JJC_KEYS", "Dw")
        .args(["diff"])
        .arg(&left)
        .arg(&right)
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("tree entry change"));
    assert_eq!(fs::read_to_string(output.join("a-good.txt"))?, "new\n");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_d_can_leave_a_mode_only_change_in_the_remainder() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let (root, repo) = init_repo("mode-split")?;
    fs::write(repo.join("a-script.sh"), "#!/bin/sh\n")?;
    set_mode(&repo.join("a-script.sh"), 0o644)?;
    fs::write(repo.join("z-selected.txt"), "old\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    set_mode(&repo.join("a-script.sh"), 0o755)?;
    fs::write(repo.join("z-selected.txt"), "new\n")?;

    let result = jj(&repo)
        .env("JJC_KEYS", "Dw")
        .args(diff_editor_config())
        .args(["split", "--tool", "jjc", "-m", "selected"])
        .output()?;
    assert_success(result);

    let selected = revision_diff(&repo, "@-")?;
    assert!(selected.contains("z-selected.txt"), "{selected}");
    assert!(!selected.contains("a-script.sh"), "{selected}");
    let remainder = revision_diff(&repo, "@")?;
    assert!(remainder.contains("a-script.sh"), "{remainder}");
    assert!(!remainder.contains("z-selected.txt"), "{remainder}");
    assert!(is_executable(&repo.join("a-script.sh"))?);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_d_can_leave_a_symlink_change_in_the_remainder() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let (root, repo) = init_repo("symlink-split")?;
    symlink("old-target", repo.join("a-link"))?;
    fs::write(repo.join("z-selected.txt"), "old\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::remove_file(repo.join("a-link"))?;
    symlink("new-target", repo.join("a-link"))?;
    fs::write(repo.join("z-selected.txt"), "new\n")?;

    let result = jj(&repo)
        .env("JJC_KEYS", "Dw")
        .args(diff_editor_config())
        .args(["split", "--tool", "jjc", "-m", "selected"])
        .output()?;
    assert_success(result);

    let selected = revision_diff(&repo, "@-")?;
    assert!(selected.contains("z-selected.txt"), "{selected}");
    assert!(!selected.contains("a-link"), "{selected}");
    let remainder = revision_diff(&repo, "@")?;
    assert!(remainder.contains("a-link"), "{remainder}");
    assert!(!remainder.contains("z-selected.txt"), "{remainder}");
    assert_eq!(fs::read_link(repo.join("a-link"))?, Path::new("new-target"));
    fs::remove_dir_all(root)?;
    Ok(())
}

fn create_diff_dirs(left: &Path, right: &Path, output: &Path) -> io::Result<()> {
    fs::create_dir_all(left)?;
    fs::create_dir_all(right)?;
    fs::create_dir_all(output)
}

fn run_diff(left: &Path, right: &Path, output: &Path, keys: &str) -> io::Result<()> {
    let result = Command::new(jjc())
        .env("JJC_KEYS", keys)
        .args(["diff"])
        .arg(left)
        .arg(right)
        .arg(output)
        .output()?;
    assert_success(result);
    Ok(())
}

fn init_repo(name: &str) -> io::Result<(PathBuf, PathBuf)> {
    let root = temp_root(name)?;
    let repo = root.join("repo");
    assert_success(
        Command::new("jj")
            .args(["git", "init"])
            .arg(&repo)
            .output()?,
    );
    Ok((root, repo))
}

fn jj(repo: &Path) -> Command {
    let mut command = Command::new("jj");
    command.current_dir(repo).arg("--no-pager");
    command
}

fn diff_editor_config() -> Vec<String> {
    vec![
        "--config".into(),
        "ui.diff-editor=\"jjc\"".into(),
        "--config".into(),
        format!("merge-tools.jjc.program={}", toml_string(jjc())),
        "--config".into(),
        "merge-tools.jjc.edit-args=[\"diff\",\"$left\",\"$right\",\"$output\"]".into(),
    ]
}

fn revision_diff(repo: &Path, revision: &str) -> io::Result<String> {
    let output = jj(repo).args(["diff", "--git", "-r", revision]).output()?;
    assert_success_ref(&output);
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn jj_available() -> bool {
    let available = Command::new("jj").arg("--version").output().is_ok();
    if !available && std::env::var_os("JJC_REQUIRE_INTEGRATION").is_some() {
        panic!("jj is required when JJC_REQUIRE_INTEGRATION is set");
    }
    available
}

fn is_executable(path: &Path) -> io::Result<bool> {
    Ok(fs::symlink_metadata(path)?.permissions().mode() & 0o111 != 0)
}

fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let mut permissions = fs::symlink_metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
}

fn temp_root(name: &str) -> io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "jjc-diff-tree-{name}-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root)?;
    Ok(root)
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
