/// representation of a statement, could either be a struct or enum variant
mod core;
mod dialect_impl;
mod enum_impl;
mod field;
mod gadgets;
mod struct_impl;
mod traits;

#[cfg(feature = "debug")]
mod debug;

pub use core::Statement;
pub use dialect_impl::Dialect;
pub use enum_impl::DialectEnum;
pub use field::{Field, FieldMember, Fields, Unpacking};
pub use gadgets::*;
pub use struct_impl::DialectStruct;
pub use traits::*;

/// Creates a wrapper over TokenStream to mark
/// the compile target semantically for composability.
/// See the default `kirin` module for usage examples.
#[macro_export]
macro_rules! target {
    {$(#[$attr:meta])* $v:vis struct $name:ident} => {
        #[derive(Clone)]
        $(#[$attr])*
        $v struct $name(proc_macro2::TokenStream);

        impl std::ops::Deref for $name {
            type Target = proc_macro2::TokenStream;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl quote::ToTokens for $name {
            fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl From<$name> for proc_macro2::TokenStream {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<proc_macro2::TokenStream> for $name {
            fn from(value: proc_macro2::TokenStream) -> Self {
                $name(value)
            }
        }
    };
}
