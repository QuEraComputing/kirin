//! Core traits for pretty printing.

use std::io::{Write, stdout};

use crate::{ArenaDoc, Config, Document, RenderError};
use kirin_ir::{Dialect, GlobalSymbol, InternTable, StageInfo};
use prettyless::DocAllocator;

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
    /// Defaults to [`PrettyPrint::pretty_print`]. Override this when the type
    /// appears in `{field:name}` format positions in a `#[chumsky(format = "...")]`
    /// attribute. The derive macro generates overrides automatically; manual impls
    /// must override this if the type is used in name format positions.
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
    /// Defaults to [`PrettyPrint::pretty_print`]. Override this when the type
    /// appears in `{field:type}` format positions in a `#[chumsky(format = "...")]`
    /// attribute. The derive macro generates overrides automatically; manual impls
    /// must override this if the type is used in type format positions.
    fn pretty_print_type<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        self.pretty_print(doc)
    }

    /// Whether this type's `pretty_print` output includes result names (`%name =`).
    ///
    /// Returns `false` because result names are always printed by the
    /// statement-level printer. The dialect's `pretty_print` only handles
    /// the dialect-specific body.
    fn prints_result_names(&self) -> bool {
        false
    }

    /// Whether this type's `pretty_print` output includes the function header
    /// (`fn @name`, ports, yields, etc.).
    ///
    /// When `true`, the framework's `print_specialized_function` skips the
    /// `fn @name(types) -> type` portion and only prints `specialize @stage `.
    /// The dialect's PrettyPrint handles the rest.
    ///
    /// Returns `false` by default (framework prints the full header).
    /// The derive macro sets this to `true` when body projections are used.
    fn prints_function_header(&self) -> bool {
        false
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
///     .into_string();
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

    /// Render to a string, consuming the builder.
    pub fn into_string(self) -> Result<String, RenderError> {
        let node = self.node;
        let mut doc = self.into_document();
        Ok(doc.render(node)?)
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
///     .into_string();
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
        self.render(stage)
            .into_string()
            .unwrap_or_else(|e| panic!("render failed: {e}"))
    }
}

/// Marker trait for types whose `PrettyPrint` implementation is just
/// `doc.text(self.to_string())`. Implement this (empty) trait on your type
/// to get a blanket `PrettyPrint` impl.
///
/// # Requirements
/// - The type must implement `Display`.
/// - The type must NOT have a manual `PrettyPrint` impl (would conflict).
///
/// # Example
/// ```ignore
/// impl PrettyPrintViaDisplay for MyType {}
/// // Now MyType: PrettyPrint, rendering via Display::fmt
/// ```
pub trait PrettyPrintViaDisplay: std::fmt::Display {}

impl<T: PrettyPrintViaDisplay> PrettyPrint for T {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}
