mod attrs;
mod definition;
mod fields;
mod generics;
mod scan;
mod source;
mod to_tokens;
mod wrapper;

pub use attrs::Attrs;
pub use definition::*;
pub use fields::HasFields;
pub use generics::WithGenerics;
pub use proc_macro2::TokenStream;
pub use quote::ToTokens;
pub use scan::{ScanExtra, ScanInto};
pub use source::{Source, SourceIdent, WithInput};
pub use wrapper::{AnyWrapper, Wrapper};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_simple() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct MyStruct {
                a: u32,
                b: String,
            }
        };

        let node = EmptyLayoutImpl.scan(&input).unwrap();
        insta::assert_debug_snapshot!(node);
    }

    #[test]
    fn test_struct_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            struct MyStruct {
                value: u32,
            }
        };

        let node = EmptyLayoutImpl.scan(&input).unwrap();
        insta::assert_debug_snapshot!(node);
    }

    #[test]
    #[should_panic]
    fn test_struct_wraps_panic() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            struct MyStruct {
                a: u32,
                b: String,
            }
        };

        let node = EmptyLayoutImpl.scan(&input).unwrap();
        insta::assert_debug_snapshot!(node);
    }

    #[test]
    fn test_enum_simple() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum MyEnum {
                A(u32),
                B { x: String },
            }
        };
        let node = EmptyLayoutImpl.scan(&input).unwrap();
        insta::assert_debug_snapshot!(node);
    }

    #[test]
    fn test_enum_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum MyEnum {
                #[wraps]
                A(u32),
                B { x: String },
            }
        };
        let node = EmptyLayoutImpl.scan(&input).unwrap();
        insta::assert_debug_snapshot!(node);
    }
}
