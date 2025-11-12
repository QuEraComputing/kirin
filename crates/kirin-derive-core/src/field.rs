use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::{data::*, utils::*};

#[macro_export]
macro_rules! derive_accessor {
    ($input:expr, $method_name:expr, $matching_type:expr, $trait_path:expr) => {{
        let ctx = Context::new(
            AccessorTraitInfo::new(
                $method_name,
                syn::parse_quote! { $matching_type },
                syn::parse_quote! { $trait_path },
            ),
            $input,
        );
        let data = DataTrait::new(&ctx);
        ctx.generate_from(&data)
    }};
}

pub struct AccessorTraitInfo {
    method_name: syn::Ident,
    iter_name: syn::Ident,
    trait_path: syn::Path,
    matching_type_path: syn::Path,
    matching_type_name: syn::Ident,
    lifetime: syn::Lifetime,
    generics: syn::Generics,
}

impl AccessorTraitInfo {
    pub fn new(
        method_name: impl AsRef<str>,
        matching_type: syn::Path,
        trait_path: syn::Path,
    ) -> Self {
        let method_name_str = method_name.as_ref();
        let iter_name = format_ident!("{}Iter", to_camel_case(method_name_str), span = Span::call_site());
        let lifetime = syn::Lifetime::new("'a", Span::call_site());
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                lifetime.clone(),
            )));

        let matching_type_name = matching_type.segments.last().unwrap().ident.clone();

        Self {
            method_name: format_ident!("{}", method_name_str),
            iter_name,
            trait_path,
            matching_type_path: matching_type.clone(),
            matching_type_name,
            lifetime,
            generics,
        }
    }
}

impl<'input> TraitInfo<'input> for AccessorTraitInfo {
    type GlobalAttributeData = ();
    type MatchingFields = MatchingFields;
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
    fn trait_generics(&self) -> &syn::Generics {
        &self.generics
    }
    fn method_name(&self) -> &syn::Ident {
        &self.method_name
    }
}

pub enum MatchingFields {
    Named(NamedMatchingFields),
    Unnamed(UnnamedMatchingFields),
    Unit,
}

impl MatchingFields {
    fn from_fields<'input>(
        ctx: &Context<'input, AccessorTraitInfo>,
        fields: &'input syn::Fields,
    ) -> Self {
        match fields {
            syn::Fields::Named(named) => {
                MatchingFields::Named(NamedMatchingFields::new(&ctx.trait_info, &named))
            }
            syn::Fields::Unnamed(unnamed) => {
                MatchingFields::Unnamed(UnnamedMatchingFields::new(&ctx.trait_info, &unnamed))
            }
            syn::Fields::Unit => MatchingFields::Unit,
        }
    }
}

impl<'input> FromStructFields<'input, AccessorTraitInfo> for MatchingFields {
    fn from_struct_fields(
        ctx: &crate::data::Context<'input, AccessorTraitInfo>,
        _parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self {
        MatchingFields::from_fields(ctx, fields)
    }
}

impl<'input> FromVariantFields<'input, AccessorTraitInfo> for MatchingFields {
    fn from_variant_fields(
        ctx: &crate::data::Context<'input, AccessorTraitInfo>,
        _parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self {
        MatchingFields::from_fields(ctx, fields)
    }
}

pub struct NamedMatchingFields {
    lifetime: syn::Lifetime,
    matching_type_path: syn::Path,
    matching_fields: Vec<NamedMatchingField>,
}

impl NamedMatchingFields {
    fn new(info: &AccessorTraitInfo, fields: &syn::FieldsNamed) -> Self {
        Self {
            lifetime: info.lifetime.clone(),
            matching_type_path: info.matching_type_path.clone(),
            matching_fields: fields
                .named
                .iter()
                .filter_map(|f| NamedMatchingField::try_from_field(f, &info.matching_type_name))
                .collect(),
        }
    }

    fn vars(&self) -> Vec<syn::Ident> {
        self.matching_fields
            .iter()
            .map(|f| match f {
                NamedMatchingField::One(ident) => ident.clone(),
                NamedMatchingField::Vec(ident) => ident.clone(),
            })
            .collect()
    }

