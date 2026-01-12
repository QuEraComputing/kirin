use std::collections::BTreeMap;

use super::{ast::GenericsImpl, attrs::*};
use crate::{
    chumsky::{
        ast::ASTNodeName,
        format::{Format, FormatElement},
    },
    kirin::extra::{FieldKind, FieldMeta},
    prelude::*,
};
use bon::Builder;
use proc_macro2::Span;
use quote::quote;

#[derive(Clone, Builder)]
pub struct DeriveHasParser {
    #[builder(default = syn::parse_quote!(kirin::parsers))]
    pub default_crate_path: syn::Path,
    #[builder(default = syn::parse_quote!(HasParser))]
    pub trait_path: syn::Path,
}

impl Layout for DeriveHasParser {
    type EnumAttr = ChumskyEnumOptions;
    type StructAttr = ChumskyStructOptions;
    type VariantAttr = ChumskyVariantOptions;
    type FieldAttr = ();
    type FieldExtra = FieldMeta;
    type StatementExtra = ();
}

impl DeriveWithCratePath for DeriveHasParser {
    fn default_crate_path(&self) -> &syn::Path {
        &self.default_crate_path
    }
}

impl DeriveTrait for DeriveHasParser {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

// target! {
//     pub struct StructImpl;
// }

// impl<'src> Compile<'src, DeriveHasParser, StructImpl> for Struct<'src, DeriveHasParser> {
//     fn compile(&self, ctx: &DeriveHasParser) -> StructImpl {
//         let trait_path: TraitPath = self.compile(ctx);
//         let crate_path: CratePath = self.compile(ctx);
//         let generics: GenericsImpl = self.compile(ctx);
//         let name: ASTNodeName = self.compile(ctx);
//         let (_, src_ty_generics, _) = self.source().generics.split_for_impl();
//         let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

//         if let Some(wrapper) = self.wrapper() {
//             let ty_wrapper = &wrapper.source().ty;
//             return quote! {
//                 impl #impl_generics #trait_path for #name #src_ty_generics #where_clause {
//                     type Output = #name #ty_generics;
//                     fn parser<I: #crate_path::TokenInput<'tokens, 'src>>() -> -> #crate_path::Boxed<'tokens, 'tokens, I, Self::Output, #crate_path::ParserError<'tokens, 'src>> {
//                         <#ty_wrapper as #trait_path>::parser()
//                             .map(|inner| #name { #wrapper: inner })
//                             .boxed()
//                     }
//                 }
//             }.into();
//         }

//         let Some(format) = &self.attrs().format else {
//             return syn::Error::new_spanned(
//                 self.source_ident(),
//                 "Chumsky parser derivation requires either a 'format' attribute or a wrapper statement",
//             )
//             .to_compile_error()
//             .into();
//         };

//         let fields = self.fields();
//         match Format::parse(format, None) {
//             Ok(format) => {
//                 let mut parser = quote! {};
//                 for elem in format.elements() {
//                     match elem {
//                         FormatElement::Token(tokens, span) => {
//                             // just(token_1).then_ignore(just(token_2))...
//                             let iter = tokens.iter();
//                             let Some(first) = iter.clone().next() else {
//                                 continue;
//                             };

//                             let mut elem_parser = quote! {#crate_path::chumsky::just(#first)};
//                             for token in iter.skip(1) {
//                                 elem_parser = quote! {#elem_parser.then_ignore(#crate_path::chumsky::just(#token))};
//                             }
//                             parser = quote! {#parser.then_ignore(#elem_parser)};
//                         },
//                         FormatElement::Field(name, opt, span) => {
//                             let Some(field) = fields
//                                 .iter()
//                                 .find(|f| f.source_ident().to_string() == *name) else {
//                                 return syn::Error::new(Span::call_site(), format!("No field named '{}' found in struct '{}'", name, self.source_ident()))
//                                     .to_compile_error()
//                                     .into();
//                                 };

//                             parser = match field.extra().kind {
//                                 FieldKind::SSAValue => {
//                                     quote! {
//                                         #parser.then(#crate_path::parsers::operand())
//                                     }
//                                 }
//                                 FieldKind::Block => {
//                                     quote! {
//                                         #parser.then(#crate_path::parsers::block())
//                                     }
//                                 }
//                                 _ => {
//                                     return syn::Error::new_spanned(
//                                         field.source_ident(),
//                                         "Chumsky parser derivation only supports fields of kind 'SSAValue' or 'ResultValue'",
//                                     )
//                                     .to_compile_error()
//                                     .into();
//                                 }
//                             };
//                         }
//                     }
//                 }

//                 quote! {
//                     impl #impl_generics #trait_path for #name #src_ty_generics #where_clause {
//                         type Output = #name #ty_generics;
//                     }
//                 }
//             },
//             Err(e) => {
//                 e.to_compile_error()
//             }
//         }.into()
//     }
// }

// pub struct FormatParser<'a, 'src> {
//     format: Format<'a>,
//     fields: Fields<'a, 'src, DeriveHasParser>,
//     error: Option<syn::Error>,
// }

// impl<'a, 'src> FormatParser<'a, 'src> {
//     pub fn from_struct(node: &'a Struct<'src, DeriveHasParser>) -> Self {
//         let attrs = node.attrs();
//         let Some(format) = attrs.format.as_ref() else {
//             return FormatParser {
//                 format: Format::default(),
//                 fields: node.fields(),
//                 error: Some(syn::Error::new_spanned(
//                     node.source_ident(),
//                     "Chumsky parser derivation requires a 'format' attribute",
//                 )),
//             };
//         };

//         match Format::parse(format, None) {
//             Ok(format) => FormatParser {
//                 format,
//                 fields: node.fields(),
//                 error: None,
//             },
//             Err(e) => FormatParser {
//                 format: Format::default(),
//                 fields: node.fields(),
//                 error: Some(e),
//             },
//         }
//     }
// }

// impl<'a, 'src> ToTokens for FormatParser<'a, 'src> {
//     fn to_tokens(&self, tokens: &mut TokenStream) {
//         if let Some(error) = &self.error {
//             error.to_compile_error().to_tokens(tokens);
//             return;
//         }

//         let fields = BTreeMap::from_iter(
//             self.fields
//                 .iter()
//                 .map(|f| (f.source_ident().to_string(), f)),
//         );

//         let mut parsers = vec![];
//         for elem in self.format.elements() {
//             match elem {
//                 FormatElement::Token(tokens, _) => {
//                     let mut iter = tokens.iter();
//                     let Some(first) = iter.next() else {
//                         continue;
//                     };

//                     let mut elem_parser = quote! {just(#first)};
//                     for token in iter {
//                         elem_parser = quote! {#elem_parser.ignore_then(just(#token))};
//                     }
//                     parsers.push(ParserStep::Ignore(elem_parser));
//                 }
//                 FormatElement::Field(name, opt, _) => {
//                     let Some(field) = fields.get(&name.to_string()) else {
//                         syn::Error::new(
//                             Span::call_site(),
//                             format!(
//                                 "No field named '{}' found in '{}'",
//                                 name,
//                                 self.fields.source_ident()
//                             ),
//                         )
//                         .to_compile_error()
//                         .to_tokens(tokens);
//                         return;
//                     };

//                     let field_parser = match field.extra().kind {
//                         FieldKind::SSAValue => {
//                             quote! {
//                                 parsers::operand()
//                             }
//                         }
//                         FieldKind::Block => {
//                             quote! {
//                                 parsers::block()
//                             }
//                         }
//                         _ => {
//                             syn::Error::new_spanned(
//                                 field.source_ident(),
//                                 "Chumsky parser derivation only supports fields of kind 'SSAValue' or 'ResultValue'",
//                             )
//                             .to_compile_error()
//                             .to_tokens(tokens);
//                             return;
//                         }
//                     };
//                     parsers.push(ParserStep::Keep(field_parser));
//                 }
//             }
//         }

//         use ParserStep::*;

//         let prev_parsers = parsers.iter();
//         let curr_parsers = prev_parsers.clone().skip(1);

//         let Some(first) = parsers.first() else {
//             return;
//         };

//         let mut parsers = match first {
//             ParserStep::Ignore(parser) => {
//                 quote! {#parser}
//             }
//             ParserStep::Keep(parser) => {
//                 quote! {#parser}
//             }
//         };

//         for (prev, curr) in prev_parsers.zip(curr_parsers) {
//             match (prev, curr) {
//                 (Ignore(_), Ignore(curr_parser)) => {
//                     parsers = quote! {
//                         #parsers.ignore_then(#curr_parser)
//                     };
//                 }
//                 (Ignore(_), Keep(curr_parser)) => {
//                     parsers = quote! {
//                         #parsers.ignore_then(#curr_parser)
//                     };
//                 }
//                 (Keep(_), Ignore(curr_parser)) => {
//                     parsers = quote! {
//                         #parsers.then_ignore(#curr_parser)
//                     };
//                 }
//                 (Keep(_), Keep(curr_parser)) => {
//                     parsers = quote! {
//                         #parsers.then(#curr_parser)
//                     };
//                 }
//             }
//         }
//         parsers.to_tokens(tokens);
//     }
// }

// pub struct ParserChain {
//     steps: Vec<ParserStep>,
//     vars: BTreeMap<String, TokenStream>,
// }

// impl ParserChain {
//     pub fn new() -> Self {
//         Self { steps: vec![] }
//     }

//     pub fn add_step(&mut self, step: ParserStep) {
//         self.steps.push(step);
//     }
// }

// pub enum ParserStep {
//     Ignore(TokenStream),
//     Keep(TokenStream),
// }

// impl ToTokens for ParserChain {
//     fn to_tokens(&self, tokens: &mut TokenStream) {
        
//     }
// }


// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_format_parser() {
//         let source: syn::DeriveInput = syn::parse_quote! {
//             #[chumsky(format = "test ({a}) {{ {b} }}")]
//             struct Test {
//                 a: SSAValue,
//                 b: Block,
//             }
//         };

//         let ctx = DeriveHasParser::builder().build();
//         let input = ctx.scan(&source).unwrap();

//         let Input::Struct(data) = input else {
//             panic!("Expected struct input");
//         };

//         let fmt = FormatParser::from_struct(&data);
//         let tokens = fmt.to_token_stream();
//         println!("{}", tokens);
//     }
// }