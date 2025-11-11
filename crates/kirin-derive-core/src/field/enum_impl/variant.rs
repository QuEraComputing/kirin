use proc_macro2::TokenStream;
use quote::format_ident;

use crate::field::data::{AccessorInfo, NamedMatchedFields, UnnamedMatchedFields, has_attr};

pub struct NamedVariantRegularAccessor<'input> {
    pub info: &'input AccessorInfo,
    pub name: &'input syn::Ident,
    pub variant_name: &'input syn::Ident,
    pub matching_fields: NamedMatchedFields,
}

impl<'input> NamedVariantRegularAccessor<'input> {
    pub fn scan(
        info: &'input  AccessorInfo,
        input: &'input syn::DeriveInput,
        variant_name: &'input syn::Ident,
        fields: &'input syn::FieldsNamed,
    ) -> Self {
        let matching_fields = NamedMatchedFields::new(&info, fields);

        Self {
            info,
            name: &input.ident,
            variant_name,
            matching_fields,
        }
    }

    pub fn vars(&self) -> Vec<syn::Ident> {
        self.matching_fields.vars()
    }

    pub fn iter(&self) -> TokenStream {
        self.matching_fields.iter()
    }

    pub fn iter_type(&self) -> TokenStream {
        self.matching_fields.iter_type()
    }

    pub fn iter_variant(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let iter_type = self.iter_type();
        quote::quote! {
            #variant_name (#iter_type)
        }
    }

    pub fn method_arm(&self) -> TokenStream {
        let name = &self.name;
        let iter_name = &self.info.iter_name;
        let variant_name = &self.variant_name;
        let vars = self.vars();
        let iter = self.iter();
        quote::quote! {
            #name::#variant_name ( #(#vars),* ) => {
                #iter_name::#variant_name ( #iter )
            }
        }
    }

    pub fn iter_next_arm(&self) -> TokenStream {
        let iter_name = &self.info.iter_name;
        let variant_name = &self.variant_name;
        quote::quote! {
            #iter_name::#variant_name ( iter ) => {
                iter.next()
            }
        }
    }
}

pub struct NamedVariantWrapperAccessor<'input> {
    pub info: &'input AccessorInfo,
    pub name: &'input syn::Ident,
    pub variant_name: &'input syn::Ident,
    pub wraps: syn::Ident,
    pub wraps_type: syn::Type,
}

impl<'input> NamedVariantWrapperAccessor<'input> {
    pub fn scan(
        info: &'input AccessorInfo,
        input: &'input syn::DeriveInput,
        variant_name: &'input syn::Ident,
        fields_named: &'input syn::FieldsNamed,
    ) -> Self {
        if fields_named.named.len() == 1 {
            let f = fields_named.named.first().unwrap();
            Self {
                info,
                name: &input.ident,
                variant_name,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            }
        } else if let Some(f) = fields_named
            .named
            .iter()
            .find(|f| has_attr(&f.attrs, "kirin", "wraps"))
        {
            Self {
                info,
                name: &input.ident,
                variant_name,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            }
        } else {
            panic!(
                "global wrapper variants must have exactly one field or specify which field wraps another instruction using #[kirin(wraps)]"
            );
        }
    }

    pub fn iter_type(&self) -> TokenStream {
        let wraps_type = &self.wraps_type;
        let trait_path = &self.info.trait_path;
        quote::quote! {
            <#wraps_type as #trait_path<'a>>::Iter
        }
    }

    pub fn iter_variant(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let iter_type = self.iter_type();
        quote::quote! {
            #variant_name (#iter_type)
        }
    }

    pub fn method_arm(&self) -> TokenStream {
        let name = &self.name;
        let method_name = &self.info.name;
        let iter_name = &self.info.iter_name;
        let trait_path = &self.info.trait_path;
        let variant_name = &self.variant_name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;
        quote::quote! {
            #name::#variant_name { #wraps, .. } => {
                #iter_name::#variant_name ( <#wraps_type as #trait_path>::#method_name(#wraps) )
            },
        }
    }

    pub fn iter_next_arm(&self) -> TokenStream {
        let iter_name = &self.info.iter_name;
        let variant_name = &self.variant_name;
        quote::quote! {
            #iter_name::#variant_name ( iter ) => {
                iter.next()
            }
        }
    }
}