    fn iter(&self) -> TokenStream {
        self.matching_fields
            .iter()
            .map(|f| match f {
                NamedMatchingField::One(ident) => quote! { std::iter::once(#ident) },
                NamedMatchingField::Vec(ident) => quote! { #ident.iter() },
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote! { #acc.chain(#field) })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or(quote! { std::iter::empty() })
    }

    fn iter_type(&self) -> TokenStream {
        let lifetime = &self.lifetime;
        let matching_type_path = &self.matching_type_path;
        self.matching_fields
            .iter()
            .map(|f| match f {
                NamedMatchingField::One(_) => quote! { std::iter::Once<&'a #matching_type_path> },
                NamedMatchingField::Vec(_) => quote! { std::slice::Iter<'a, #matching_type_path> },
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote! { std::iter::Chain<#acc, #field> })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or(quote! { std::iter::Empty<&#lifetime #matching_type_path> })
    }
}

pub struct UnnamedMatchingFields {
    nfields: usize,
    lifetime: syn::Lifetime,
    matching_type_path: syn::Path,
    matching_fields: Vec<UnnamedMatchingField>,
}

impl UnnamedMatchingFields {
    fn new(info: &AccessorTraitInfo, fields: &syn::FieldsUnnamed) -> Self {
        Self {
            nfields: fields.unnamed.len(),
            lifetime: info.lifetime.clone(),
            matching_type_path: info.matching_type_path.clone(),
            matching_fields: fields
                .unnamed
                .iter()
                .enumerate()
                .filter_map(|(i, f)| {
                    UnnamedMatchingField::try_from_field(i, f, &info.matching_type_name)
                })
                .collect(),
        }
    }

    fn vars(&self) -> Vec<syn::Ident> {
        (0..self.nfields)
            .map(|i| format_ident!("field_{}", i))
            .collect()
    }

    fn iter(&self) -> TokenStream {
        self.matching_fields
            .iter()
            .map(|f| match f {
                UnnamedMatchingField::One(index) => {
                    let var = format_ident!("field_{}", index);
                    quote::quote! { std::iter::once(#var) }
                }
                UnnamedMatchingField::Vec(index) => {
                    let var = format_ident!("field_{}", index);
                    quote::quote! { #var.iter() }
                }
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote::quote! { #acc.chain(#field) })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or_else(|| quote::quote! { std::iter::empty() })
    }

    fn iter_type(&self) -> TokenStream {
        let lifetime = &self.lifetime;
        let matching_type_path = &self.matching_type_path;
        self.matching_fields
            .iter()
            .map(|f| match f {
                UnnamedMatchingField::One(_) => {
                    quote::quote! { std::iter::Once<&'a #matching_type_path> }
                }
                UnnamedMatchingField::Vec(_) => {
                    quote::quote! { std::slice::Iter<'a, #matching_type_path> }
                }
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote::quote! { std::iter::Chain<#acc, #field> })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or_else(|| quote::quote! { std::iter::Empty<&#lifetime #matching_type_path> })
    }
}

enum NamedMatchingField {
    One(syn::Ident),
    Vec(syn::Ident),
}

enum UnnamedMatchingField {
    One(usize),
    Vec(usize),
}

impl NamedMatchingField {
    fn try_from_field(f: &syn::Field, matching_type: &syn::Ident) -> Option<Self> {
        if is_type(&f.ty, matching_type) {
            Some(NamedMatchingField::One(f.ident.clone().unwrap()))
        } else if is_vec_type(&f.ty, matching_type) {
            Some(NamedMatchingField::Vec(f.ident.clone().unwrap()))
        } else if is_type_in_generic(&f.ty, matching_type) {
            panic!("generic types other than Vec are not supported");
        } else {
            None
        }
    }
}

impl UnnamedMatchingField {
    fn try_from_field(index: usize, f: &syn::Field, matching_type: &syn::Ident) -> Option<Self> {
        if is_type(&f.ty, matching_type) {
            Some(UnnamedMatchingField::One(index))
        } else if is_vec_type(&f.ty, matching_type) {
            Some(UnnamedMatchingField::Vec(index))
        } else if is_type_in_generic(&f.ty, matching_type) {
            panic!("generic types other than Vec are not supported");
        } else {
            None
        }
    }
}

impl GenerateFrom<'_, NamedWrapperStruct<'_, AccessorTraitInfo>> for AccessorTraitInfo {
    fn generate_from(&self, data: &NamedWrapperStruct<'_, AccessorTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let lifetime = &self.lifetime;
        let trait_path = &self.trait_path;
        let wraps = &data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                type Iter = <#wraps_type as #trait_path>::Iter;
                fn #method_name(&#lifetime self) -> Self::Iter {
                    let Self { #wraps, .. } = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps)
                }
            }
        }
    }
}

impl GenerateFrom<'_, UnnamedWrapperStruct<'_, AccessorTraitInfo>> for AccessorTraitInfo {
    fn generate_from(&self, data: &UnnamedWrapperStruct<'_, AccessorTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let lifetime = &self.lifetime;
        let trait_path = &self.trait_path;
        let wraps_index = data.wraps;
        let wraps_type = &data.wraps_type;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();
        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps_name = &vars[wraps_index];

        quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                type Iter = <#wraps_type as #trait_path>::Iter;
                fn #method_name(&#lifetime self) -> Self::Iter {
                    let Self (#(#vars,)* ..) = self;
                    <#wraps_type as #trait_path>::#method_name(#wraps_name)
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularStruct<'_, AccessorTraitInfo>> for AccessorTraitInfo {
    fn generate_from(&self, data: &RegularStruct<'_, AccessorTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let lifetime = &self.lifetime;
        let trait_path = &self.trait_path;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();

        match &data.fields {
            MatchingFields::Named(fields) => {
                let iter = fields.iter();
                let iter_type = fields.iter_type();
                let unpacking_vars = fields.vars();
                quote::quote! {
                    impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                        type Iter = #iter_type;
                        fn #method_name(&#lifetime self) -> Self::Iter {
                            let Self { #(#unpacking_vars,)* .. } = self;
                            #iter
                        }
                    }
                }
            }
            MatchingFields::Unnamed(fields) => {
                let iter = fields.iter();
                let iter_type = fields.iter_type();
                let unpacking_vars = fields.vars();
                quote::quote! {
                    impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                        type Iter = #iter_type;
                        fn #method_name(&#lifetime self) -> Self::Iter {
                            let Self ( #(#unpacking_vars,)* .. ) = self;
                            #iter
                        }
                    }
                }
            }
            MatchingFields::Unit => {
                quote::quote! {
                    impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                        type Iter = std::iter::Empty<&#lifetime ()>;
                        fn #method_name(&#lifetime self) -> Self::Iter {
                            std::iter::empty()
                        }
                    }
                }
            }
        }
    }
}

impl GenerateFrom<'_, RegularEnum<'_, AccessorTraitInfo>> for AccessorTraitInfo {
    fn generate_from(&self, data: &RegularEnum<'_, AccessorTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let lifetime = &self.lifetime;
        let trait_path = &self.trait_path;
        let iter_name = format_ident!("{}{}", name, data.ctx.trait_info.iter_name);
        let matching_type_path = &self.matching_type_path;
        let (impl_generics, trait_ty_generics, input_type_generics, where_clause) =
            data.ctx.split_for_impl();

        let iter_variants = data.variants.iter().map(|variant| variant.iter_variant());
        let method_arms = data
            .variants
            .iter()
            .map(|variant| variant.method_arm())
            .collect::<Vec<_>>();
        let iter_next_arms = data.variants.iter().map(|variant| variant.iter_next_arm());

        quote::quote! {
            impl #impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #where_clause {
                type Iter = #iter_name<#lifetime>;
                fn #method_name(&#lifetime self) -> Self::Iter {
                    match self {
                        #(#method_arms)*
                    }
                }
            }

            // note that if only regular, we have no type parameters to forward
            pub enum #iter_name<#lifetime> {
                #(#iter_variants),*
            }

            impl<#lifetime> Iterator for #iter_name<#lifetime> {
                type Item = &#lifetime #matching_type_path;
                fn next(&mut self) -> Option<Self::Item> {
                    match self {
                        #(#iter_next_arms)*
                    }
                }
            }
        }
    }
}

impl GenerateFrom<'_, WrapperEnum<'_, AccessorTraitInfo>> for AccessorTraitInfo {
    fn generate_from(&self, data: &WrapperEnum<'_, AccessorTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = &self.method_name;
        let lifetime = &self.lifetime;
        let trait_path = &self.trait_path;
        let iter_name = format_ident!("{}{}", name, data.ctx.trait_info.iter_name);
        let matching_type_path = &self.matching_type_path;

        let (trait_impl_generics, trait_ty_generics, input_type_generics, trait_where_clause) =
            data.ctx.split_for_impl();
        let iter_variants = data.variants.iter().map(|variant| variant.iter_variant());
        let method_arms = data.variants.iter().map(|variant| variant.method_arm());
        let iter_next_arms = data.variants.iter().map(|variant| variant.iter_next_arm());
        let iter_generics = data.ctx.generics.clone();
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        quote! {
            impl #trait_impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #trait_where_clause {
                type Iter = #iter_name<#lifetime>;
                fn #method_name(&#lifetime self) -> Self::Iter {
                    match self {
                        #(#method_arms)*
                    }
                }
            }

            pub enum #iter_name #iter_generics {
                #(#iter_variants),*
            }

            impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                type Item = &#lifetime #matching_type_path;
                fn next(&mut self) -> Option<Self::Item> {
                    match self {
                        #(#iter_next_arms)*
                    }
                }
            }
        }
    }
}

impl GenerateFrom<'_, EitherEnum<'_, AccessorTraitInfo>> for AccessorTraitInfo {
    fn generate_from(&self, data: &EitherEnum<'_, AccessorTraitInfo>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let method_name = data.ctx.trait_info.method_name();
        let trait_path = data.ctx.trait_info.trait_path();
        let iter_name = format_ident!("{}{}", name, data.ctx.trait_info.iter_name);
        let matching_type_path = &data.ctx.trait_info.matching_type_path;
        let lifetime = &data.ctx.trait_info.lifetime;

        let iter_variants = data.variants.iter().map(|variant| variant.iter_variant());
        let method_arms = data
            .variants
            .iter()
            .map(|variant| variant.method_arm())
            .collect::<Vec<_>>();
        let iter_next_arms = data.variants.iter().map(|variant| variant.iter_next_arm());

        let (trait_impl_generics, trait_ty_generics, input_type_generics, trait_where_clause) =
            data.ctx.split_for_impl();
        let iter_generics = data.ctx.generics.clone();
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        quote::quote! {
            impl #trait_impl_generics #trait_path #trait_ty_generics for #name #input_type_generics #trait_where_clause {
                type Iter = #iter_name<#lifetime, #matching_type_path>;
                fn #method_name(&#lifetime self) -> Self::Iter {
                    match self {
                        #(#method_arms)*
                    }
                }
            }

            pub enum #iter_name #iter_generics {
                #(#iter_variants),*
            }

            impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                type Item = &#lifetime #matching_type_path;
                fn next(&mut self) -> Option<Self::Item> {
                    match self {
                        #(#iter_next_arms)*
                    }
                }
            }
        }
    }
}

trait MethodMatchingArm {
    fn method_arm(&self) -> TokenStream;
}

impl MethodMatchingArm for RegularVariant<'_, AccessorTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        let name = &self.ctx.input.ident;
        let iter_name = format_ident!("{}{}", name, self.ctx.trait_info.iter_name);
        let variant_name = self.variant_name;

        match &self.matching_fields {
            MatchingFields::Named(fields) => {
                let vars = fields.vars();
                let iter = fields.iter();
                quote::quote! {
                    #name::#variant_name { #(#vars,)* .. } => {
                        #iter_name::#variant_name ( #iter )
                    }
                }
            },
            MatchingFields::Unnamed(fields) => {
                let vars = fields.vars();
                let iter = fields.iter();
                quote::quote! {
                    #name::#variant_name ( #(#vars,)* .. ) => {
                        #iter_name::#variant_name ( #iter )
                    }
                }
            },
            MatchingFields::Unit => {
                quote::quote! {
                    #name::#variant_name => {
                        #iter_name::#variant_name ( std::iter::empty() )
                    }
                }
            },
        }
    }
}

impl MethodMatchingArm for WrapperOrRegularVariant<'_, AccessorTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        match self {
            WrapperOrRegularVariant::Wrapper(wrapper) => wrapper.method_arm(),
            WrapperOrRegularVariant::Regular(regular) => regular.method_arm(),
        }
    }
}

