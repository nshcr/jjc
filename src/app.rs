use std::io;
use std::path::Path;

use crossterm::cursor::SetCursorStyle;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::cli::Command;
use crate::diff::DiffApp;
use crate::doctor;
use crate::editor::Editor;
use crate::input;
use crate::merge::MergeApp;

pub fn run(command: Command) -> io::Result<()> {
    match command {
        Command::Doctor => doctor::run(),
        Command::Edit { file } => {
            ensure_file(&file)?;
            let mut editor = Editor::open(file)?;
            if input::scripted() {
                return editor.run_scripted();
            }
            let mut terminal = TerminalSession::start()?;
            editor.run(terminal.terminal_mut())
        }
        Command::Diff {
            left,
            right,
            output,
        } => {
            ensure_dir(&left)?;
            ensure_dir(&right)?;
            ensure_dir(&output)?;
            let mut diff = DiffApp::open(left, right, output)?;
            if input::scripted() {
                return diff.run_scripted();
            }
            let mut terminal = TerminalSession::start()?;
            diff.run(terminal.terminal_mut())
        }
        Command::Merge {
            left,
            base,
            right,
            output,
            marker_length,
            path,
        } => {
            ensure_file(&left)?;
            ensure_file(&base)?;
            ensure_file(&right)?;
            ensure_parent(&output)?;
            let mut merge = MergeApp::open(left, base, right, output, marker_length, path)?;
            if input::scripted() {
                return merge.run_scripted();
            }
            let mut terminal = TerminalSession::start()?;
            merge.run(terminal.terminal_mut())
        }
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalSession {
    fn start() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(err);
        }
        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(err) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                return Err(err);
            }
        };
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            SetCursorStyle::DefaultUserShape,
            LeaveAlternateScreen
        );
    }
}

fn ensure_file(path: &Path) -> io::Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected file: {}", path.display()),
        ))
    }
}

fn ensure_dir(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected directory: {}", path.display()),
        ))
    }
}

fn ensure_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() || parent.is_dir() => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected output parent directory: {}", path.display()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    #[test]
    fn validates_files_dirs_and_output_parent() {
        let root = temp_root();
        let file = root.join("file.txt");
        let dir = root.join("dir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&file, "").unwrap();

        assert!(ensure_file(&file).is_ok());
        assert!(ensure_file(&dir).is_err());
        assert!(ensure_dir(&dir).is_ok());
        assert!(ensure_dir(&file).is_err());
        assert!(ensure_parent(&dir.join("out.txt")).is_ok());
        assert!(ensure_parent(Path::new("out.txt")).is_ok());
        assert!(ensure_parent(&root.join("missing").join("out.txt")).is_err());

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "jjc-app-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
