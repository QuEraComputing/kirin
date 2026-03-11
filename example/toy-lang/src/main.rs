mod language;
mod stage;

use clap::{Parser, Subcommand};
use kirin::interpreter::{StackInterpreter, StageAccess};
use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;

use language::{HighLevel, LowLevel};
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
        /// Stage name (e.g. "source" or "lowered")
        #[arg(long)]
        stage: String,
        /// Function name (e.g. "main")
        #[arg(long, value_name = "NAME")]
        function: String,
        /// Arguments to the function (parsed as i64)
        #[arg(allow_negative_numbers = true)]
        args: Vec<String>,
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
        } => {
            run_program(&file, &stage_name, &func_name, &args)?;
            Ok(())
        }
    }
}

fn run_program(
    file: &std::path::Path,
    stage_name: &str,
    func_name: &str,
    cli_args: &[String],
) -> anyhow::Result<()> {
    let src = std::fs::read_to_string(file)?;
    let mut pipeline: Pipeline<Stage> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, &src)?;

    // Find the stage by name.
    let stage_id = pipeline
        .stages()
        .iter()
        .find_map(|s| {
            let sym = s.stage_name()?;
            let name = pipeline.resolve(sym)?;
            if name == stage_name {
                s.stage_id()
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("stage '{}' not found", stage_name))?;

    // Find the function by name.
    let global_sym = pipeline
        .lookup_symbol(func_name)
        .ok_or_else(|| anyhow::anyhow!("function '{}' not found", func_name))?;
    let function = pipeline
        .function_by_name(global_sym)
        .ok_or_else(|| anyhow::anyhow!("function '{}' not found", func_name))?;
    let func_info = pipeline
        .function_info(function)
        .ok_or_else(|| anyhow::anyhow!("function '{}' info not found", func_name))?;
    let staged_function = func_info
        .staged_functions()
        .get(&stage_id)
        .copied()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "function '{}' not found in stage '{}'",
                func_name,
                stage_name
            )
        })?;

    // Parse CLI args as i64.
    let args: Vec<i64> = cli_args
        .iter()
        .map(|s| s.parse::<i64>())
        .collect::<Result<_, _>>()?;

    // Dispatch based on stage name.
    match stage_name {
        "source" => {
            let spec = resolve_specialization::<HighLevel>(
                &pipeline,
                stage_id,
                staged_function,
                func_name,
            )?;
            let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
            let result = interp.in_stage::<HighLevel>().call(spec, &args)?;
            println!("{result}");
        }
        "lowered" => {
            let spec = resolve_specialization::<LowLevel>(
                &pipeline,
                stage_id,
                staged_function,
                func_name,
            )?;
            let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
            let result = interp.in_stage::<LowLevel>().call(spec, &args)?;
            println!("{result}");
        }
        other => {
            anyhow::bail!("unknown stage '{}'", other);
        }
    }

    Ok(())
}

fn resolve_specialization<L: Dialect>(
    pipeline: &Pipeline<Stage>,
    stage_id: CompileStage,
    staged_function: StagedFunction,
    func_name: &str,
) -> anyhow::Result<SpecializedFunction>
where
    Stage: HasStageInfo<L>,
{
    let stage_meta = pipeline
        .stage(stage_id)
        .ok_or_else(|| anyhow::anyhow!("stage not found"))?;
    let stage_info: &StageInfo<L> = stage_meta
        .try_stage_info()
        .ok_or_else(|| anyhow::anyhow!("stage type mismatch"))?;
    let staged_info = staged_function
        .get_info(stage_info)
        .ok_or_else(|| anyhow::anyhow!("function '{}' has no staged info", func_name))?;
    let spec = staged_info
        .specializations()
        .iter()
        .find(|s| !s.is_invalidated())
        .ok_or_else(|| anyhow::anyhow!("function '{}' has no active specialization", func_name))?;
    Ok(spec.id())
}