impl MethodMatchingArm for WrapperVariant<'_, AccessorTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        match self {
            WrapperVariant::Named(named) => named.method_arm(),
            WrapperVariant::Unnamed(unnamed) => unnamed.method_arm(),
        }
    }
}

impl MethodMatchingArm for NamedWrapperVariant<'_, AccessorTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        let name = &self.ctx.input.ident;
        let method_name = &self.ctx.trait_info.method_name;
        let iter_name = format_ident!("{}{}", name, self.ctx.trait_info.iter_name);
        let trait_path = &self.ctx.trait_info.trait_path;
        let variant_name = &self.variant_name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;
        quote::quote! {
            #name::#variant_name { #wraps, .. } => {
                #iter_name::#variant_name ( <#wraps_type as #trait_path>::#method_name(#wraps) )
            },
        }
    }
}

impl MethodMatchingArm for UnnamedWrapperVariant<'_, AccessorTraitInfo> {
    fn method_arm(&self) -> TokenStream {
        let name = &self.ctx.input.ident;
        let method_name = &self.ctx.trait_info.method_name;
        let iter_name = format_ident!("{}{}", name, self.ctx.trait_info.iter_name);
        let trait_path = &self.ctx.trait_info.trait_path;
        let variant_name = &self.variant_name;
        let wraps_index = self.wraps;
        let wraps_type = &self.wraps_type;
        let vars = (0..=wraps_index)
            .map(|i| syn::Ident::new(&format!("field_{}", i), Span::call_site()))
            .collect::<Vec<_>>();
        let wraps_name = &vars[wraps_index];

        quote::quote! {
            #name::#variant_name (#(#vars,)* ..) => {
                #iter_name::#variant_name ( <#wraps_type as #trait_path>::#method_name(#wraps_name) )
            },
        }
    }
}

