#![cfg(unix)]

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[test]
fn tui_leaves_alternate_screen_on_exit() -> io::Result<()> {
    if !expect_available() {
        return Ok(());
    }
    let root = temp_root()?;
    let message = root.join("message.txt");
    let log = root.join("tty.log");
    fs::write(&message, "message\n")?;

    expect_alt_screen(&log, jjc(), &[s("edit"), path_arg(&message)], ":wq\r")?;

    assert_alt_screen_log(&log)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn edit_tty_scrolls_to_long_file_cursor() -> io::Result<()> {
    if !expect_available() {
        return Ok(());
    }
    let root = temp_root()?;
    let message = root.join("message.txt");
    let log = root.join("tty.log");
    fs::write(&message, numbered_lines("line", 60))?;

    expect_alt_screen_after_keys(
        &log,
        jjc(),
        &[s("edit"), path_arg(&message)],
        "\x1b[6~\x1b[6~\x1b[6~\x1b[6~",
        "line-040",
        ":wq\r",
    )?;

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn edit_tty_cursor_style_follows_vim_mode() -> io::Result<()> {
    if !expect_available() {
        return Ok(());
    }
    let root = temp_root()?;
    let message = root.join("message.txt");
    let log = root.join("tty.log");
    fs::write(&message, "message\n")?;

    expect_cursor_style_switch(&log, jjc(), &[s("edit"), path_arg(&message)])?;

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn diff_tty_scrolls_inside_long_hunk() -> io::Result<()> {
    if !expect_available() {
        return Ok(());
    }
    let root = temp_root()?;
    let left = root.join("left");
    let right = root.join("right");
    let output = root.join("output");
    let log = root.join("tty.log");
    fs::create_dir_all(&left)?;
    fs::create_dir_all(&right)?;
    fs::create_dir_all(&output)?;
    fs::write(left.join("file.txt"), numbered_lines("old", 60))?;
    fs::write(right.join("file.txt"), numbered_lines("new", 60))?;

    expect_alt_screen_after_keys(
        &log,
        jjc(),
        &[
            s("diff"),
            path_arg(&left),
            path_arg(&right),
            path_arg(&output),
        ],
        "\x1b[6~\x1b[6~\x1b[6~\x1b[6~",
        "new-040",
        "w",
    )?;

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn merge_tty_scrolls_output_pane() -> io::Result<()> {
    if !expect_available() {
        return Ok(());
    }
    let root = temp_root()?;
    let left = root.join("left.txt");
    let base = root.join("base.txt");
    let right = root.join("right.txt");
    let output = root.join("output.txt");
    let log = root.join("tty.log");
    fs::write(&left, numbered_lines("left", 60))?;
    fs::write(&base, numbered_lines("base", 60))?;
    fs::write(&right, numbered_lines("right", 60))?;
    fs::write(&output, numbered_lines("out", 60))?;

    expect_alt_screen_after_keys(
        &log,
        jjc(),
        &[
            s("merge"),
            path_arg(&left),
            path_arg(&base),
            path_arg(&right),
            path_arg(&output),
            s("--marker-length"),
            s("7"),
            s("--path"),
            s("file.txt"),
        ],
        "\x1b[6~\x1b[6~\x1b[6~\x1b[6~",
        "out-040",
        ":wq\r",
    )?;

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_diffedit_uses_diff_editor_tty() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = changed_repo("diffedit")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["diffedit", "--tool", "jjc", "-r", "@"]),
        "w",
    )?;

    assert_alt_screen_log(&log)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_restore_interactive_uses_diff_editor_tty() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = changed_repo("restore")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["restore", "-i", "--tool", "jjc"]),
        "w",
    )?;

    assert_alt_screen_log(&log)?;
    assert_eq!(fs::read_to_string(repo.join("file.txt"))?, "a\nold\nc\n");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_uses_diff_editor_tty() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = changed_repo("split")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["split", "--tool", "jjc", "-m", "selected"]),
        "w",
    )?;

    assert_alt_screen_log(&log)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_squash_interactive_uses_diff_editor_tty() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = changed_repo("squash")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(
            &repo,
            ["squash", "-i", "--tool", "jjc", "--use-destination-message"],
        ),
        "w",
    )?;

    assert_alt_screen_log(&log)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_resolve_uses_merge_editor_tty() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = conflict_repo("resolve-tty")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_merge_args(&repo, ["resolve", "--tool", "jjc", "root:file.txt"]),
        "3:wq\r",
    )?;

    assert_eq!(fs::read_to_string(repo.join("file.txt"))?, "right\n");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_tty_can_unselect_one_whole_hunk() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = two_hunk_repo("split-whole-hunk")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["split", "--tool", "jjc", "-m", "selected"]),
        " w",
    )?;

    assert_alt_screen_log(&log)?;
    assert_eq!(file_show(&repo, "@-", "file.txt")?, second_hunk_selected());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_restore_tty_can_restore_only_one_hunk() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = two_hunk_repo("restore-one-hunk")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["restore", "-i", "--tool", "jjc"]),
        " w",
    )?;

    assert_alt_screen_log(&log)?;
    assert_eq!(
        fs::read_to_string(repo.join("file.txt"))?,
        first_hunk_kept()
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_tty_can_select_one_changed_line_inside_a_hunk() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = compact_two_change_repo("split-line")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["split", "--tool", "jjc", "-m", "selected"]),
        "nxw",
    )?;

    assert_alt_screen_log(&log)?;
    assert_eq!(file_show(&repo, "@-", "file.txt")?, "a\nold1\nc\nnew2\ne\n");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_tty_function_toggle_selects_related_hunks() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = function_hunk_repo("split-function")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["split", "--tool", "jjc", "-m", "selected"]),
        "fw",
    )?;

    assert_alt_screen_log(&log)?;
    assert_eq!(
        file_show(&repo, "@-", "lib.rs")?,
        only_other_function_selected()
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn jj_split_tty_undo_redo_preserves_selection_state() -> io::Result<()> {
    if !expect_available() || !jj_available() {
        return Ok(());
    }
    let (root, repo) = two_hunk_repo("split-undo-redo")?;
    let log = root.join("tty.log");

    expect_alt_screen(
        &log,
        "jj",
        &jj_args(&repo, ["split", "--tool", "jjc", "-m", "selected"]),
        " urw",
    )?;

    assert_alt_screen_log(&log)?;
    assert_eq!(file_show(&repo, "@-", "file.txt")?, second_hunk_selected());
    fs::remove_dir_all(root)?;
    Ok(())
}

fn expect_cursor_style_switch(log: &Path, program: &str, args: &[String]) -> io::Result<()> {
    let script = format!(
        "log_file -noappend {}\nset timeout 10\nspawn {} {}\nexpect \"\\033\\[?1049h\"\nexpect \"\\033\\[2 q\"\nsend -- i\nexpect \"\\033\\[6 q\"\nsend -- \"\\033\"\nexpect \"\\033\\[2 q\"\nsend -- :wq\\r\nexpect \"\\033\\[?1049l\"\nexpect eof\nset wait_result [wait]\nexit [lindex $wait_result 3]\n",
        tcl_word(&path_arg(log)),
        tcl_word(program),
        args.iter()
            .map(|arg| tcl_word(arg))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let output = Command::new("expect").arg("-c").arg(script).output()?;
    assert_success(output);
    assert_alt_screen_log(log)?;
    Ok(())
}

fn expect_available() -> bool {
    Command::new("expect").arg("-v").output().is_ok()
}

fn jj_available() -> bool {
    Command::new("jj").arg("--version").output().is_ok()
}

fn jjc() -> &'static str {
    env!("CARGO_BIN_EXE_jjc")
}

fn changed_repo(name: &str) -> io::Result<(PathBuf, PathBuf)> {
    let root = temp_root()?;
    let repo = init_repo(&root, name)?;
    fs::write(repo.join("file.txt"), "a\nold\nc\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nnew\nc\n")?;
    Ok((root, repo))
}

fn compact_two_change_repo(name: &str) -> io::Result<(PathBuf, PathBuf)> {
    let root = temp_root()?;
    let repo = init_repo(&root, name)?;
    fs::write(repo.join("file.txt"), "a\nold1\nc\nold2\ne\n")?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), "a\nnew1\nc\nnew2\ne\n")?;
    Ok((root, repo))
}

