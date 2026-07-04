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
    #[command(about = "Edit a single text file, used by jj ui.editor")]
    Edit { file: PathBuf },
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
            Command::Edit { file } => assert_eq!(file, PathBuf::from("message.txt")),
            _ => panic!("expected edit command"),
        }
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