pub struct UnnamedVariantRegularAccessor<'input> {
    pub info: &'input AccessorInfo,
    pub name: &'input syn::Ident,
    pub variant_name: &'input syn::Ident,
    pub matching_fields: UnnamedMatchedFields,
}

impl<'input> UnnamedVariantRegularAccessor<'input> {
    pub fn scan(
        info: &'input AccessorInfo,
        input: &'input syn::DeriveInput,
        variant_name: &'input syn::Ident,
        fields: &'input syn::FieldsUnnamed,
    ) -> Self {
        let matching_fields = UnnamedMatchedFields::new(&info, fields);

        Self {
            info,
            name: &input.ident,
            variant_name,
            matching_fields,
        }
    }

    pub fn vars(&self) -> Vec<syn::Ident> {
        self.matching_fields.vars()
    }

    pub fn iter(&self) -> TokenStream {
        self.matching_fields.iter()
    }

    pub fn iter_type(&self) -> TokenStream {
        self.matching_fields.iter_type()
    }

    pub fn iter_variant(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let iter_type = self.iter_type();
        quote::quote! {
            #variant_name (#iter_type)
        }
    }

    pub fn method_arm(&self) -> TokenStream {
        let name = &self.name;
        let iter_name = &self.info.iter_name;
        let variant_name = &self.variant_name;
        let vars = self.vars();
        let iter = self.iter();
        quote::quote! {
            #name::#variant_name ( #(#vars),* ) => {
                #iter_name::#variant_name ( #iter )
            }
        }
    }

    pub fn iter_next_arm(&self) -> TokenStream {
        let iter_name = &self.info.iter_name;
        let variant_name = &self.variant_name;
        quote::quote! {
            #iter_name::#variant_name ( iter ) => {
                iter.next()
            }
        }
    }
}

pub struct UnnamedVariantWrapperAccessor<'input> {
    pub info: &'input AccessorInfo,
    pub name: &'input syn::Ident,
    pub variant_name: &'input syn::Ident,
    pub nfields: usize,
    pub wraps: usize,
    pub wraps_type: syn::Type,
}

impl<'input> UnnamedVariantWrapperAccessor<'input> {
    pub fn scan(
        info: &'input AccessorInfo,
        input: &'input syn::DeriveInput,
        variant_name: &'input syn::Ident,
        fields_unnamed: &'input syn::FieldsUnnamed,
    ) -> Self {
        if fields_unnamed.unnamed.len() == 1 {
            Self {
                info,
                name: &input.ident,
                variant_name,
                nfields: 1,
                wraps: 0,
                wraps_type: fields_unnamed.unnamed.first().unwrap().ty.clone(),
            }
        } else if let Some((index, f)) = fields_unnamed
            .unnamed
            .iter()
            .enumerate()
            .find(|(_, f)| has_attr(&f.attrs, "kirin", "wraps"))
        {
            Self {
                info,
                name: &input.ident,
                variant_name,
                nfields: fields_unnamed.unnamed.len(),
                wraps: index,
                wraps_type: f.ty.clone(),
            }
        } else {
            panic!(
                "global wrapper variants must have exactly one field or specify which field wraps another instruction using #[kirin(wraps)]"
            );
        }
    }

    pub fn iter_type(&self) -> TokenStream {
        let wraps_type = &self.wraps_type;
        let trait_path = &self.info.trait_path;
        quote::quote! {
            <#wraps_type as #trait_path<'a>>::Iter
        }
    }

    pub fn iter_variant(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let iter_type = self.iter_type();
        quote::quote! {
            #variant_name (#iter_type)
        }
    }

    pub fn method_arm(&self) -> TokenStream {
        let name = &self.name;
        let method_name = &self.info.name;
        let iter_name = &self.info.iter_name;
        let trait_path = &self.info.trait_path;
        let variant_name = &self.variant_name;
        let vars = (0..=self.nfields)
            .map(|i| format_ident!("field_{}", i))
            .collect::<Vec<_>>();
        let wraps_name = &vars[self.wraps];
        let wraps_type = &self.wraps_type;
        quote::quote! {
            #name::#variant_name (#(#vars),*) => {
                #iter_name::#variant_name ( <#wraps_type as #trait_path>::#method_name(#wraps_name) )
            },
        }
    }

    pub fn iter_next_arm(&self) -> TokenStream {
        let iter_name = &self.info.iter_name;
        let variant_name = &self.variant_name;
        quote::quote! {
            #iter_name::#variant_name ( iter ) => {
                iter.next()
            }
        }
    }
}
