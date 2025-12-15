use bon::Builder;

use super::extra::FieldExtra;
use crate::data::*;
use crate::kirin::attrs::*;
use crate::utils::to_camel_case;

/// context information for deriving field iterators with following signature:
///
/// ```ignore
/// pub trait <TraitName><'trait_lifetime, ...> {
///    type <IterName>: Iterator<Item = &mut <MatchingTypePath>>;
///    fn <TraitMethod>(&'trait_lifetime self) -> Self::<IterName>;
/// }
/// ```
#[derive(Clone, Builder)]
pub struct FieldsIter {
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub default_crate_path: syn::Path,
    pub mutable: bool,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_path: syn::Path,
    #[builder(default = strip_path(&trait_path))]
    pub trait_name: syn::Ident,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_lifetime: syn::Lifetime,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_type_iter: syn::Ident,
    // #[builder(default, with = |s: impl Into<String>| from_str(s))]
    // pub trait_generics: syn::Generics,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub trait_method: syn::Ident,
    /// name of the iterator to generate
    #[builder(
        default = quote::format_ident!("{}Iter", to_camel_case(&trait_method.to_string())),
        with = |s: impl Into<String>| from_str(s)
    )]
    pub iter_name: syn::Ident,
    #[builder(with = |s: impl Into<String>| from_str(s))]
    pub matching_type: syn::Path,
    #[builder(default = strip_path(&matching_type))]
    pub matching_type_name: syn::Ident,
}

fn strip_path(path: &syn::Path) -> syn::Ident {
    path.segments
        .last()
        .expect("matching_type_path must have at least one segment")
        .ident
        .clone()
}

fn from_str<T: syn::parse::Parse>(s: impl Into<String>) -> T {
    syn::parse_str(&s.into()).unwrap()
}

impl<'src> Context<'src> for FieldsIter {
    type AttrGlobal = KirinGlobalOptions;
    type AttrStatement = KirinStatementOptions;
    type AttrField = KirinFieldOptions;
    type FieldExtra = FieldExtra;
    type StatementExtra = ();

    fn helper_attribute() -> &'static str {
        "kirin"
    }

    fn crate_path(&self) -> &syn::Path {
        &self.default_crate_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> FieldsIter {
        FieldsIter::builder()
            .default_crate_path("kirin")
            .mutable(true)
            .trait_path("MyFieldIterTrait")
            .trait_lifetime("'a")
            .trait_method("my_field_iter")
            .trait_type_iter("Iter")
            .matching_type("T")
            .build()
    }

    #[test]
    fn test_simple() {
        let ctx = ctx();

        let input: syn::DeriveInput = syn::parse_quote! {
            struct MyStruct {
                a: i32,
                b: i32,
            }
        };

        let data = Statement::from_context(&ctx, &input).unwrap();
        insta::assert_snapshot!(format!("{:#?}", data));
    }

    #[test]
    fn test_with_no_fn() {
        let ctx = ctx();
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = MyLattice, crate = some_path)]
            struct MyStruct {
                a: i32,
                b: T,
            }
        };

        let data = Dialect::from_context(&ctx, &input).unwrap();
        insta::assert_snapshot!(format!("{:#?}", data));
    }

    #[test]
    fn test_with_wraps() {
        let ctx = ctx();
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = MyLattice, crate = some_path)]
            struct MyStruct {
                #[wraps]
                other: Other
            }
        };

        let data = Dialect::from_context(&ctx, &input).unwrap();
        insta::assert_snapshot!(format!("{:#?}", data));
    }
}