trait IterVariantDef {
    fn iter_variant(&self) -> TokenStream;
}

impl IterVariantDef for RegularVariant<'_, AccessorTraitInfo> {
    fn iter_variant(&self) -> TokenStream {
        let variant_name = self.variant_name;
        let iter_type = match &self.matching_fields {
            MatchingFields::Named(fields) => fields.iter_type(),
            MatchingFields::Unnamed(fields) => fields.iter_type(),
            MatchingFields::Unit => quote! { std::iter::Empty<()> },
        };
        quote::quote! {
            #variant_name (#iter_type)
        }
    }
}

impl IterVariantDef for WrapperOrRegularVariant<'_, AccessorTraitInfo> {
    fn iter_variant(&self) -> TokenStream {
        match self {
            WrapperOrRegularVariant::Wrapper(wrapper) => wrapper.iter_variant(),
            WrapperOrRegularVariant::Regular(regular) => regular.iter_variant(),
        }
    }
}

impl IterVariantDef for WrapperVariant<'_, AccessorTraitInfo> {
    fn iter_variant(&self) -> TokenStream {
        match self {
            WrapperVariant::Named(named) => named.iter_variant(),
            WrapperVariant::Unnamed(unnamed) => unnamed.iter_variant(),
        }
    }
}

impl IterVariantDef for NamedWrapperVariant<'_, AccessorTraitInfo> {
    fn iter_variant(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let wraps_type = &self.wraps_type;
        let trait_path = &self.ctx.trait_info.trait_path;
        let (_, trait_ty_generics, _, _) = self.ctx.split_for_impl();
        quote::quote! {
            #variant_name (<#wraps_type as #trait_path #trait_ty_generics>::Iter)
        }
    }
}

