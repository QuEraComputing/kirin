mod attribute;

use proc_macro2::TokenStream;

use crate::{
    DeriveContext, DeriveTrait, FieldAccessor, WriteTokenStream,
    accessor::Config,
    instruction::attribute::{AttributeInfo, DeriveAttribute},
};

pub struct DeriveInstruction;

impl DeriveTrait for DeriveInstruction {
    fn scan(_ctx: &DeriveContext<Self::HelperAttribute>) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self)
    }

    fn trait_path() -> TokenStream {
        syn::parse_str::<TokenStream>("::kirin_ir::Instruction").unwrap()
    }

    fn generate(input: syn::DeriveInput) -> TokenStream {
        let mut ctx: DeriveContext<AttributeInfo> =
            DeriveContext::new(Self::trait_path(), input.clone());
        ctx.write_helper_impl(DeriveHasArguments::generate(input.clone()));
        ctx.write_helper_impl(DeriveHasResults::generate(input.clone()));
        ctx.write_helper_impl(DeriveHasSuccessors::generate(input.clone()));
        ctx.write_helper_impl(DeriveHasRegions::generate(input.clone()));
        ctx.write_helper_impl(DeriveIsTerminator::generate(input.clone()));
        ctx.write_helper_impl(DeriveIsConstant::generate(input.clone()));
        ctx.write_helper_impl(DeriveIsPure::generate(input.clone()));
        ctx.generate()
    }
}

impl WriteTokenStream for DeriveInstruction {
    type HelperAttribute = AttributeInfo;
    fn write_token(&mut self, _ctx: &mut DeriveContext<Self::HelperAttribute>) -> eyre::Result<()> {
        Ok(())
    }
}

pub struct DeriveHasArguments(FieldAccessor<AttributeInfo>);
pub struct DeriveHasResults(FieldAccessor<AttributeInfo>);
pub struct DeriveHasSuccessors(FieldAccessor<AttributeInfo>);
pub struct DeriveHasRegions(FieldAccessor<AttributeInfo>);
pub struct DeriveIsTerminator(DeriveAttribute);
pub struct DeriveIsConstant(DeriveAttribute);
pub struct DeriveIsPure(DeriveAttribute);

macro_rules! impl_accessor {
    ($name:ident, $accessor:expr, $matching_type:expr,$trait_path:expr) => {
        impl DeriveTrait for $name {
            fn scan(ctx: &DeriveContext<Self::HelperAttribute>) -> eyre::Result<Self>
            where
                Self: Sized,
            {
                Ok(Self(FieldAccessor::new(
                    Config::new($accessor, $matching_type, $trait_path),
                    ctx,
                )))
            }

            fn trait_path() -> proc_macro2::TokenStream {
                syn::parse_str::<proc_macro2::TokenStream>($trait_path).unwrap()
            }
        }

        impl WriteTokenStream for $name {
            type HelperAttribute = AttributeInfo;
            fn write_token(
                &mut self,
                ctx: &mut crate::DeriveContext<AttributeInfo>,
            ) -> eyre::Result<()> {
                self.0.write_token(ctx)
            }
        }
    };
}

impl_accessor!(
    DeriveHasArguments,
    "arguments",
    "SSAValue",
    "::kirin_ir::HasArguments"
);
impl_accessor!(
    DeriveHasResults,
    "results",
    "ResultValue",
    "::kirin_ir::HasResults"
);
impl_accessor!(
    DeriveHasSuccessors,
    "successors",
    "Block",
    "::kirin_ir::HasSuccessors"
);
impl_accessor!(
    DeriveHasRegions,
    "regions",
    "Region",
    "::kirin_ir::HasRegions"
);

macro_rules! impl_checker {
    ($name:ident, $method:expr, $trait_path:expr) => {
        impl DeriveTrait for $name {
            fn scan(_ctx: &DeriveContext<Self::HelperAttribute>) -> eyre::Result<Self>
            where
                Self: Sized,
            {
                Ok(Self(DeriveAttribute($method.to_string())))
            }

            fn trait_path() -> proc_macro2::TokenStream {
                syn::parse_str::<proc_macro2::TokenStream>($trait_path).unwrap()
            }
        }

        impl WriteTokenStream for $name {
            type HelperAttribute = AttributeInfo;
            fn write_token(
                &mut self,
                ctx: &mut crate::DeriveContext<AttributeInfo>,
            ) -> eyre::Result<()> {
                self.0.write_token(ctx)
            }
        }
    };
}

impl_checker!(
    DeriveIsTerminator,
    "is_terminator",
    "::kirin_ir::IsTerminator"
);
impl_checker!(DeriveIsConstant, "is_constant", "::kirin_ir::IsConstant");
impl_checker!(DeriveIsPure, "is_pure", "::kirin_ir::IsPure");

#[cfg(test)]
mod tests;
