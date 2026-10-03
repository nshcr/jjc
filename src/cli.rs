use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

#[derive(Debug, Parser)]
#[command(version, about = "A terminal-native editor for jj")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Check local jjc usability and print recommended jj config")]
    Doctor,
    #[command(about = "Edit a single text file, used by jj ui.editor")]
    Edit {
        file: PathBuf,
        #[arg(long, default_value = "1", help = "Start on this 1-based line")]
        line: NonZeroUsize,
    },
    #[command(about = "Edit a jj diff using left/right/output directories")]
    Diff {
        left: PathBuf,
        right: PathBuf,
        output: PathBuf,
    },
    #[command(about = "Resolve a jj three-way text conflict")]
    Merge {
        left: PathBuf,
        base: PathBuf,
        right: PathBuf,
        output: PathBuf,
        #[arg(long)]
        marker_length: usize,
        #[arg(long)]
        path: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_edit_command() {
        let cli = Cli::try_parse_from(["jjc", "edit", "message.txt"]).unwrap();

        match cli.command {
            Command::Edit { file, line } => {
                assert_eq!(file, PathBuf::from("message.txt"));
                assert_eq!(line.get(), 1);
            }
            _ => panic!("expected edit command"),
        }
    }

    #[test]
    fn parses_editor_line_and_explicit_path() {
        let cli = Cli::try_parse_from([
            "jjc",
            "edit",
            "--line",
            "12",
            "--",
            "a path/message.jjdescription",
        ])
        .unwrap();
        match cli.command {
            Command::Edit { file, line } => {
                assert_eq!(file, PathBuf::from("a path/message.jjdescription"));
                assert_eq!(line.get(), 12);
            }
            _ => panic!("expected edit command"),
        }
        for line in ["0", "-1", "not-a-line"] {
            assert!(Cli::try_parse_from(["jjc", "edit", "--line", line, "message.txt"]).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn preserves_non_utf8_tool_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let paths: Vec<OsString> = [
            b"left\xff".to_vec(),
            b"base\xff".to_vec(),
            b"right\xff".to_vec(),
            b"output\xff".to_vec(),
        ]
        .into_iter()
        .map(OsString::from_vec)
        .collect();
        let edit = Cli::try_parse_from([
            OsString::from("jjc"),
            OsString::from("edit"),
            paths[0].clone(),
        ])
        .unwrap();
        assert!(matches!(edit.command, Command::Edit { file, .. } if file.as_os_str() == paths[0]));

        let diff = Cli::try_parse_from([
            OsString::from("jjc"),
            OsString::from("diff"),
            paths[0].clone(),
            paths[2].clone(),
            paths[3].clone(),
        ])
        .unwrap();
        match diff.command {
            Command::Diff {
                left,
                right,
                output,
            } => {
                assert_eq!(left.as_os_str(), paths[0]);
                assert_eq!(right.as_os_str(), paths[2]);
                assert_eq!(output.as_os_str(), paths[3]);
            }
            _ => panic!("expected diff command"),
        }

        let mut args = vec![OsString::from("jjc"), OsString::from("merge")];
        args.extend(paths.iter().cloned());
        args.extend(["--marker-length", "7", "--path", "src/lib.rs"].map(OsString::from));
        match Cli::try_parse_from(args).unwrap().command {
            Command::Merge {
                left,
                base,
                right,
                output,
                ..
            } => {
                assert_eq!(left.as_os_str(), paths[0]);
                assert_eq!(base.as_os_str(), paths[1]);
                assert_eq!(right.as_os_str(), paths[2]);
                assert_eq!(output.as_os_str(), paths[3]);
            }
            _ => panic!("expected merge command"),
        }
    }

    #[test]
    fn parses_doctor_command() {
        let cli = Cli::try_parse_from(["jjc", "doctor"]).unwrap();

        assert!(matches!(cli.command, Command::Doctor));
    }

    #[test]
    fn parses_diff_command() {
        let cli = Cli::try_parse_from(["jjc", "diff", "left", "right", "output"]).unwrap();

        match cli.command {
            Command::Diff {
                left,
                right,
                output,
            } => {
                assert_eq!(left, PathBuf::from("left"));
                assert_eq!(right, PathBuf::from("right"));
                assert_eq!(output, PathBuf::from("output"));
            }
            _ => panic!("expected diff command"),
        }
    }

    #[test]
    fn parses_merge_command() {
        let cli = Cli::try_parse_from([
            "jjc",
            "merge",
            "left",
            "base",
            "right",
            "output",
            "--marker-length",
            "7",
            "--path",
            "src/lib.rs",
        ])
        .unwrap();

        match cli.command {
            Command::Merge {
                left,
                base,
                right,
                output,
                marker_length,
                path,
            } => {
                assert_eq!(left, PathBuf::from("left"));
                assert_eq!(base, PathBuf::from("base"));
                assert_eq!(right, PathBuf::from("right"));
                assert_eq!(output, PathBuf::from("output"));
                assert_eq!(marker_length, 7);
                assert_eq!(path, "src/lib.rs");
            }
            _ => panic!("expected merge command"),
        }
    }

    #[test]
    fn rejects_malformed_commands() {
        assert!(Cli::try_parse_from(["jjc"]).is_err());
        assert!(
            Cli::try_parse_from([
                "jjc",
                "merge",
                "left",
                "base",
                "right",
                "output",
                "--marker-length",
                "7",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "jjc",
                "merge",
                "left",
                "base",
                "right",
                "output",
                "--marker-length",
                "bad",
                "--path",
                "file.txt",
            ])
            .is_err()
        );
    }
}
