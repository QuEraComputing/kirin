mod language;
mod stage;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "toy-lang")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a .kirin file and pretty-print the IR
    Parse {
        /// Path to the .kirin file
        file: std::path::PathBuf,
    },
    /// Parse and interpret a function
    Run {
        /// Path to the .kirin file
        file: std::path::PathBuf,
        /// Stage name (e.g. "source" or "lowered")
        #[arg(long)]
        stage: String,
        /// Function name (e.g. "main")
        #[arg(long, value_name = "NAME")]
        function: String,
        /// Arguments to the function (parsed as i64)
        args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { file } => {
            println!("TODO: parse {}", file.display());
        }
        Command::Run {
            file,
            stage,
            function,
            args,
        } => {
            println!(
                "TODO: run {} @{} @{} {:?}",
                file.display(),
                stage,
                function,
                args
            );
        }
    }
    Ok(())
}
