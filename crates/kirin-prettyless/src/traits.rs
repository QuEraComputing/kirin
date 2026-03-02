//! Core traits for pretty printing.

use std::io::{Write, stdout};

use kirin_ir::{Dialect, GlobalSymbol, InternTable, StageInfo};
use prettyless::DocBuilder;

use crate::{ArenaDoc, Config, Document};

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
        L::Type: std::fmt::Display;
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
    pub fn to_string(self) -> String {
        let node = self.node;
        let mut doc = self.into_document();
        doc.render(node).expect("render failed")
    }

    /// Write to a writer.
    pub fn write_to(self, writer: &mut impl Write) {
        let output = self.to_string();
        writer.write_all(output.as_bytes()).expect("write failed");
    }

    /// Print to stdout.
    pub fn print(self) {
        let output = self.to_string();
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    /// Display with bat pager.
    #[cfg(feature = "bat")]
    pub fn bat(self) {
        let node = self.node;
        let mut doc = self.into_document();
        doc.pager(node).expect("pager failed");
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
        self.render(stage).to_string()
    }
}

/// Trait for types that can print their name (e.g., SSA values).
pub trait PrettyPrintName {
    /// Pretty print just the name part (e.g., `%x` for an SSA value).
    fn pretty_print_name<'a, L: Dialect>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> DocBuilder<'a, prettyless::Arena<'a>>;
}

/// Trait for types that can print their type annotation.
pub trait PrettyPrintType {
    /// Pretty print just the type part (e.g., `i32` for a typed value).
    fn pretty_print_type<'a, L: Dialect>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> DocBuilder<'a, prettyless::Arena<'a>>
    where
        L::Type: std::fmt::Display;
}

// Blanket impls for references - allows calling on &T when the impl is on T
impl<T: PrettyPrintName> PrettyPrintName for &T {
    fn pretty_print_name<'a, L: Dialect>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> DocBuilder<'a, prettyless::Arena<'a>> {
        (*self).pretty_print_name(doc)
    }
}

impl<T: PrettyPrintType> PrettyPrintType for &T {
    fn pretty_print_type<'a, L: Dialect>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> DocBuilder<'a, prettyless::Arena<'a>>
    where
        L::Type: std::fmt::Display,
    {
        (*self).pretty_print_type(doc)
    }
}

impl<T: PrettyPrint> PrettyPrint for &T {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        (*self).pretty_print(doc)
    }
}