fn two_hunk_repo(name: &str) -> io::Result<(PathBuf, PathBuf)> {
    let root = temp_root()?;
    let repo = init_repo(&root, name)?;
    fs::write(repo.join("file.txt"), two_hunk_base())?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("file.txt"), two_hunk_work())?;
    Ok((root, repo))
}

fn function_hunk_repo(name: &str) -> io::Result<(PathBuf, PathBuf)> {
    let root = temp_root()?;
    let repo = init_repo(&root, name)?;
    fs::write(repo.join("lib.rs"), function_hunk_base())?;
    assert_success(jj(&repo).args(["describe", "-m", "base"]).output()?);
    assert_success(jj(&repo).args(["new", "-m", "work"]).output()?);
    fs::write(repo.join("lib.rs"), function_hunk_work())?;
    Ok((root, repo))
}

fn conflict_repo(name: &str) -> io::Result<(PathBuf, PathBuf)> {
    let root = temp_root()?;
    let repo = init_repo(&root, name)?;
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
    Ok((root, repo))
}

fn init_repo(root: &Path, name: &str) -> io::Result<PathBuf> {
    let repo = root.join(name);
    assert_success(
        Command::new("jj")
            .args(["git", "init"])
            .arg(&repo)
            .output()?,
    );
    Ok(repo)
}

