use quote::quote;

use crate::prelude::*;
use crate::kirin::field::context::FieldsIter;
use crate::target;

use super::expr::Expr;
use super::impl_head::ImplHead;
use super::item::MatchingItem;
use super::name::Name;
use super::ty::InnerType;
use super::type_head::TypeHead;

/// Type definition of the struct iterator
pub type StructTypeDef = Alt<WrapperStructTypeDef, RegularStructTypeDef>;

/// Expression to create an instance of the struct iterator
pub type StructExpr = Alt<WrapperStructExpr, RegularStructExpr>;

target! {
    /// Struct impl for field iterator
    pub struct StructImpl
}

impl<'src> Compile<'src, Struct<'src, Self>, StructImpl> for FieldsIter {
    fn compile(&self, node: &Struct<'src, Self>) -> StructImpl {
        let iter: StructTypeDef = self.compile(node);
        let item: MatchingItem = self.compile(node);
        let impl_head: ImplHead = self.compile(node);
        StructImpl(quote! {
            #iter
            #impl_head {
                type Item = #item;
                fn next(&mut self) -> Option<Self::Item> {
                    self.inner.next()
                }
            }
        })
    }
}

target! {
    /// Regular struct type definition
    pub struct RegularStructTypeDef
}

impl<'src> Compile<'src, Struct<'src, Self>, RegularStructTypeDef> for FieldsIter {
    fn compile(&self, node: &Struct<'src, Self>) -> RegularStructTypeDef {
        let head: TypeHead = self.compile(node);
        let ty: InnerType = self.compile(node);
        RegularStructTypeDef(quote! {
            #[automatically_derived]
            pub struct #head {
                inner: #ty,
            }
        })
    }
}

target! {
    /// Wrapper struct type definition
    pub struct WrapperStructTypeDef
}

impl<'src> Compile<'src, Struct<'src, Self>, WrapperStructTypeDef> for FieldsIter {
    fn compile(&self, node: &Struct<'src, Self>) -> WrapperStructTypeDef {
        let trait_path = &self.trait_path;
        let trait_type_iter = &self.trait_type_iter;
        let head: TypeHead = self.compile(node);
        let wrapped_type = node.wrapper_type_tokens();
        WrapperStructTypeDef(quote! {
            #[automatically_derived]
            pub struct #head {
                inner: <#wrapped_type as #trait_path>::#trait_type_iter,
            }
        })
    }
}

target! {
    pub struct RegularStructExpr
}

impl<'src> Compile<'src, Struct<'src, Self>, RegularStructExpr> for FieldsIter {
    fn compile(&self, node: &Struct<'src, Self>) -> RegularStructExpr {
        let name: Name = self.compile(node);
        let expr: Expr = self.compile(node);
        RegularStructExpr(quote! {
            #name {
                inner: #expr,
            }
        })
    }
}

target! {
    pub struct WrapperStructExpr
}

impl<'src> Compile<'src, Struct<'src, Self>, WrapperStructExpr> for FieldsIter {
    fn compile(&self, node: &Struct<'src, Self>) -> WrapperStructExpr {
        let iter_name: Name = self.compile(node);
        let trait_path = &self.trait_path;
        let trait_method = &self.trait_method;
        let wrapped_ty = node.wrapper_type_tokens();
        let wrapper = node.wrapper_tokens();
        WrapperStructExpr(quote! {
            #iter_name {
                inner: <#wrapped_ty as #trait_path>::#trait_method(#wrapper),
            }
        })
    }
}
