use super::enum_impl::DialectEnum;
use super::struct_impl::DialectStruct;
use super::traits::*;

pub enum Dialect<'src, Ctx: Context<'src>> {
    Struct(DialectStruct<'src, Ctx>),
    Enum(DialectEnum<'src, Ctx>),
}

impl<'src, Ctx: Context<'src>> Source for Dialect<'src, Ctx> {
    type Output = syn::DeriveInput;
    fn source(&self) -> &Self::Output {
        match self {
            Dialect::Struct(s) => s.source(),
            Dialect::Enum(e) => e.source(),
        }
    }
}

impl<'src, Ctx: Context<'src>> FromContext<'src, Ctx, syn::DeriveInput> for Dialect<'src, Ctx> {
    fn from_context(ctx: &Ctx, node: &'src syn::DeriveInput) -> syn::Result<Self> {
        match &node.data {
            syn::Data::Struct(_) => Ok(Dialect::Struct(DialectStruct::from_context(ctx, node)?)),
            syn::Data::Enum(_) => Ok(Dialect::Enum(DialectEnum::from_context(ctx, node)?)),
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

    pub fn combined_generics(&self, other: &syn::Generics) -> syn::Generics {
        match self {
            Dialect::Struct(s) => s.combine_generics(other),
            Dialect::Enum(e) => e.combine_generics(other),
        }
    }
}

impl<'src, Ctx: Context<'src>> HasGenerics for Dialect<'src, Ctx> {
    fn generics(&self) -> &syn::Generics {
        match self {
            Dialect::Struct(s) => s.generics(),
            Dialect::Enum(e) => e.generics(),
        }
    }
}

impl<'src, Ctx: Context<'src>> ContainsWrapper for Dialect<'src, Ctx> {
    fn contains_wrapper(&self) -> bool {
        match self {
            Dialect::Struct(s) => s.contains_wrapper(),
            Dialect::Enum(e) => e.contains_wrapper(),
        }
    }
}

impl<'src, Ctx: Context<'src>> std::fmt::Debug for Dialect<'src, Ctx>
where
    Ctx::AttrGlobal: std::fmt::Debug,
    DialectStruct<'src, Ctx>: std::fmt::Debug,
    DialectEnum<'src, Ctx>: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dialect::Struct(s) => f.debug_tuple("Dialect::Struct").field(s).finish(),
            Dialect::Enum(e) => f.debug_tuple("Dialect::Enum").field(e).finish(),
        }
    }
}
