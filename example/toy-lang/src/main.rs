mod interpreter;
mod language;
mod stage;

use clap::{Parser, Subcommand};
use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;

use stage::Stage;

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
        /// Stage name to enter at (e.g. "source" or "lowered").
        ///
        /// Calls from the entered function may cross into other stages
        /// unless `--per-language` is set.
        #[arg(long)]
        stage: String,
        /// Function name (e.g. "main")
        #[arg(long, value_name = "NAME")]
        function: String,
        /// Arguments to the function (parsed as i64)
        #[arg(allow_negative_numbers = true)]
        args: Vec<String>,
        /// Run constant propagation instead of concrete execution.
        #[arg(long)]
        constprop: bool,
        /// Run liveness analysis (strong demand + classic per-point) instead
        /// of concrete execution.
        #[arg(long)]
        liveness: bool,
        /// Restrict execution to the entry stage's language; reject calls
        /// that would dispatch into a different stage. Default is cross-
        /// language.
        #[arg(long)]
        per_language: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { file } => {
            let src = std::fs::read_to_string(&file)?;
            let mut pipeline: Pipeline<Stage> = Pipeline::new();
            ParsePipelineText::parse(&mut pipeline, &src)?;
            let rendered = pipeline.sprint();
            print!("{rendered}");
            Ok(())
        }
        Command::Run {
            file,
            stage: stage_name,
            function: func_name,
            args,
            constprop,
            liveness,
            per_language,
        } => run_program(
            &file,
            &stage_name,
            &func_name,
            &args,
            constprop,
            liveness,
            per_language,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_program(
    file: &std::path::Path,
    stage_name: &str,
    func_name: &str,
    cli_args: &[String],
    constprop: bool,
    liveness: bool,
    per_language: bool,
) -> anyhow::Result<()> {
    let src = std::fs::read_to_string(file)?;
    let mut pipeline: Pipeline<Stage> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, &src)?;

    let args: Vec<i64> = cli_args
        .iter()
        .map(|s| s.parse::<i64>())
        .collect::<Result<_, _>>()?;

    if liveness {
        let (demand, dense) = interpreter::analyze_liveness(&pipeline, stage_name, func_name)?;
        println!("demanded: {:?}", demand.demanded());
        let mut boundaries: Vec<_> = dense
            .blocks()
            .map(|(block, live_in, live_out)| format!("{block:?}: in={live_in:?} out={live_out:?}"))
            .collect();
        boundaries.sort();
        for line in boundaries {
            println!("{line}");
        }
        return Ok(());
    }

    if constprop {
        let abstract_args = args
            .iter()
            .copied()
            .map(kirin_constprop::ConstPropValue::Const)
            .collect::<Vec<_>>();
        let result =
            interpreter::analyze_constprop(&pipeline, stage_name, func_name, &abstract_args)?;
        println!("{result:?}");
        return Ok(());
    }

    let result = if per_language {
        match stage_name {
            "source" => interpreter::run_source_i64(&pipeline, func_name, &args)?,
            "lowered" => interpreter::run_lowered_i64(&pipeline, func_name, &args)?,
            other => anyhow::bail!("unknown stage '{}'", other),
        }
    } else {
        interpreter::run_i64(&pipeline, stage_name, func_name, &args)?
    };
    println!("{result}");
    Ok(())
}
