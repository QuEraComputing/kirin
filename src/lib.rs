pub mod dialects;
pub use kirin_ir as ir;

#[cfg(feature = "parser")]
pub use kirin_chumsky as parsers;

#[cfg(feature = "pretty")]
pub use kirin_prettyless as pretty;

#[cfg(feature = "interpret")]
pub use kirin_interpreter as interpreter;

pub mod prelude {
    #[cfg(feature = "parser")]
    pub use kirin_chumsky::prelude::*;
    pub use kirin_ir::*;
    #[cfg(feature = "pretty")]
    pub use kirin_prettyless::prelude::{
        ArenaDoc, Config, DocAllocator, Document, FunctionRenderBuilder, PipelineDocument,
        PipelinePrintExt, PipelineRenderBuilder, PrettyPrint, PrettyPrintExt,
        PrettyPrintViaDisplay, PrintExt, RenderDispatch, RenderError,
    };
}
