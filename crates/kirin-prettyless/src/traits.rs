//! Core traits for pretty printing.

use std::io::{Write, stdout};

use kirin_ir::{Dialect, GlobalSymbol, InternTable, StageInfo};
use prettyless::DocBuilder;

use crate::{ArenaDoc, Config, Document, ScanResultWidth};

/// Core trait for pretty printing values to a document.
///
/// This trait defines how a type should be rendered to a document representation.
/// The method is generic over the dialect type `L`, allowing the same implementation
/// to work with any `Document<L>`.
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

/// Extension trait providing convenience methods for pretty printing IR nodes.
///
/// This trait is automatically implemented for any type that implements both
/// `PrettyPrint` and `ScanResultWidth<L>`. All methods require a `&StageInfo<L>`
/// parameter since IR nodes (like `Statement`, `Block`, `Region`, etc.) need to
/// look up their data from the context.
///
/// # Example
///
/// ```ignore
/// use kirin_prettyless::{Config, PrettyPrintExt};
///
/// // Render to string with custom config
/// let output = statement.sprint_with_config(config, &context);
///
/// // Render to string with default config
/// let output = statement.sprint(&context);
/// ```
pub trait PrettyPrintExt<L: Dialect + PrettyPrint>: PrettyPrint + ScanResultWidth<L>
where
    L::Type: std::fmt::Display,
{
    /// Render to string with custom config.
    fn sprint_with_config(&self, config: Config, stage: &StageInfo<L>) -> String;

    /// Render to string with default config.
    fn sprint(&self, stage: &StageInfo<L>) -> String;

    /// Write to writer with custom config.
    fn write_with_config(&self, writer: &mut impl Write, config: Config, stage: &StageInfo<L>);

    /// Write to writer with default config.
    fn write(&self, writer: &mut impl Write, stage: &StageInfo<L>);

    /// Print to stdout with custom config.
    fn print_with_config(&self, config: Config, stage: &StageInfo<L>);

    /// Print to stdout with default config.
    fn print(&self, stage: &StageInfo<L>);

    /// Render to string with default config and global symbol resolution.
    fn sprint_with_globals(
        &self,
        stage: &StageInfo<L>,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) -> String;

    /// Render to string with custom config and global symbol resolution.
    fn sprint_with_globals_config(
        &self,
        config: Config,
        stage: &StageInfo<L>,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) -> String;

    /// Print to stdout with default config and global symbol resolution.
    fn print_with_globals(
        &self,
        stage: &StageInfo<L>,
        global_symbols: &InternTable<String, GlobalSymbol>,
    );

    /// Display with bat pager with custom config.
    #[cfg(feature = "bat")]
    fn bat_with_config(&self, config: Config, stage: &StageInfo<L>);

    /// Display with bat pager with default config.
    #[cfg(feature = "bat")]
    fn bat(&self, stage: &StageInfo<L>);
}

// Blanket implementation: any type that implements PrettyPrint + ScanResultWidth<L>
// automatically gets the context-aware convenience methods.
impl<L: Dialect + PrettyPrint, T: PrettyPrint + ScanResultWidth<L>> PrettyPrintExt<L> for T
where
    L::Type: std::fmt::Display,
{
    fn sprint_with_config(&self, config: Config, stage: &StageInfo<L>) -> String {
        let mut doc = Document::new(config, stage);
        doc.render(self).expect("render failed")
    }

    fn sprint(&self, stage: &StageInfo<L>) -> String {
        let mut doc = Document::new(Config::default(), stage);
        doc.render(self).expect("render failed")
    }

    fn write_with_config(&self, writer: &mut impl Write, config: Config, stage: &StageInfo<L>) {
        let mut doc = Document::new(config, stage);
        let output = doc.render(self).expect("render failed");
        writer.write_all(output.as_bytes()).expect("write failed");
    }

    fn write(&self, writer: &mut impl Write, stage: &StageInfo<L>) {
        let mut doc = Document::new(Config::default(), stage);
        let output = doc.render(self).expect("render failed");
        writer.write_all(output.as_bytes()).expect("write failed");
    }

    fn print_with_config(&self, config: Config, stage: &StageInfo<L>) {
        let mut doc = Document::new(config, stage);
        let output = doc.render(self).expect("render failed");
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    fn print(&self, stage: &StageInfo<L>) {
        let mut doc = Document::new(Config::default(), stage);
        let output = doc.render(self).expect("render failed");
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    fn sprint_with_globals(
        &self,
        stage: &StageInfo<L>,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) -> String {
        let mut doc = Document::with_global_symbols(Config::default(), stage, global_symbols);
        doc.render(self).expect("render failed")
    }

    fn sprint_with_globals_config(
        &self,
        config: Config,
        stage: &StageInfo<L>,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) -> String {
        let mut doc = Document::with_global_symbols(config, stage, global_symbols);
        doc.render(self).expect("render failed")
    }

    fn print_with_globals(
        &self,
        stage: &StageInfo<L>,
        global_symbols: &InternTable<String, GlobalSymbol>,
    ) {
        let mut doc = Document::with_global_symbols(Config::default(), stage, global_symbols);
        let output = doc.render(self).expect("render failed");
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    #[cfg(feature = "bat")]
    fn bat_with_config(&self, config: Config, stage: &StageInfo<L>) {
        let mut doc = Document::new(config, stage);
        doc.pager(self).expect("pager failed");
    }

    #[cfg(feature = "bat")]
    fn bat(&self, stage: &StageInfo<L>) {
        let mut doc = Document::new(Config::default(), stage);
        doc.pager(self).expect("pager failed");
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
