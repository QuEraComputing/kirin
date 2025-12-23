extern crate proc_macro;

use kirin_derive_core::kirin::prelude::*;
use kirin_derive_core::chumsky::prelude::*;
use kirin_derive_core::prelude::*;
use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(Dialect, attributes(kirin, wraps))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let mut tokens = proc_macro2::TokenStream::new();

    for (mutable, trait_name, matching_type, trait_method, trait_type_iter) in [
        (false, "HasArguments", "SSAValue", "arguments", "Iter"),
        (
            true,
            "HasArgumentsMut",
            "SSAValue",
            "arguments_mut",
            "IterMut",
        ),
        (false, "HasResults", "ResultValue", "results", "Iter"),
        (
            true,
            "HasResultsMut",
            "ResultValue",
            "results_mut",
            "IterMut",
        ),
        (false, "HasBlocks", "Block", "blocks", "Iter"),
        (true, "HasBlocksMut", "Block", "blocks_mut", "IterMut"),
        (false, "HasSuccessors", "Successor", "successors", "Iter"),
        (
            true,
            "HasSuccessorsMut",
            "Successor",
            "successors_mut",
            "IterMut",
        ),
        (false, "HasRegions", "Region", "regions", "Iter"),
        (true, "HasRegionsMut", "Region", "regions_mut", "IterMut"),
    ] {
        FieldsIter::builder()
            .trait_lifetime("'a")
            .default_crate_path("::kirin::ir")
            .mutable(mutable)
            .trait_path(trait_name)
            .matching_type(matching_type)
            .trait_method(trait_method)
            .trait_type_iter(trait_type_iter)
            .build()
            .emit(&ast)
            .to_tokens(&mut tokens);
    }

    Property::<IsTerminator>::builder()
        .default_crate_path("::kirin::ir")
        .trait_path("IsTerminator")
        .trait_method("is_terminator")
        .value_type("bool")
        .build()
        .emit(&ast)
        .to_tokens(&mut tokens);

    Property::<IsConstant>::builder()
        .default_crate_path("::kirin::ir")
        .trait_path("IsConstant")
        .trait_method("is_constant")
        .value_type("bool")
        .build()
        .emit(&ast)
        .to_tokens(&mut tokens);

    Property::<IsPure>::builder()
        .default_crate_path("::kirin::ir")
        .trait_path("IsPure")
        .trait_method("is_pure")
        .value_type("bool")
        .build()
        .emit(&ast)
        .to_tokens(&mut tokens);

    Builder::default().emit(&ast).to_tokens(&mut tokens);
    DialectMarker::builder()
        .crate_path("::kirin::ir")
        .trait_path("Dialect")
        .build()
        .emit(&ast)
        .to_tokens(&mut tokens);
    // let name = derive_name!(&ast);
    tokens.into()
}

macro_rules! derive_fields_iter {
    ($mutable:expr, $name:ident, $matching_type:ident, $trait_method:ident, $trait_type_iter:ident) => {
        paste::paste! {
            #[proc_macro_derive($name, attributes(kirin, wraps))]
            pub fn [<derive_ $name:snake>](input: TokenStream) -> TokenStream {
                let ast = parse_macro_input!(input as syn::DeriveInput);
                FieldsIter::builder()
                        .trait_lifetime("'a")
                        .default_crate_path("::kirin::ir")
                        .mutable($mutable)
                        .trait_path(stringify!($name))
                        .matching_type(stringify!($matching_type))
                        .trait_method(stringify!($trait_method))
                        .trait_type_iter(stringify!($trait_type_iter))
                        .build()
                        .emit(&ast).into()
            }
        }
    };
}

derive_fields_iter!(false, HasArguments, SSAValue, arguments, Iter);
derive_fields_iter!(true, HasArgumentsMut, SSAValue, arguments_mut, IterMut);
derive_fields_iter!(false, HasResults, ResultValue, results, Iter);
derive_fields_iter!(true, HasResultsMut, ResultValue, results_mut, IterMut);
derive_fields_iter!(false, HasBlocks, Block, blocks, Iter);
derive_fields_iter!(true, HasBlocksMut, Block, blocks_mut, IterMut);
derive_fields_iter!(false, HasSuccessors, Successor, successors, Iter);
derive_fields_iter!(true, HasSuccessorsMut, Successor, successors_mut, IterMut);
derive_fields_iter!(false, HasRegions, Region, regions, Iter);
derive_fields_iter!(true, HasRegionsMut, Region, regions_mut, IterMut);

macro_rules! derive_property {
    ($name:ident) => {
        paste::paste! {
            #[proc_macro_derive($name, attributes(kirin, wraps))]
            pub fn [<derive_ $name:snake>](input: TokenStream) -> TokenStream {
                let ast = parse_macro_input!(input as syn::DeriveInput);
                Property::<$name>::builder()
                    .default_crate_path("::kirin::ir")
                    .trait_path(stringify!($name))
                    .trait_method(stringify!($name:snake))
                    .value_type("bool")
                    .build()
                    .emit(&ast)
                    .into()
            }
        }
    };
}

derive_property!(IsTerminator);
derive_property!(IsConstant);
derive_property!(IsPure);

#[proc_macro_derive(WithAbstractSyntaxTree, attributes(chumsky, wraps))]
pub fn derive_with_abstract_syntax_tree(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    DeriveAST::builder()
        .build()
        .emit(&ast).into()
}
