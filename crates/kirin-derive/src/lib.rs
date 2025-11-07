extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod fields;

use crate::fields::FieldInfoFilter;

#[proc_macro_derive(Instruction)]
pub fn derive_instruction(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let result_info = ast.data.filter_fields(
        &ast,
        |ty| matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident("ResultValue")),
    );
    let results = result_info.filtering_function("ResultIter", "results", "ResultValue");
    let results_iterator = result_info.filtering_iterator("ResultIter");
    let results_iterator_impl = result_info.filtering_iterator_impl("ResultIter", "ResultValue");

    let argument_info = ast.data.filter_fields(
        &ast,
        |ty| matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident("SSAValue")),
    );
    let arguments = argument_info.filtering_function("ArgumentIter", "arguments", "SSAValue");
    let arguments_iterator = argument_info.filtering_iterator("ArgumentIter");
    let arguments_iterator_impl = argument_info.filtering_iterator_impl("ArgumentIter", "SSAValue");

    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let generated = quote! {
        #[automatically_derived]
        impl #impl_generics ::kirin_ir::Instruction for #name #ty_generics #where_clause {
            #results
            #arguments
        }
        #results_iterator
        #results_iterator_impl
        #arguments_iterator
        #arguments_iterator_impl
    };
    // panic!("{}", generated.to_string());
    generated.into()
}
