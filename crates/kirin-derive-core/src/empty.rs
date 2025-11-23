use quote::quote;

use crate::data::*;

#[macro_export]
macro_rules! derive_empty {
    ($input:expr, $trait_path:expr, $path_crate:expr) => {{
        let path_crate = syn::parse_quote! {$path_crate};
        let trait_path = syn::parse_quote! {$trait_path};
        let trait_info = Empty::new(path_crate, trait_path);
        let data = Data::builder()
            .trait_info(&trait_info)
            .input($input)
            .build();
        trait_info.generate_from(&data)
    }};
}

pub struct Empty {
    pub crate_path: syn::Path,
    pub trait_path: syn::Path,
    pub generics: syn::Generics,
}

impl Empty {
    pub fn new(
        crate_path: syn::Path,
        trait_path: syn::Path,
    ) -> Self {
        Self {
            crate_path,
            trait_path,
            generics: syn::Generics::default(),
        }
    }
}

impl HasDefaultCratePath for Empty {
    fn default_crate_path(&self) -> syn::Path {
        self.crate_path.clone()       
    }
}

impl HasTraitGenerics for Empty {
    fn trait_generics(&self) -> &syn::Generics {
        &self.generics       
    }
}

impl StatementFields<'_> for Empty {
    type FieldsType = ();
    type InfoType = ();
}

impl GenerateFrom<'_, Data<'_, Empty>> for Empty {
    fn generate_from(&self, data: &Data<'_, Empty>) -> proc_macro2::TokenStream {
        let SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause,
        } = data.split_for_impl(self);
        let name = &data.input().ident;
        let trait_path = data.absolute_path(self, &self.trait_path);
        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_ty_generics #where_clause {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_empty() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct MyStruct {
                field1: i32,
                field2: String,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        rustfmt(derive_empty!(&input, EmptyTrait, ::my_crate))
    }
}
