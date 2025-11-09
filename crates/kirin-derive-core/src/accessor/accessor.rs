use std::cell::OnceCell;

use eyre::eyre;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

use crate::{
    derive::DeriveContext,
    traits::{Attribute, Generate, Scan},
};

use super::iterator::IteratorImpl;

pub struct FieldAccessor<F> {
    f: F,
    accessor: syn::Ident,
    iter: syn::Ident,
    matching_type: syn::Ident,
    lifetime: syn::Lifetime,
    generics: OnceCell<syn::Generics>,
    data: OnceCell<DataInfo>,
}

impl<F: Fn(&syn::Type) -> bool> FieldAccessor<F> {
    pub fn new(
        f: F,
        iter: impl AsRef<str>,
        accessor: impl AsRef<str>,
        ty: impl AsRef<str>,
    ) -> Self {
        Self {
            f,
            iter: format_ident!("__Kirin{}Iter", iter.as_ref()),
            accessor: format_ident!("{}", accessor.as_ref()),
            matching_type: format_ident!("{}", ty.as_ref()),
            lifetime: syn::Lifetime::new("'__kirin_ir_iter_a", proc_macro2::Span::call_site()),
            generics: OnceCell::new(),
            data: OnceCell::new(),
        }
    }
}

pub enum DataInfo {
    Struct(IteratorImpl),
    Enum(Vec<IteratorImpl>),
}

impl<F, A: Attribute> Scan<A> for FieldAccessor<F>
where
    F: Fn(&syn::Type) -> bool,
{
    fn scan(&mut self, ctx: &DeriveContext<A>) -> eyre::Result<()> {
        let mut generics = ctx.input.generics.clone();
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(self.lifetime.clone())),
        );
        self.generics
            .set(generics)
            .map_err(|_| eyre!("duplicated scan"))?;

        match &ctx.input.data {
            syn::Data::Struct(data) => {
                self.data
                    .set(DataInfo::Struct(IteratorImpl::from_struct(
                        ctx.attributes.global_wraps(),
                        &ctx.input.ident,
                        data,
                        &self.f,
                    )))
                    .map_err(|_| eyre!("duplicated scan"))?;
            }
            syn::Data::Enum(data) => {
                self.data
                    .set(DataInfo::Enum(
                        data.variants
                            .iter()
                            .map(|variant| {
                                IteratorImpl::from_enum(
                                    ctx.attributes.global_wraps()
                                        || ctx.attributes.variant_wraps(&variant.ident),
                                    variant,
                                    &self.f,
                                )
                            })
                            .collect::<Vec<_>>(),
                    ))
                    .map_err(|_| eyre!("duplicated scan"))?;
            }
            _ => {
                return Err(eyre::eyre!("only structs and enums are supported"));
            }
        }
        Ok(())
    }
}

impl<F, A: Attribute> Generate<A> for FieldAccessor<F>
where
    F: Fn(&syn::Type) -> bool,
{
    fn generate(&mut self, ctx: &mut DeriveContext<A>) -> eyre::Result<()> {
        let name = &ctx.input.ident;
        let accessor = &self.accessor;
        let matching_type = &self.matching_type;
        let iter = &self.iter;
        let lifetime = self.lifetime.clone();
        let instruction_trait = &ctx.trait_path;
        let Some(iter_generics) = self.generics.take() else {
            return Err(eyre::eyre!("FieldAccessor not scanned"));
        };
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        self.generate_trait_impl(ctx)?;

        let (_, ty_generics, _) = ctx.input.generics.split_for_impl();
        ctx.write_helper_impl(quote! {
            #[automatically_derived]
            pub struct #iter #iter_generics {
                parent: &#lifetime #name #ty_generics,
                index: usize,
            }
        });

        match &self.data.get().unwrap() {
            DataInfo::Struct(info) => {
                ctx.write_helper_impl(info.struct_iter_impl(
                    &name,
                    &iter,
                    &iter_generics,
                    matching_type,
                ));
            }
            DataInfo::Enum(info) => {
                let match_arms = info
                    .iter()
                    .map(|variant_info| variant_info.variant_iterator_impl(&name))
                    .collect::<Vec<_>>();

                ctx.write_helper_impl(quote! {
                    #[automatically_derived]
                    impl #iter_impl_generics Iterator for #iter #iter_ty_generics #iter_where_clause {
                        type Item = ::kirin_ir::#matching_type;
                        fn next(&mut self) -> Option<Self::Item> {
                            match self.parent {
                                #(#match_arms)*
                                _ => None,
                            }
                        }
                    }
                });
            }
        }
        Ok(())
    }
}

impl<F> FieldAccessor<F>
where
    F: Fn(&syn::Type) -> bool,
{
    fn data(&self) -> eyre::Result<&DataInfo> {
        self.data
            .get()
            .ok_or_else(|| eyre::eyre!("FieldAccessor not scanned"))
    }
    fn generate_trait_impl<A: Attribute>(&self, ctx: &mut DeriveContext<A>) -> eyre::Result<bool> {
        let accessor = &self.accessor;
        let iter = &self.iter;
        let matching_type = &self.matching_type;
        let instruction_trait = &ctx.trait_path;
        match self.data()? {
            DataInfo::Struct(PerInstructionInfo::Wraps(name)) => {
                ctx.write_trait_impl(quote! {
                    fn #accessor(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
                        let #name (wrapped_instruction) = self;
                        <wrapped_instruction as #instruction_trait>::#accessor()
                    }
                });
                Ok(true) // finished
            }
            DataInfo::Enum(info) if ctx.attributes.global_wraps() => {
                let arms = info.iter().map(|i| {
                    let PerInstructionInfo::Wraps(variant_name) = i else {
                        panic!("expected Wraps variant, when #[kirin(wraps)] is enabled globally");
                    };

                    quote! {
                        Self::#variant_name (wrapped_instruction) => {
                            <wrapped_instruction as #instruction_trait>::#accessor()
                        }
                    }
                });

                ctx.write_trait_impl(quote! {
                    fn #accessor(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
                        match self {
                            #(#arms)*
                        }
                    }
                });
                return Ok(true); // finished
            }
            DataInfo::Enum(info)
                if info
                    .iter()
                    .any(|i| matches!(i, PerInstructionInfo::Wraps(_))) =>
            {
                let arms = info.iter().filter_map(|i| {
                    let PerInstructionInfo::Wraps(variant_name) = i else {
                        return None;
                    };
                    Some(quote! {
                        Self::#variant_name (wrapped_instruction) => {
                            <wrapped_instruction as #instruction_trait>::#accessor()
                        }
                    })
                });
                ctx.write_trait_impl(quote! {
                    fn #accessor(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
                        match self {
                            #(#arms)*
                            _ => {
                                #iter {
                                    parent: self,
                                    index: 0,
                                }
                            }
                        }
                    }
                });
                return Ok(false); // continue to generate the helper impl
            }
            _ => {
                ctx.write_trait_impl(quote! {
                    fn #accessor(&self) -> impl Iterator<Item = ::kirin_ir::#matching_type> {
                        #iter {
                            parent: self,
                            index: 0,
                        }
                    }
                });
                return Ok(false); // continue to generate the helper impl
            }
        }
    }
}
