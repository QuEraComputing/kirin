//! Program-wide pretty printing support.
//!
//! Provides [`RenderDispatch`] for type-erased per-function rendering within a
//! stage, and [`ProgramDocument`] for printing a specific function across all
//! stages in a program.

use std::io::{Write, stdout};

use kirin_ir::{Dialect, Function, GlobalSymbol, InternTable, Program, StageInfo, StagedFunction};

use crate::{Config, Document, PrettyPrint, RenderError};

/// Trait for rendering a specific staged function within a compilation stage.
///
/// This provides type-erased rendering: the inner dialect type `L` is hidden
/// behind the trait, allowing program-wide iteration over heterogeneous stages.
///
/// The stage prefix (e.g., `stage @A`) is derived from the context's own
/// identity (name and/or stage ID), so callers don't need to supply it.
///
/// A generic blanket implementation is provided for [`StageInfo<L>`] where
/// `L: Dialect + PrettyPrint`. User stage enums delegate to this via simple
/// match arms:
///
/// ```ignore
/// impl RenderDispatch for Stage {
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
pub trait RenderDispatch {
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
/// printing automatically gets `RenderDispatch`.
impl<L> RenderDispatch for StageInfo<L>
where
    L: Dialect + PrettyPrint,
    L::Type: std::fmt::Display,
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

/// Program-wide document for printing a specific function across all stages.
///
/// Given a [`Function`] ID, looks up its [`FunctionInfo`](kirin_ir::FunctionInfo)
/// to find the staged functions at each compilation stage, then renders each one.
#[must_use]
pub struct ProgramDocument<'a, S> {
    config: Config,
    program: &'a Program<S>,
}

impl<'a, S: RenderDispatch> ProgramDocument<'a, S> {
    /// Create a new program document.
    pub fn new(config: Config, program: &'a Program<S>) -> Self {
        Self { config, program }
    }

    /// Render a function across all stages where it has a staged representation.
    ///
    /// Looks up the [`FunctionInfo`](kirin_ir::FunctionInfo) for the given
    /// [`Function`] and renders each staged function in its corresponding stage,
    /// separated by blank lines. The stage prefix (e.g., `stage @A`) is derived
    /// from each context's own identity — no external prefix is needed.
    pub fn render_function(&self, func: Function) -> Result<String, RenderError> {
        let gs = self.program.global_symbols();
        let func_info = self
            .program
            .function_info(func)
            .ok_or(RenderError::UnknownFunction(func))?;

        let mut output = String::new();
        for (&stage_id, &sf_id) in func_info.staged_functions() {
            // A stage a rewrite pass took or poisoned cannot be rendered.
            // `Quarantined::report` exists for that case. Be explicit rather than
            // dropping the stage.
            let Some(stage) = self.program.stage(stage_id) else {
                let note = match self.program.stage_status(stage_id) {
                    Some(status) => format!("// <stage {stage_id:?} unavailable: {status:?}>"),
                    None => format!("// <stage {stage_id:?} not in program>"),
                };
                if !output.is_empty() {
                    output.push_str("\n\n");
                }
                output.push_str(&note);
                continue;
            };
            if let Some(rendered) = stage.render_staged_function(sf_id, &self.config, gs)? {
                if !output.is_empty() {
                    output.push_str("\n\n");
                }
                output.push_str(rendered.trim_end_matches('\n'));
            }
        }
        Ok(output)
    }
}

/// Builder for rendering a specific [`Function`] across program stages.
#[must_use = "call .into_string(), .print(), or .bat() to produce output"]
pub struct FunctionRenderBuilder<'a, S> {
    function: Function,
    program: &'a Program<S>,
    config: Config,
}

impl<'a, S: RenderDispatch> FunctionRenderBuilder<'a, S> {
    /// Set custom rendering configuration.
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Render to a string, consuming the builder.
    pub fn into_string(self) -> Result<String, RenderError> {
        ProgramDocument::new(self.config, self.program).render_function(self.function)
    }

    /// Write to a writer.
    pub fn write_to(self, writer: &mut impl Write) -> Result<(), RenderError> {
        let output = self.into_string()?;
        writer.write_all(output.as_bytes())?;
        Ok(())
    }

    /// Print to stdout.
    pub fn print(self) -> Result<(), RenderError> {
        let output = self.into_string()?;
        stdout().write_all(output.as_bytes())?;
        Ok(())
    }

    /// Display with bat pager.
    #[cfg(feature = "bat")]
    pub fn bat(self) -> Result<(), RenderError> {
        crate::bat::print_str(&self.into_string()?)?;
        Ok(())
    }
}

/// Builder for rendering all functions in a [`Program`].
#[must_use = "call .into_string(), .print(), or .bat() to produce output"]
pub struct ProgramRenderBuilder<'a, S> {
    program: &'a Program<S>,
    config: Config,
}

impl<'a, S: RenderDispatch> ProgramRenderBuilder<'a, S> {
    /// Set custom rendering configuration.
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Render to a string, consuming the builder.
    pub fn into_string(self) -> Result<String, RenderError> {
        let doc = ProgramDocument::new(self.config, self.program);
        let mut parts = Vec::new();
        for func_info in self.program.function_arena().iter() {
            let rendered = doc.render_function(func_info.id())?;
            if !rendered.is_empty() {
                parts.push(rendered);
            }
        }
        Ok(parts.join("\n"))
    }

    /// Write to a writer.
    pub fn write_to(self, writer: &mut impl Write) -> Result<(), RenderError> {
        let output = self.into_string()?;
        writer.write_all(output.as_bytes())?;
        Ok(())
    }

    /// Print to stdout.
    pub fn print(self) -> Result<(), RenderError> {
        let output = self.into_string()?;
        stdout().write_all(output.as_bytes())?;
        Ok(())
    }

    /// Display with bat pager.
    #[cfg(feature = "bat")]
    pub fn bat(self) -> Result<(), RenderError> {
        crate::bat::print_str(&self.into_string()?)?;
        Ok(())
    }
}

/// Extension trait for cross-stage printing on [`Function`] IDs.
pub trait PrintExt {
    /// Create a builder for rendering this function across all stages.
    fn render<'a, S: RenderDispatch>(
        &self,
        program: &'a Program<S>,
    ) -> FunctionRenderBuilder<'a, S>;

    /// Convenience shorthand: render to string with default config.
    fn sprint<S: RenderDispatch>(&self, program: &Program<S>) -> String {
        self.render(program)
            .into_string()
            .unwrap_or_else(|e| panic!("render failed: {e}"))
    }
}

impl PrintExt for Function {
    fn render<'a, S: RenderDispatch>(
        &self,
        program: &'a Program<S>,
    ) -> FunctionRenderBuilder<'a, S> {
        FunctionRenderBuilder {
            function: *self,
            program,
            config: Config::default(),
        }
    }
}

/// Extension trait for printing all functions in a [`Program`].
pub trait ProgramPrintExt {
    type Stage: RenderDispatch;

    /// Create a builder for rendering every function in the program.
    fn render(&self) -> ProgramRenderBuilder<'_, Self::Stage>;

    /// Convenience shorthand: render to string with default config.
    fn sprint(&self) -> String {
        self.render()
            .into_string()
            .unwrap_or_else(|e| panic!("render failed: {e}"))
    }
}

impl<S: RenderDispatch> ProgramPrintExt for Program<S> {
    type Stage = S;

    fn render(&self) -> ProgramRenderBuilder<'_, Self::Stage> {
        ProgramRenderBuilder {
            program: self,
            config: Config::default(),
        }
    }
}
