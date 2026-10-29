mod interpreter;
mod language;
mod stage;

use clap::{Parser, Subcommand};
use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;
use kirin_interpreter::{ProgramPoint, Scoped};

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
        /// Run classic per-program-point liveness analysis (dense backward)
        /// instead of concrete execution.
        #[arg(long)]
        liveness: bool,
        /// Run strong liveness (sparse backward demand) instead of concrete
        /// execution.
        #[arg(long)]
        demand: bool,
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
            demand,
            per_language,
        } => run_program(
            &file,
            &stage_name,
            &func_name,
            &args,
            constprop,
            liveness,
            demand,
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
    demand: bool,
    per_language: bool,
) -> anyhow::Result<()> {
    let src = std::fs::read_to_string(file)?;
    let mut pipeline: Pipeline<Stage> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, &src)?;

    let args: Vec<i64> = cli_args
        .iter()
        .map(|s| s.parse::<i64>())
        .collect::<Result<_, _>>()?;

    if [constprop, liveness, demand]
        .iter()
        .filter(|&&flag| flag)
        .count()
        > 1
    {
        anyhow::bail!("--constprop, --liveness and --demand are mutually exclusive");
    }

    if demand {
        let result = interpreter::analyze_demand(&pipeline, stage_name, func_name)?;
        let demanded = result.demanded().iter().copied();
        let demanded = match pipeline
            .stage(result.root_scope().0)
            .ok_or_else(|| anyhow::anyhow!("resolved stage is missing"))?
        {
            Stage::Source(info) => value_set(info, demanded),
            Stage::Lowered(info) => value_set(info, demanded),
        };
        println!("demanded: {demanded}");
        return Ok(());
    }

    if liveness {
        let (stage, cfg, dense) =
            interpreter::analyze_classic_liveness(&pipeline, stage_name, func_name)?;
        match pipeline
            .stage(stage)
            .ok_or_else(|| anyhow::anyhow!("resolved stage is missing"))?
        {
            Stage::Source(info) => print_block_liveness(info, cfg, &dense),
            Stage::Lowered(info) => print_block_liveness(info, cfg, &dense),
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

/// Print each block's live-in and live-out sets, in block order, by source
/// name.
fn print_block_liveness<L: Dialect>(
    info: &StageInfo<L>,
    cfg: CFG,
    dense: &kirin_liveness::DenseLivenessResult,
) {
    let scope = dense.root_scope();
    for block in cfg.blocks(info) {
        let point = |item| dense.point_facts(Scoped::new(scope, item));
        let (Some(live_in), Some(live_out)) = (
            point(ProgramPoint::BlockEntry(block)),
            point(ProgramPoint::BlockExit(block)),
        ) else {
            continue;
        };
        let name = block
            .expect_info(info)
            .name()
            .and_then(|name| info.symbol_table().resolve(name))
            .map_or_else(|| format!("{block:?}"), |name| format!("^{name}"));
        println!(
            "{name}: in={} out={}",
            value_set(info, live_in.iter()),
            value_set(info, live_out.iter())
        );
    }
}

/// `{%a, %b}`: `values` ordered by raw SSA id, by source name (the raw id
/// when a value is unnamed).
fn value_set<L: Dialect>(
    info: &StageInfo<L>,
    values: impl IntoIterator<Item = SSAValue>,
) -> String {
    let mut values: Vec<_> = values.into_iter().collect();
    values.sort_by_key(|value| Id::from(*value).raw());
    let names: Vec<_> = values
        .iter()
        .map(|value| {
            value
                .expect_info(info)
                .name()
                .and_then(|name| info.symbol_table().resolve(name))
                .map_or_else(|| format!("{value:?}"), |name| format!("%{name}"))
        })
        .collect();
    format!("{{{}}}", names.join(", "))
}
