pub mod codegen;
pub mod derive;
pub mod emit;
pub mod ir;
pub mod misc;
pub mod scan;
pub mod stage;
pub mod test_util;
pub mod tokens;

pub mod prelude {
    pub use crate::codegen::{
        self, ConstructorBuilder, FieldBindings, GenericsBuilder, combine_where_clauses,
        deduplicate_types,
    };
    pub use crate::derive::{self, InputMeta, PathBuilder};
    pub use crate::emit::{self, Emit};
    pub use crate::ir::fields::{FieldCategory, FieldData, FieldInfo};
    pub use crate::ir::{self, Layout, StandardLayout};
    pub use crate::scan::{self, Scan};
    pub use crate::tokens::{
        self, FieldPatternTokens, IterEnumDefTokens, IterStructDefTokens, IteratorImplTokens,
        TraitImplTokens, TraitMethodImplTokens, VariantDefTokens, WrapperCallTokens,
        WrapperIterTypeTokens,
    };
    pub use darling;
    pub use proc_macro2;

    // Deprecated re-exports for backwards compatibility
    #[allow(deprecated)]
    pub use crate::derive::{InputBuilder, InputContext};
}
