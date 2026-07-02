#![cfg(unix)]

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
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

    let script = format!(
        "log_file -noappend {}\nset timeout 5\nspawn {} edit {}\nexpect \"\\033\\[?1049h\"\nsend \":q!\\r\"\nexpect eof\n",
        tcl_path(&log),
        tcl_path(Path::new(jjc())),
        tcl_path(&message)
    );
    let output = Command::new("expect").arg("-c").arg(script).output()?;
    assert!(
        output.status.success(),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let log = fs::read(log)?;
    assert!(
        log.windows(b"\x1b[?1049h".len())
            .any(|w| w == b"\x1b[?1049h")
    );
    assert!(
        log.windows(b"\x1b[?1049l".len())
            .any(|w| w == b"\x1b[?1049l")
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

fn expect_available() -> bool {
    Command::new("expect").arg("-v").output().is_ok()
}

fn jjc() -> &'static str {
    env!("CARGO_BIN_EXE_jjc")
}

fn tcl_path(path: &Path) -> String {
    format!("{{{}}}", path.display())
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
