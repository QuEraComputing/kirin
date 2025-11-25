use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::data::Builder;
use super::field::FieldInfo;
use crate::{data::*, utils::is_type};

impl StatementFields<'_> for Builder {
    type FieldsType = Fields;
    type InfoType = ();
}

#[derive(Debug)]
pub struct Fields(pub Vec<FieldInfo>);

impl Fields {
    pub fn inputs(&self) -> Vec<TokenStream> {
        self.0
            .iter()
            .filter_map(|f| {
                if f.is_result || f.default.is_some() {
                    return None;
                }
                Some(f.input_signature())
            })
            .collect()
    }

    pub fn names(&self) -> Vec<syn::Ident> {
        self.0.iter().map(|f| f.name.clone()).collect()
    }

    pub fn result_names(&self) -> Vec<syn::Ident> {
        self.0
            .iter()
            .filter(|f| f.is_result)
            .map(|f| f.name.clone())
            .collect()
    }

    pub fn build_inputs(&self) -> Vec<TokenStream> {
        self.0
            .iter()
            .filter(|f| !f.is_result)
            .map(|f| f.build_input())
            .collect()
    }

    pub fn build_results(&self, statement_id: &syn::Ident) -> Vec<TokenStream> {
        self.0
            .iter()
            .filter(|f| f.is_result)
            .enumerate()
            .map(|(i, f)| f.build_result(statement_id, i))
            .collect()
    }

    pub fn initialization(&self, fields: &syn::Fields) -> TokenStream {
        let field_names = self.names();
        match fields {
            syn::Fields::Named(_) => {
                quote! {
                    {
                        #(#field_names,)*
                    }
                }
            }
            syn::Fields::Unnamed(_) => {
                quote! {
                    (
                        #(#field_names,)*
                    )
                }
            }
            syn::Fields::Unit => {
                quote! {}
            }
        }
    }

    pub fn ref_struct(&self, name: &syn::Ident) -> TokenStream {
        let field_names = self.result_names();
        let field_defs = field_names.iter().map(|name| {
            quote! { pub #name: ResultValue }
        });

        quote! {
            pub struct #name {
                pub id: StatementId,
                #(#field_defs,)*
            }
        }
    }
}

impl<'input> FromStructFields<'input, Builder> for Fields {
    fn from_struct_fields(
        _trait_info: &Builder,
        attrs: &StructAttribute,
        _parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self {
        Fields(from_fields(fields, |i| {
            attrs.get_field_attribute(i).cloned()
        }))
    }
}

impl<'input> FromVariantFields<'input, Builder> for Fields {
    fn from_variant_fields(
        _trait_info: &Builder,
        attrs: &VariantAttribute,
        _parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self {
        Fields(from_fields(fields, |i| {
            attrs.get_field_attribute(i).cloned()
        }))
    }
}

fn from_fields(fs: &syn::Fields, attr: impl Fn(usize) -> Option<FieldAttribute>) -> Vec<FieldInfo> {
    let n_results = count_results(fs);
    let mut result_count = 0;

    fs.iter()
        .enumerate()
        .map(|(i, f)| from_field(f, i, n_results, &mut result_count, &attr))
        .collect()
}

fn count_results(fs: &syn::Fields) -> usize {
    fs.iter().filter(|f| is_type(&f.ty, "ResultValue")).count()
}

fn from_field(
    f: &syn::Field,
    index: usize,
    n_results: usize,
    result_count: &mut usize,
    attr: impl FnOnce(usize) -> Option<FieldAttribute>,
) -> FieldInfo {
    let attr = attr(index);
    let default = if let Some(FieldAttribute {
        builder: Some(FieldBuilder { default, .. }),
        ..
    }) = &attr
    {
        default.clone()
    } else {
        None
    };

    let is_result = is_type(&f.ty, "ResultValue");
    let name = match &f.ident {
        Some(ident) => ident.clone(),
        None => default_field_name(n_results, index, is_result, *result_count),
    };

    if is_result {
        *result_count += 1;
    }
    FieldInfo {
        attr,
        name,
        ty: f.ty.clone(),
        is_result,
        default,
    }
}

fn default_field_name(
    total_results: usize,
    index: usize,
    is_result: bool,
    result_count: usize,
) -> syn::Ident {
    if is_result {
        if total_results == 1 {
            format_ident!("result")
        } else {
            format_ident!("result_{}", result_count)
        }
    } else {
        format_ident!("field_{}", index)
    }
}
