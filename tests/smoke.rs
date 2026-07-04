use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn jj_describe_uses_jjc_editor() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("describe")?;
    let output = jj(&repo)
        .env("JJC_KEYS", "iSmoke<Esc>:wq<Enter>")
        .arg("--config")
        .arg(format!("ui.editor=[{},\"edit\"]", toml_string(jjc())))
        .arg("describe")
        .arg("--editor")
        .output()?;
    assert_success(output);

    let output = jj(&repo)
        .args(["log", "-r", "@", "--no-graph", "-T", "description"])
        .output()?;
    assert_success_ref(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Smoke\n");
    Ok(())
}

#[test]
fn jj_describe_q_bang_cancels_edit() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("describe-cancel")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);

    let output = jj(&repo)
        .env("JJC_KEYS", "iChanged<Esc>:q!<Enter>")
        .arg("--config")
        .arg(format!("ui.editor=[{},\"edit\"]", toml_string(jjc())))
        .arg("describe")
        .arg("--editor")
        .output()?;
    assert!(!output.status.success());

    let output = jj(&repo)
        .args(["log", "-r", "@", "--no-graph", "-T", "description"])
        .output()?;
    assert_success_ref(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "base\n");
    Ok(())
}

#[test]
fn jj_restore_uses_jjc_diff_editor() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("restore")?;
    fs::write(repo.join("file.txt"), "a\nold\nc\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nnew\nc\n")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "w")
        .args(diff_editor_config())
        .args(["restore", "-i", "--tool", "jjc"])
        .output()?;
    assert_success(output);

    assert_eq!(fs::read_to_string(repo.join("file.txt"))?, "a\nold\nc\n");
    Ok(())
}

#[test]
fn jj_restore_q_cancels_diff_editor() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("restore-cancel")?;
    fs::write(repo.join("file.txt"), "a\nold\nc\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nnew\nc\n")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "q")
        .args(diff_editor_config())
        .args(["restore", "-i", "--tool", "jjc"])
        .output()?;
    assert!(!output.status.success());

    assert_eq!(fs::read_to_string(repo.join("file.txt"))?, "a\nnew\nc\n");
    Ok(())
}

#[test]
fn jj_split_uses_jjc_diff_editor() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("split")?;
    fs::write(repo.join("file.txt"), "a\nold1\nc\nold2\ne\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nnew1\nc\nnew2\ne\n")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "w")
        .args(diff_editor_config())
        .args(["split", "--tool", "jjc", "-m", "selected"])
        .output()?;
    assert_success(output);

    let output = jj(&repo)
        .args(["log", "-r", "@-", "--no-graph", "-T", "description"])
        .output()?;
    assert_success_ref(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "selected\n");
    Ok(())
}

#[test]
fn jj_split_can_select_one_changed_line_with_jjc() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("line-split")?;
    fs::write(repo.join("file.txt"), "a\nold1\nc\nold2\ne\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nnew1\nc\nnew2\ne\n")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "nxw")
        .args(diff_editor_config())
        .args(["split", "--tool", "jjc", "-m", "selected"])
        .output()?;
    assert_success(output);

    let output = jj(&repo)
        .args(["file", "show", "-r", "@-", "file.txt"])
        .output()?;
    assert_success_ref(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a\nold1\nc\nnew2\ne\n"
    );
    Ok(())
}

#[test]
fn jj_split_can_select_deleted_line_with_jjc() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("deleted-line-split")?;
    fs::write(repo.join("file.txt"), "a\nold\nc\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nc\n")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "w")
        .args(diff_editor_config())
        .args(["split", "--tool", "jjc", "-m", "selected"])
        .output()?;
    assert_success(output);

    let output = jj(&repo)
        .args(["file", "show", "-r", "@-", "file.txt"])
        .output()?;
    assert_success_ref(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "a\nc\n");
    Ok(())
}

#[test]
fn jj_resolve_uses_jjc_merge_editor() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("resolve")?;
    fs::write(repo.join("file.txt"), "base\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    fs::write(repo.join("file.txt"), "left\n")?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::write(repo.join("file.txt"), "right\n")?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );

    let output = jj(&repo)
        .env("JJC_KEYS", "3:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "file.txt"])
        .output()?;
    assert_success(output);

    assert_eq!(fs::read_to_string(repo.join("file.txt"))?, "right\n");
    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No conflicts found"));
    Ok(())
}

#[test]
fn jj_resolve_q_cancels_merge_editor() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = conflict_repo("resolve-cancel")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "q")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "file.txt"])
        .output()?;
    assert!(!output.status.success());

    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("file.txt"));
    Ok(())
}

#[test]
fn jj_resolve_uses_jjc_for_binary_conflict() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("binary-resolve")?;
    fs::write(repo.join("file.bin"), [0, 1, 2])?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    fs::write(repo.join("file.bin"), [0xff, 1])?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::write(repo.join("file.bin"), [0xfe, 2])?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );

    let output = jj(&repo)
        .env("JJC_KEYS", "1w")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "file.bin"])
        .output()?;
    assert_success(output);

    assert_eq!(fs::read(repo.join("file.bin"))?, vec![0xff, 1]);
    Ok(())
}

