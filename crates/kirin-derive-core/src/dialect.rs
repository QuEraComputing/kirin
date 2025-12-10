use quote::quote;

use crate::data::*;

#[macro_export]
macro_rules! derive_dialect {
    ($input:expr) => {{
        let trait_info = DialectInfo::new();
        let data = Data::builder()
            .trait_info(&trait_info)
            .input($input)
            .build();
        trait_info.generate_from(&data)
    }};
}

pub struct DialectInfo {
    generics: syn::Generics,
}

impl DialectInfo {
    pub fn new() -> Self {
        Self {
            generics: syn::Generics::default(),
        }
    }
}

impl HasDefaultCratePath for DialectInfo {
    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::kirin::ir }
    }
}

impl HasGenerics for DialectInfo {
    fn generics(&self) -> &syn::Generics {
        &self.generics
    }
}

impl StatementFields<'_> for DialectInfo {
    type FieldsType = ();
    type InfoType = ();
}

impl GenerateFrom<'_, Data<'_, DialectInfo>> for DialectInfo {
    fn generate_from(&self, data: &Data<'_, DialectInfo>) -> proc_macro2::TokenStream {
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);
        let name = &data.input().ident;
        let trait_path = data.absolute_path(self, &syn::parse_quote! { Dialect });
        if let Some(ty) = data.type_lattice() {
            quote! {
                impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {
                    type TypeLattice = #ty;
                }
            }
        } else {
            syn::Error::new_spanned(
                &data.input().ident,
                "Dialect must specify a type lattice using the `#[kirin(type_lattice = ...)]` attribute",
            )
            .to_compile_error()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_dialect() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = SimpleTypeLattice)]
            enum TestDialect {
                Add(SSAValue, SSAValue),
                Const(Value),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_wrapper_enum_generic() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(fn, type_lattice = T, wraps)]
            pub enum StructuredControlFlow<T: TypeLattice> {
                If(If<T>),
                For(For<T>),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        rustfmt(derive_dialect!(&input))
    }
}
