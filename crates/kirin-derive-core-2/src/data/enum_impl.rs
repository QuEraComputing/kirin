use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::ToTokens;

use super::core::Statement;
use super::traits::*;

pub struct DialectEnum<'src, Ctx: Context<'src>> {
    pub attrs: Ctx::AttrGlobal,
    pub wraps: bool,
    pub src: &'src syn::DeriveInput,
    pub variants: Vec<Statement<'src, syn::Variant, Ctx>>,
}

impl<'src, Ctx: Context<'src>> DialectEnum<'src, Ctx> {
    pub fn input(&self) -> &'src syn::DeriveInput {
        self.src
    }

    /// Returns the idents of all variants in the enum.
    ///
    /// !!! Note
    /// Usually used in conjunction with `unpacking` to form match arms.
    pub fn variant_idents(&self) -> Vec<&syn::Ident> {
        self.variants.iter().map(|v| &v.src.ident).collect()
    }

    /// Returns the unpacking patterns for all variants in the enum.
    ///
    /// !!! Note
    /// Usually used in conjunction with `variant_idents` to form match arms.
    pub fn unpacking(&self) -> Vec<TokenStream> {
        self.variants
            .iter()
            .map(|v| v.fields.unpacking().to_token_stream())
            .collect()
    }

    /// Compiles the match arm bodies for all variants in the enum.
    ///
    /// !!! Note
    /// The type `T` is the output type of the compilation, usually `TokenStream`.
    /// Usually used in conjunction with `variant_idents` and `unpacking` to form match arms.
    pub fn match_action<T>(&self, ctx: &Ctx) -> Vec<T>
    where
        T: ToTokens,
        Ctx: Compile<'src, Statement<'src, syn::Variant, Ctx>, T>,
    {
        self.variants.iter().map(|v| ctx.compile(v)).collect()
    }
}

impl<'src, Ctx: Context<'src>> TopLevel<'src, Ctx> for DialectEnum<'src, Ctx> {
    fn attrs_global(&self) -> &Ctx::AttrGlobal {
        &self.attrs
    }
}

impl<'src, Ctx: Context<'src>> Source for DialectEnum<'src, Ctx> {
    type Output = syn::DeriveInput;
    fn source(&self) -> &Self::Output {
        self.src
    }
}

impl<'src, Ctx: Context<'src>> HasGenerics for DialectEnum<'src, Ctx> {
    fn generics(&self) -> &syn::Generics {
        &self.src.generics
    }
}

impl<'src, Ctx: Context<'src>> ContainsWrapper for DialectEnum<'src, Ctx> {
    fn contains_wrapper(&self) -> bool {
        self.wraps || self.variants.iter().any(|v| v.contains_wrapper())
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::DeriveInput> for DialectEnum<'src, Ctx> {
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        let syn::Data::Enum(data) = &node.data else {
            return Err(syn::Error::new_spanned(
                node,
                "DialectEnum can only be created from enum data",
            ));
        };

        let wraps = node.attrs.iter().any(|attr| attr.path().is_ident("wraps"));
        let mut variants = data
            .variants
            .iter()
            .map(|variant| Statement::from_context(ctx, variant))
            .collect::<syn::Result<Vec<_>>>()?;

        if wraps {
            for variant in &mut variants {
                variant.wraps = true;
                variant.fields.set_wrapper()?;
            }
        }

        Ok(DialectEnum {
            attrs: Ctx::AttrGlobal::from_derive_input(node)?,
            wraps,
            src: node,
            variants,
        })
    }
}

impl<'src, Ctx: Context<'src>> std::fmt::Debug for DialectEnum<'src, Ctx>
where
    Ctx::AttrGlobal: std::fmt::Debug,
    Statement<'src, syn::Variant, Ctx>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialectEnum")
            .field("wraps", &self.wraps)
            .field("attrs", &self.attrs)
            .field("variants", &self.variants)
            .finish()
    }
}
