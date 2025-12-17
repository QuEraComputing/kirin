use bon::Builder;
use proc_macro2::TokenStream;

use crate::utils::{from_str, strip_path};
use crate::{
    data::*,
    kirin::attrs::{KirinFieldOptions, KirinGlobalOptions, KirinStatementOptions},
};

pub trait SearchProperty: Sized {
    /// how to search for property in global-level enum attributes
    fn search_globally_enum<'src>(data: &DialectEnum<'src, Property<Self>>) -> TokenStream;
    /// how to search for property in statement-level attributes
    fn search_in_statement<'src, S>(data: &Statement<'src, S, Property<Self>>) -> TokenStream;
    /// how to combine global and statement-level property expressions
    fn combine(glob: &TokenStream, stmt: &TokenStream) -> TokenStream;
}

macro_rules! boolean_property {
    ($name:ident, $key:ident) => {
        pub struct $name;

        impl SearchProperty for $name {
            fn search_globally_enum<'src>(data: &DialectEnum<'src, Property<Self>>) -> TokenStream {
                let glob = data.attrs.$key;
                quote::quote! { #glob }
            }

            fn search_in_statement<'src, S>(
                data: &Statement<'src, S, Property<Self>>,
            ) -> TokenStream {
                let stmt = data.attrs.$key;
                quote::quote! { #stmt }
            }

            fn combine(glob: &TokenStream, stmt: &TokenStream) -> TokenStream {
                quote::quote! { #glob || #stmt }
            }
        }
    };
}

boolean_property!(IsConstant, constant);
boolean_property!(IsPure, pure);
boolean_property!(IsTerminator, terminator);

#[derive(Debug, Clone, Builder)]
pub struct Property<S: SearchProperty> {
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub default_crate_path: syn::Path,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_path: syn::Path,
    #[builder(default = strip_path(&trait_path))]
    pub trait_name: syn::Ident,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_method: syn::Ident,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub value_type: syn::Type,
    #[builder(default)]
    marker: std::marker::PhantomData<S>,
}

impl<'src, S: SearchProperty> Context<'src> for Property<S> {
    type AttrGlobal = KirinGlobalOptions;
    type AttrStatement = KirinStatementOptions;
    type AttrField = KirinFieldOptions;
    type FieldExtra = (); // all info are in attributes
    type StatementExtra = ();

    fn helper_attribute() -> &'static str {
        "kirin"
    }
}

impl<'src, S: SearchProperty> TraitContext<'src> for Property<S> {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl<'src, S: SearchProperty> AllowCratePath<'src> for Property<S> {
    fn crate_path(&self) -> &syn::Path {
        &self.default_crate_path
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{FromContext, Statement};

    use super::*;

    fn ctx() -> Property<IsConstant> {
        Property::builder()
            .default_crate_path("::kirin::ir")
            .trait_path("MyFieldIterTrait")
            .trait_method("my_field_iter")
            .value_type("bool")
            .build()
    }

    #[test]
    fn test_simple() {
        let ctx = ctx();

        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(constant, pure, terminator)]
            struct MyStruct {
                a: i32,
                b: i32,
            }
        };

        let data = Statement::from_context(&ctx, &input).unwrap();
        insta::assert_snapshot!(format!("{:#?}", data));
    }
}