impl IterVariantDef for UnnamedWrapperVariant<'_, AccessorTraitInfo> {
    fn iter_variant(&self) -> TokenStream {
        let variant_name = &self.variant_name;
        let wraps_type = &self.wraps_type;
        let trait_path = &self.ctx.trait_info.trait_path;
        let (_, trait_ty_generics, _, _) = self.ctx.split_for_impl();
        quote::quote! {
            #variant_name (<#wraps_type as #trait_path #trait_ty_generics>::Iter)
        }
    }
}

trait IterNextArm {
    fn iter_next_arm(&self) -> TokenStream;
}

macro_rules! impl_iter_next_arm {
    ($variant:ident) => {
        impl IterNextArm for $variant<'_, AccessorTraitInfo> {
            fn iter_next_arm(&self) -> TokenStream {
                let name = &self.ctx.input.ident;
                let iter_name = format_ident!("{}{}", name, self.ctx.trait_info.iter_name);
                let variant_name = &self.variant_name;
                quote::quote! {
                    #iter_name::#variant_name ( iter ) => {
                        iter.next()
                    }
                }
            }
        }
    };
}

impl_iter_next_arm!(RegularVariant);
impl_iter_next_arm!(NamedWrapperVariant);
impl_iter_next_arm!(UnnamedWrapperVariant);

