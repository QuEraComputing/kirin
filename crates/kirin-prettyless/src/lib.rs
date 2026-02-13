//! Pretty printing support for Kirin IR.
//!
//! This crate provides a pretty printing framework for Kirin IR, built on top
//! of the `prettyless` library. It offers:
//!
//! - [`Config`] - Configuration options for formatting output
//! - [`Document`] - A document builder for constructing pretty-printed output
//! - [`PrettyPrint`] - Core trait for defining how types render to documents
//! - [`PrettyPrintExt`] - Extension trait with convenience methods
//!
//! # Example
//!
//! ```ignore
//! use kirin_prettyless::{Config, PrettyPrintExt};
//!
//! // Render an IR node to string with default config
//! let output = statement.sprint(&context);
//!
//! // Render with custom config
//! let config = Config::default().with_width(80);
//! let output = statement.sprint_with_config(config, &context);
//! ```

mod config;
mod document;
mod impls;
mod pipeline;
mod scan;
mod traits;

// Re-export main types
pub use config::Config;
pub use document::Document;
pub use pipeline::{PipelineDocument, PipelinePrintExt, PrintExt, RenderStage};

#[cfg(feature = "derive")]
pub use kirin_prettyless_derive::RenderStage;
pub use scan::ScanResultWidth;
pub use traits::{PrettyPrint, PrettyPrintExt, PrettyPrintName, PrettyPrintType};

// Re-export from prettyless for convenience
pub use prettyless::{Arena, DocAllocator};
pub type ArenaDoc<'a> = prettyless::DocBuilder<'a, Arena<'a>>;

/// Prelude module for common imports.
pub mod prelude {
    pub use crate::{
        ArenaDoc, Config, DocAllocator, Document, PipelineDocument, PipelinePrintExt, PrettyPrint,
        PrettyPrintExt, PrettyPrintName, PrettyPrintType, PrintExt, RenderStage,
    };
    pub use prettyless;
}

#[cfg(feature = "bat")]
mod bat;

#[cfg(test)]
mod tests;
