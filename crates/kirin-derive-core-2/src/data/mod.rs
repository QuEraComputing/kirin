/// representation of a statement, could either be a struct or enum variant
mod core;
mod enum_impl;
mod field;
pub mod gadgets;
mod struct_impl;
mod traits;

pub use core::Statement;
pub use enum_impl::DialectEnum;
pub use field::{Field, Fields, FieldMember};
pub use struct_impl::DialectStruct;
pub use traits::{Compile, Context, FromContext, SimpleTraitDerive};

pub enum Dialect<'src, Ctx: Context<'src>> {
    Struct(struct_impl::DialectStruct<'src, Ctx>),
    Enum(enum_impl::DialectEnum<'src, Ctx>),
}

impl<'src, Ctx: Context<'src>> traits::FromContext<'src, Ctx, syn::DeriveInput>
    for Dialect<'src, Ctx>
{
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        match &node.data {
            syn::Data::Struct(_) => Ok(Dialect::Struct(struct_impl::DialectStruct::from_context(
                ctx, node,
            )?)),
            syn::Data::Enum(_) => Ok(Dialect::Enum(enum_impl::DialectEnum::from_context(
                ctx, node,
            )?)),
            _ => Err(syn::Error::new_spanned(
                node,
                "Dialect can only be created from struct or enum data",
            )),
        }
    }
}

impl<'src, Ctx: Context<'src>> Dialect<'src, Ctx> {
    pub fn attrs(&self) -> &Ctx::AttrGlobal {
        match self {
            Dialect::Struct(s) => &s.attrs,
            Dialect::Enum(e) => &e.attrs,
        }
    }

    pub fn input(&self) -> &'src syn::DeriveInput {
        match self {
            Dialect::Struct(s) => s.input(),
            Dialect::Enum(e) => e.input(),
        }
    }
}

impl<'src, Ctx: Context<'src>> std::fmt::Debug for Dialect<'src, Ctx>
where
    Ctx::AttrGlobal: std::fmt::Debug,
    struct_impl::DialectStruct<'src, Ctx>: std::fmt::Debug,
    enum_impl::DialectEnum<'src, Ctx>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dialect::Struct(s) => f.debug_tuple("Dialect::Struct").field(s).finish(),
            Dialect::Enum(e) => f.debug_tuple("Dialect::Enum").field(e).finish(),
        }
    }
}
