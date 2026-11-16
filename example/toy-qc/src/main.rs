mod circuit;
mod stage;
mod types;
mod zx;

use clap::{Parser, Subcommand};
use kirin::prelude::*;
use kirin::pretty::ProgramPrintExt;

use stage::Stage;

#[derive(Parser)]
#[command(name = "toy-qc")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a .kirin file and pretty-print the IR
    Parse {
        /// Path to the .kirin file
        file: std::path::PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse { file } => {
            let src = std::fs::read_to_string(&file)?;
            let mut program: Program<Stage> = Program::new();
            ParseProgramText::parse(&mut program, &src)?;
            let rendered = program.sprint();
            print!("{rendered}");
            Ok(())
        }
    }
}

#[cfg(all(test, feature = "serde"))]
mod json_tests;
#[cfg(test)]
mod parse_tests;
