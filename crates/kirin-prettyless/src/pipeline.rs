//! Pipeline-wide pretty printing support.
//!
//! Provides [`RenderStage`] for type-erased per-function rendering within a
//! stage, and [`PipelineDocument`] for printing a specific function across all
//! stages in a pipeline.

use std::io::{Write, stdout};

use kirin_ir::{Dialect, Function, GlobalSymbol, InternTable, Pipeline, StageInfo, StagedFunction};

use crate::{Config, Document, PrettyPrint, ScanResultWidth};

/// Trait for rendering a specific staged function within a compilation stage.
///
/// This provides type-erased rendering: the inner dialect type `L` is hidden
/// behind the trait, allowing pipeline-wide iteration over heterogeneous stages.
///
/// The stage prefix (e.g., `stage @A`) is derived from the context's own
/// identity (name and/or stage ID), so callers don't need to supply it.
///
/// A generic blanket implementation is provided for [`StageInfo<L>`] where
/// `L: Dialect + PrettyPrint`. User stage enums delegate to this via simple
/// match arms:
///
/// ```ignore
/// impl RenderStage for Stage {
///     fn render_staged_function(
///         &self, sf: StagedFunction, config: &Config,
///         gs: &InternTable<String, GlobalSymbol>,
///     ) -> Result<Option<String>, std::fmt::Error> {
///         match self {
///             Stage::A(ctx) => ctx.render_staged_function(sf, config, gs),
///             Stage::B(ctx) => ctx.render_staged_function(sf, config, gs),
///         }
///     }
/// }
/// ```
pub trait RenderStage {
    /// Render a staged function by its [`StagedFunction`] ID.
    ///
    /// The stage prefix is derived from the context's own identity, so no
    /// external prefix parameter is needed.
    ///
    /// Returns `Ok(Some(rendered))` if the staged function exists in this
    /// stage, `Ok(None)` if this stage doesn't own that ID.
    fn render_staged_function(
        &self,
        sf: StagedFunction,
        config: &Config,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) -> Result<Option<String>, std::fmt::Error>;
}

/// Generic blanket implementation: any `StageInfo<L>` where `L` supports pretty
/// printing automatically gets `RenderStage`.
impl<L> RenderStage for StageInfo<L>
where
    L: Dialect + PrettyPrint,
    L::Type: std::fmt::Display,
    for<'a> StagedFunction: ScanResultWidth<L>,
{
    fn render_staged_function(
        &self,
        sf: StagedFunction,
        config: &Config,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) -> Result<Option<String>, std::fmt::Error> {
        let Some(info) = self.staged_function_arena().get(sf) else {
            return Ok(None);
        };
        if info.is_invalidated() {
            return Ok(None);
        }
        let mut doc = Document::with_global_symbols(config.clone(), self, global_symbols);
        let rendered = doc.render(&sf)?;
        Ok(Some(rendered))
    }
}

/// Pipeline-wide document for printing a specific function across all stages.
///
/// Given a [`Function`] ID, looks up its [`FunctionInfo`](kirin_ir::FunctionInfo)
/// to find the staged functions at each compilation stage, then renders each one.
pub struct PipelineDocument<'a, S> {
    config: Config,
    pipeline: &'a Pipeline<S>,
}

impl<'a, S: RenderStage> PipelineDocument<'a, S> {
    /// Create a new pipeline document.
    pub fn new(config: Config, pipeline: &'a Pipeline<S>) -> Self {
        Self { config, pipeline }
    }

    /// Render a function across all stages where it has a staged representation.
    ///
    /// Looks up the [`FunctionInfo`](kirin_ir::FunctionInfo) for the given
    /// [`Function`] and renders each staged function in its corresponding stage,
    /// separated by blank lines. The stage prefix (e.g., `stage @A`) is derived
    /// from each context's own identity — no external prefix is needed.
    pub fn render_function(&self, func: Function) -> Result<String, std::fmt::Error> {
        let gs = self.pipeline.global_symbols();
        let func_info = self
            .pipeline
            .function_info(func)
            .expect("Function ID not found in pipeline");

        let mut output = String::new();
        for (&stage_id, &sf_id) in func_info.staged_functions() {
            if let Some(stage) = self.pipeline.stage(stage_id) {
                if let Some(rendered) = stage.render_staged_function(sf_id, &self.config, gs)? {
                    if !output.is_empty() {
                        output.push_str("\n\n");
                    }
                    output.push_str(rendered.trim_end_matches('\n'));
                }
            }
        }
        Ok(output)
    }
}

/// Extension trait for cross-stage printing on [`Function`] IDs.
///
/// Provides convenience methods to render a function across all stages in a
/// pipeline without manually constructing a [`PipelineDocument`].
///
/// ```ignore
/// let output = func.sprint(&pipeline);
/// func.print(&pipeline);
/// ```
pub trait FunctionPrintExt {
    /// Render a function across all stages to a string with default config.
    fn sprint<S: RenderStage>(&self, pipeline: &Pipeline<S>) -> String;

    /// Render a function across all stages to a string with custom config.
    fn sprint_with_config<S: RenderStage>(&self, config: Config, pipeline: &Pipeline<S>) -> String;

    /// Print a function across all stages to stdout with default config.
    fn print<S: RenderStage>(&self, pipeline: &Pipeline<S>);

    /// Print a function across all stages to stdout with custom config.
    fn print_with_config<S: RenderStage>(&self, config: Config, pipeline: &Pipeline<S>);

    /// Write a function across all stages to a writer with default config.
    fn write<S: RenderStage>(&self, writer: &mut impl Write, pipeline: &Pipeline<S>);
}

impl FunctionPrintExt for Function {
    fn sprint<S: RenderStage>(&self, pipeline: &Pipeline<S>) -> String {
        PipelineDocument::new(Config::default(), pipeline)
            .render_function(*self)
            .expect("render failed")
    }

    fn sprint_with_config<S: RenderStage>(&self, config: Config, pipeline: &Pipeline<S>) -> String {
        PipelineDocument::new(config, pipeline)
            .render_function(*self)
            .expect("render failed")
    }

    fn print<S: RenderStage>(&self, pipeline: &Pipeline<S>) {
        let output = self.sprint(pipeline);
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    fn print_with_config<S: RenderStage>(&self, config: Config, pipeline: &Pipeline<S>) {
        let output = self.sprint_with_config(config, pipeline);
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    fn write<S: RenderStage>(&self, writer: &mut impl Write, pipeline: &Pipeline<S>) {
        let output = self.sprint(pipeline);
        writer.write_all(output.as_bytes()).expect("write failed");
    }
}
