use bon::Builder;
use quote::quote;

use crate::prelude::*;

use super::attrs::{KirinEnumOptions, KirinFieldOptions, KirinStructOptions, KirinVariantOptions};

/// derive a marker trait, a marker trait
/// is a trait without any methods or associated items,
/// used to mark types for special behavior or categorization.
#[derive(Builder)]
pub struct DialectMarker {
    #[builder(with = |s: impl Into<String>| from_str(s))]
    trait_path: syn::Path,
    #[builder(default = syn::parse_quote!(::kirin::ir), with = |s: impl Into<String>| from_str(s))]
    crate_path: syn::Path,
}

impl Layout for DialectMarker {
    type EnumAttr = KirinEnumOptions;
    type StructAttr = KirinStructOptions;
    type VariantAttr = KirinVariantOptions;
    type FieldAttr = KirinFieldOptions;
    type FieldExtra = ();
    type StatementExtra = ();
}

impl DeriveTrait for DialectMarker {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl DeriveWithCratePath for DialectMarker {
    fn default_crate_path(&self) -> &syn::Path {
        &self.crate_path
    }
}

macro_rules! impl_compile {
    ($name:ident) => {
        impl<'src> Compile<'src, $name<'src, DialectMarker>, TokenStream> for DialectMarker {
            fn compile(&self, node: &$name<'src, DialectMarker>) -> TokenStream {
                let name = &node.input().ident;
                let (impl_generics, ty_generics, where_clause) =
                    node.input().generics.split_for_impl();
                let trait_path: TraitPath = self.compile(node);
                let type_lattice = &node.attrs().type_lattice;
                quote! {
                    #[automatically_derived]
                    impl #impl_generics #trait_path for #name #ty_generics #where_clause {
                        type TypeLattice = #type_lattice;
                    }
                }
            }
        }
    };
}

impl_compile!(Enum);
impl_compile!(Struct);

impl<'src> Emit<'src> for DialectMarker {
    type EnumImpl = TokenStream;
    type StructImpl = TokenStream;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marker_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = T, crate = ::my::path)]
            pub enum StructuredControlFlow {
                If {
                    condition: SSAValue,
                    then_block: Block,
                    else_block: Block,
                },
                Loop {
                    body_block: Block,
                    exit_block: Block,
                },
            }
        };
        insta::assert_snapshot!(
            DialectMarker::builder()
                .trait_path("MarkerTrait")
                .build()
                .print(&input)
        );
    }
}