#[test]
fn jj_resolve_delete_modify_can_keep_modified_side() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = delete_modify_repo("delete-modify-keep")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "3:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "file.txt"])
        .output()?;
    assert_success(output);

    assert_eq!(fs::read_to_string(repo.join("file.txt"))?, "right\n");
    Ok(())
}

#[test]
fn jj_resolve_delete_modify_delete_side_stays_protocol_limited() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = delete_modify_repo("delete-modify-delete")?;

    let output = jj(&repo)
        .env("JJC_KEYS", "1:wq<Enter>")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "file.txt"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("empty"));

    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("file.txt"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn jj_resolve_executable_bit_conflict_stays_protocol_limited() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("executable-bit")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    fs::write(repo.join("script.sh"), "#!/bin/sh\necho left\n")?;
    set_mode(&repo.join("script.sh"), 0o755)?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::write(repo.join("script.sh"), "#!/bin/sh\necho right\n")?;
    set_mode(&repo.join("script.sh"), 0o644)?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );

    let output = jj(&repo)
        .env("JJC_KEYS", "3w")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "script.sh"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("executable bit"));

    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("script.sh"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn jj_resolve_symlink_conflict_stays_protocol_limited() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("symlink")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    std::os::unix::fs::symlink("left-target", repo.join("link"))?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::write(repo.join("link"), "right\n")?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );

    let output = jj(&repo)
        .env("JJC_KEYS", "3w")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "link"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("normal files"));

    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("symlink"));
    Ok(())
}

#[test]
fn jj_resolve_file_directory_conflict_stays_protocol_limited() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("file-directory")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    fs::write(repo.join("path"), "left\n")?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::create_dir(repo.join("path"))?;
    fs::write(repo.join("path").join("file.txt"), "right\n")?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );

    let output = jj(&repo)
        .env("JJC_KEYS", "3w")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "path"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("normal files"));

    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("directory"));
    Ok(())
}

#[test]
fn jj_resolve_multi_side_conflict_stays_protocol_limited() -> io::Result<()> {
    if !jj_available() {
        return Ok(());
    }
    let repo = init_repo("multi-side")?;
    fs::write(repo.join("file.txt"), "base\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    let one = change_file_on_new_child(&repo, "one", "one\n")?;
    let two = change_file_on_new_child(&repo, "two", "two\n")?;
    let three = change_file_on_new_child(&repo, "three", "three\n")?;
    assert_success(
        jj(&repo)
            .args(["new", &one, &two, &three, "-m", "merge"])
            .output()?,
    );

    let output = jj(&repo)
        .env("JJC_KEYS", "3w")
        .args(merge_editor_config())
        .args(["resolve", "--tool", "jjc", "file.txt"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("3 sides"));

    let output = jj(&repo).args(["resolve", "--list"]).output()?;
    assert_success_ref(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("3-sided"));
    Ok(())
}

fn jj_available() -> bool {
    Command::new("jj").arg("--version").output().is_ok()
}

fn init_repo(name: &str) -> io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "jjc-smoke-{name}-{}-{}",
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
    Ok(repo)
}

fn delete_modify_repo(name: &str) -> io::Result<PathBuf> {
    let repo = init_repo(name)?;
    fs::write(repo.join("file.txt"), "base\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    fs::remove_file(repo.join("file.txt"))?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::write(repo.join("file.txt"), "right\n")?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );
    Ok(repo)
}

fn conflict_repo(name: &str) -> io::Result<PathBuf> {
    let repo = init_repo(name)?;
    fs::write(repo.join("file.txt"), "base\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "left"]).output()?);
    fs::write(repo.join("file.txt"), "left\n")?;
    let left = rev(&repo)?;
    assert_success(jj(&repo).args(["new", "@-", "-m", "right"]).output()?);
    fs::write(repo.join("file.txt"), "right\n")?;
    let right = rev(&repo)?;
    assert_success(
        jj(&repo)
            .args(["new", &left, &right, "-m", "merge"])
            .output()?,
    );
    Ok(repo)
}

fn change_file_on_new_child(repo: &Path, message: &str, content: &str) -> io::Result<String> {
    assert_success(jj(repo).args(["new", "@-", "-m", message]).output()?);
    fs::write(repo.join("file.txt"), content)?;
    rev(repo)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
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

fn merge_editor_config() -> Vec<String> {
    vec![
        "--config".into(),
        "ui.merge-editor=\"jjc\"".into(),
        "--config".into(),
        format!("merge-tools.jjc.program={}", toml_string(jjc())),
        "--config".into(),
        "merge-tools.jjc.merge-args=[\"merge\",\"$left\",\"$base\",\"$right\",\"$output\",\"--marker-length\",\"$marker_length\",\"--path\",\"$path\"]".into(),
    ]
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