fn jj(repo: &Path) -> Command {
    let mut command = Command::new("jj");
    command.current_dir(repo).arg("--no-pager");
    command
}

fn jj_args<const N: usize>(repo: &Path, tail: [&str; N]) -> Vec<String> {
    let mut args = vec![
        s("--no-pager"),
        s("-R"),
        path_arg(repo),
        s("--config"),
        s("ui.diff-editor=\"jjc\""),
        s("--config"),
        format!("merge-tools.jjc.program={}", toml_string(jjc())),
        s("--config"),
        s("merge-tools.jjc.edit-args=[\"diff\",\"$left\",\"$right\",\"$output\"]"),
    ];
    args.extend(tail.into_iter().map(s));
    args
}

fn jj_merge_args<const N: usize>(repo: &Path, tail: [&str; N]) -> Vec<String> {
    let mut args = vec![
        s("--no-pager"),
        s("-R"),
        path_arg(repo),
        s("--config"),
        s("ui.merge-editor=\"jjc\""),
        s("--config"),
        format!("merge-tools.jjc.program={}", toml_string(jjc())),
        s("--config"),
        s(
            "merge-tools.jjc.merge-args=[\"merge\",\"$left\",\"$base\",\"$right\",\"$output\",\"--marker-length\",\"$marker_length\",\"--path\",\"$path\"]",
        ),
    ];
    args.extend(tail.into_iter().map(s));
    args
}

fn expect_alt_screen(log: &Path, program: &str, args: &[String], keys: &str) -> io::Result<()> {
    let script = format!(
        "log_file -noappend {}\nset timeout 10\nspawn {} {}\nexpect \"\\033\\[?1049h\"\nsend -- {}\nexpect \"\\033\\[?1049l\"\nexpect eof\nset wait_result [wait]\nexit [lindex $wait_result 3]\n",
        tcl_word(&path_arg(log)),
        tcl_word(program),
        args.iter()
            .map(|arg| tcl_word(arg))
            .collect::<Vec<_>>()
            .join(" "),
        tcl_string(keys),
    );
    let output = Command::new("expect").arg("-c").arg(script).output()?;
    assert_success(output);
    Ok(())
}

