use clap::Parser;

use jjc::cli::Cli;

fn main() -> std::io::Result<()> {
    jjc::app::run(Cli::parse().command)
}