impl IterNextArm for WrapperVariant<'_, AccessorTraitInfo> {
    fn iter_next_arm(&self) -> TokenStream {
        match self {
            WrapperVariant::Named(named) => named.iter_next_arm(),
            WrapperVariant::Unnamed(unnamed) => unnamed.iter_next_arm(),
        }
    }
}

impl IterNextArm for WrapperOrRegularVariant<'_, AccessorTraitInfo> {
    fn iter_next_arm(&self) -> TokenStream {
        match self {
            WrapperOrRegularVariant::Wrapper(wrapper) => wrapper.iter_next_arm(),
            WrapperOrRegularVariant::Regular(regular) => regular.iter_next_arm(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_either_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { #[kirin(wraps)] wrapped: InnerStructA<T> },
                #[kirin(wraps)]
                VariantB(InnerStructB),
                VariantC { a: SSAValue, b: T, c: SSAValue },
                VariantD(SSAValue, f64, SSAValue),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_global_enum_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(wraps)]
            enum TestEnum<T> {
                VariantA { wrapped: InnerStructA<T> },
                VariantB(InnerStructB),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_regular_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { a: SSAValue, b: T, c: SSAValue },
                VariantB(SSAValue, f64, SSAValue),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_arith_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            pub enum ArithInstruction<T> {
                Add(SSAValue, Vec<SSAValue>, ResultValue, T),
                Sub(SSAValue, Vec<SSAValue>, ResultValue, T),
                Mul(SSAValue, Vec<SSAValue>, ResultValue),
                Div(SSAValue, Vec<SSAValue>, ResultValue),
            }
        };
        // insta::assert_snapshot!(generate(input.clone()));
        insta::assert_snapshot!(rustfmt(derive_accessor!(
            &input,
            "regions",
            kirin_ir::Region,
            kirin_ir::HasRegions
        )))
    }

    #[test]
    fn test_regular_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: f64,
                c: T,
            }
        };
        insta::assert_snapshot!(generate(input));

        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: SSAValue,
                c: T,
            }
        };
        insta::assert_snapshot!(generate(input));

        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: SSAValue,
                c: Vec<SSAValue>,
                d: T,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_named() {
        let input: syn::DeriveInput = syn::parse_quote! {
            pub enum ControlFlowInstruction {
                Branch {
                    target: Block,
                },
                ConditionalBranch {
                    condition: SSAValue,
                    true_target: Block,
                    false_target: Block,
                },
                Return(SSAValue),
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_named_struct_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                wrapped: InnerStruct<T>,
            }
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_unnamed_struct_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T>(SSAValue, T, SSAValue, String, f64);
        };
        insta::assert_snapshot!(generate(input));
    }

    #[test]
    fn test_unnamed_struct_regular() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct(SSAValue, SSAValue, SSAValue);
        };
        insta::assert_snapshot!(generate(input));
    }

    fn generate(input: syn::DeriveInput) -> String {
        rustfmt(derive_accessor!(
            &input,
            "arguments",
            kirin_ir::SSAValue,
            kirin_ir::HasArguments
        ))
    }
}