fn expect_alt_screen_after_keys(
    log: &Path,
    program: &str,
    args: &[String],
    keys: &str,
    expected: &str,
    exit_keys: &str,
) -> io::Result<()> {
    let script = format!(
        "log_file -noappend {}\nset timeout 10\nspawn {} {}\nexpect \"\\033\\[?1049h\"\nsend -- {}\nexpect {}\nsend -- {}\nexpect \"\\033\\[?1049l\"\nexpect eof\nset wait_result [wait]\nexit [lindex $wait_result 3]\n",
        tcl_word(&path_arg(log)),
        tcl_word(program),
        args.iter()
            .map(|arg| tcl_word(arg))
            .collect::<Vec<_>>()
            .join(" "),
        tcl_string(keys),
        tcl_word(expected),
        tcl_string(exit_keys),
    );
    let output = Command::new("expect").arg("-c").arg(script).output()?;
    assert_success(output);
    assert_alt_screen_log(log)?;
    Ok(())
}

fn file_show(repo: &Path, rev: &str, path: &str) -> io::Result<String> {
    let output = jj(repo).args(["file", "show", "-r", rev, path]).output()?;
    assert_success_ref(&output);
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn rev(repo: &Path) -> io::Result<String> {
    let output = jj(repo)
        .args(["log", "-r", "@", "--no-graph", "-T", "change_id.short()"])
        .output()?;
    assert_success_ref(&output);
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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

fn assert_alt_screen_log(log: &Path) -> io::Result<()> {
    let log = fs::read(log)?;
    assert!(
        log.windows(b"\x1b[?1049h".len())
            .any(|w| w == b"\x1b[?1049h")
    );
    assert!(
        log.windows(b"\x1b[?1049l".len())
            .any(|w| w == b"\x1b[?1049l")
    );
    Ok(())
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}

fn s(value: &str) -> String {
    value.to_owned()
}

fn tcl_word(value: &str) -> String {
    format!("{{{}}}", value.replace('\\', "\\\\").replace('}', "\\}"))
}

fn tcl_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('\r', "\\r")
    )
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn two_hunk_base() -> &'static str {
    "top\nold-a\n1\n2\n3\n4\n5\n6\n7\nold-b\nbottom\n"
}

fn two_hunk_work() -> &'static str {
    "top\nnew-a\n1\n2\n3\n4\n5\n6\n7\nnew-b\nbottom\n"
}

fn second_hunk_selected() -> &'static str {
    "top\nold-a\n1\n2\n3\n4\n5\n6\n7\nnew-b\nbottom\n"
}

fn first_hunk_kept() -> &'static str {
    "top\nnew-a\n1\n2\n3\n4\n5\n6\n7\nold-b\nbottom\n"
}

fn function_hunk_base() -> &'static str {
    "fn demo() {\n    let a = 1;\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n    let b = 1;\n}\n\n\n\n\n\n\n\n\n\n\nfn other() {\n    let c = 1;\n}\n"
}

fn function_hunk_work() -> &'static str {
    "fn demo() {\n    let a = 2;\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n    let b = 2;\n}\n\n\n\n\n\n\n\n\n\n\nfn other() {\n    let c = 2;\n}\n"
}

fn only_other_function_selected() -> &'static str {
    "fn demo() {\n    let a = 1;\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n    let b = 1;\n}\n\n\n\n\n\n\n\n\n\n\nfn other() {\n    let c = 2;\n}\n"
}

fn numbered_lines(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|index| format!("{prefix}-{index:03}\n"))
        .collect()
}

fn temp_root() -> io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "jjc-tty-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root)?;
    Ok(root)
}
