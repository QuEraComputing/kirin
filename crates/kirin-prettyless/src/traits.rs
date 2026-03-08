//! Core traits for pretty printing.

use std::io::{Write, stdout};

use crate::{ArenaDoc, Config, Document, RenderError};
use kirin_ir::{Dialect, GlobalSymbol, InternTable, StageInfo};

/// Core trait for pretty printing values to a document.
///
/// This trait defines how a type should be rendered to a document representation.
/// The method is generic over the dialect type `L`, allowing the same implementation
/// to work with any `Document<L>`.
///
/// The target invariant is that `parse(sprint(ir)) == ir` — pretty-printing
/// should produce output that roundtrips through the parser.
///
/// The bounds on `L` (`PrettyPrint` and `Type: Display`) are required because:
/// - `L: PrettyPrint` is needed to print nested Block/Region structures
/// - `Type: Display` is needed to print type annotations (`:type` format)
///
/// For IR nodes that require context (like `Statement`, `Block`, `Region`), use
/// the convenience methods provided on `Document` instead.
///
/// # Example
///
/// ```ignore
/// impl PrettyPrint for MyType {
///     fn pretty_print<'a, L: Dialect + PrettyPrint>(
///         &self,
///         doc: &'a Document<'a, L>,
///     ) -> ArenaDoc<'a>
///     where
///         L::Type: std::fmt::Display,
///     {
///         doc.text(format!("MyType({})", self.value))
///     }
/// }
/// ```
pub trait PrettyPrint {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        self.namespaced_pretty_print(doc, &[])
    }

    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display;

    /// Pretty print only the "name" view of a value.
    ///
    /// For most types this is identical to [`PrettyPrint::pretty_print`].
    fn pretty_print_name<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        self.pretty_print(doc)
    }

    /// Pretty print only the "type" view of a value.
    ///
    /// For most types this is identical to [`PrettyPrint::pretty_print`].
    fn pretty_print_type<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        self.pretty_print(doc)
    }
}

/// Builder for rendering pretty-printed IR nodes.
///
/// Created via [`PrettyPrintExt::render`]. Allows optional configuration
/// and global symbol resolution before producing output.
///
/// # Example
///
/// ```ignore
/// // Simple usage
/// let output = node.sprint(&stage);
///
/// // Builder usage with options
/// let output = node.render(&stage)
///     .config(Config::default().with_width(80))
///     .globals(&global_symbols)
///     .to_string();
///
/// // Print to stdout
/// node.render(&stage).print();
///
/// // Display with bat pager
/// node.render(&stage).bat();
/// ```
pub struct RenderBuilder<'n, 's, N, L: Dialect> {
    node: &'n N,
    stage: &'s StageInfo<L>,
    config: Config,
    global_symbols: Option<&'s InternTable<String, GlobalSymbol>>,
}

impl<'n, 's, N: PrettyPrint, L: Dialect + PrettyPrint> RenderBuilder<'n, 's, N, L>
where
    L::Type: std::fmt::Display,
{
    /// Set custom configuration for rendering.
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Set global symbol table for resolving function names.
    pub fn globals(mut self, global_symbols: &'s InternTable<String, GlobalSymbol>) -> Self {
        self.global_symbols = Some(global_symbols);
        self
    }

    /// Render to a string.
    pub fn to_string(self) -> Result<String, RenderError> {
        let node = self.node;
        let mut doc = self.into_document();
        Ok(doc.render(node)?)
    }

    /// Write to a writer.
    pub fn write_to(self, writer: &mut impl Write) -> Result<(), RenderError> {
        let output = self.to_string()?;
        writer.write_all(output.as_bytes())?;
        Ok(())
    }

    /// Print to stdout.
    pub fn print(self) -> Result<(), RenderError> {
        let output = self.to_string()?;
        stdout().write_all(output.as_bytes())?;
        Ok(())
    }

    /// Display with bat pager.
    #[cfg(feature = "bat")]
    pub fn bat(self) -> Result<(), RenderError> {
        let node = self.node;
        let mut doc = self.into_document();
        doc.pager(node)?;
        Ok(())
    }

    fn into_document(self) -> Document<'s, L> {
        match self.global_symbols {
            Some(gs) => Document::with_global_symbols(self.config, self.stage, gs),
            None => Document::new(self.config, self.stage),
        }
    }
}

/// Extension trait providing convenience methods for pretty printing IR nodes.
///
/// This trait is automatically implemented for any type that implements
/// `PrettyPrint`. Use [`render`](PrettyPrintExt::render) for the builder API,
/// or [`sprint`](PrettyPrintExt::sprint) as a shorthand for rendering to string.
///
/// # Example
///
/// ```ignore
/// use kirin_prettyless::PrettyPrintExt;
///
/// // Shorthand
/// let output = statement.sprint(&stage);
///
/// // Builder
/// let output = statement.render(&stage)
///     .config(Config::default().with_width(80))
///     .globals(&gs)
///     .to_string();
/// ```
pub trait PrettyPrintExt<L: Dialect + PrettyPrint>: PrettyPrint
where
    L::Type: std::fmt::Display,
{
    /// Create a render builder for this node.
    fn render<'n, 's>(&'n self, stage: &'s StageInfo<L>) -> RenderBuilder<'n, 's, Self, L>
    where
        Self: Sized;

    /// Convenience shorthand: render to string with default config.
    fn sprint(&self, stage: &StageInfo<L>) -> String;
}

// Blanket implementation: any type that implements PrettyPrint
// automatically gets the context-aware convenience methods.
impl<L: Dialect + PrettyPrint, T: PrettyPrint> PrettyPrintExt<L> for T
where
    L::Type: std::fmt::Display,
{
    fn render<'n, 's>(&'n self, stage: &'s StageInfo<L>) -> RenderBuilder<'n, 's, Self, L> {
        RenderBuilder {
            node: self,
            stage,
            config: Config::default(),
            global_symbols: None,
        }
    }

    fn sprint(&self, stage: &StageInfo<L>) -> String {
        self.render(stage).to_string().expect("render failed")
    }
}

impl<T: PrettyPrint> PrettyPrint for &T {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        T::namespaced_pretty_print(self, doc, namespace)
    }

    fn pretty_print_name<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        (*self).pretty_print_name(doc)
    }

    fn pretty_print_type<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        (*self).pretty_print_type(doc)
    }
}
