mod context;
mod enum_impl;
mod parse;
mod step;
mod struct_impl;

// impl Format<'_> {
//     fn to_tokens<'src>(
//         &self,
//         ctx: &DeriveHasParser,
//         fields: Fields<'_, 'src, DeriveHasParser>,
//     ) -> TokenStream {
//         let crate_path: CratePath = fields.compile(ctx);
//         let fields_map =
//             BTreeMap::from_iter(fields.iter().map(|f| (f.source_ident().to_string(), f)));
//         let mut iter = self.elements.iter();

//         let (mut chain, mut pattern, mut vars, mut is_keeping) = match iter.next() {
//             Some(FormatElement::Field(name, opt, span)) => {
//                 let var = format_ident!("{}", name);
//                 let Some(f) = fields_map.get(*name) else {
//                     let msg = format!("Field '{}' not found for format", name);
//                     return syn::Error::new(Span::call_site(), msg).to_compile_error();
//                 };

//                 let expr = match f.extra().kind {
//                     FieldKind::SSAValue => quote! { #crate_path::operand() },
//                     FieldKind::Block => quote! { #crate_path::block() },
//                     FieldKind::Successor => quote! { #crate_path::successor() },
//                     _ => {
//                         let msg = format!("Field '{}' cannot be used in format", name);
//                         return syn::Error::new(Span::call_site(), msg).to_compile_error();
//                     }
//                 };
//                 (quote! { #expr }, quote! {# var }, vec![var], true)
//             }
//             Some(FormatElement::Token(tokens_vec, span)) => {
//                 let mut iter = tokens_vec.iter();
//                 let Some(first_token) = iter.next() else {
//                     let msg = "Format string cannot be empty";
//                     return syn::Error::new(Span::call_site(), msg).to_compile_error();
//                 };
//                 let mut expr = quote! { just(#first_token) };
//                 for t in iter {
//                     expr = quote! { #expr.then(just(#t)) };
//                 }
//                 (expr, quote! { _ }, vec![], false)
//             }
//             None => {
//                 let msg = "Format string cannot be empty";
//                 return syn::Error::new(Span::call_site(), msg).to_compile_error();
//             }
//         };

//         for step in iter {
//             match step {
//                 FormatElement::Field(name, opt, span) => {
//                     let var = format_ident!("{}", name);
//                     let Some(f) = fields_map.get(*name) else {
//                         let msg = format!("Field '{}' not found for format", name);
//                         return syn::Error::new(Span::call_site(), msg).to_compile_error();
//                     };

//                     let expr = match f.extra().kind {
//                         FieldKind::SSAValue => quote! { #crate_path::operand() },
//                         FieldKind::Block => quote! { #crate_path::block() },
//                         FieldKind::Successor => quote! { #crate_path::successor() },
//                         _ => {
//                             let msg = format!("Field '{}' cannot be used in format", name);
//                             return syn::Error::new(Span::call_site(), msg).to_compile_error();
//                         }
//                     };

//                     if is_keeping {
//                         // Chain: .then(B)
//                         // Pattern: (prev, v1)
//                         chain = quote! { #chain.then(#expr) };
//                         pattern = quote! { (#pattern, #var) };
//                     } else {
//                         chain = quote! { #chain.ignore_then(#expr) };
//                         pattern = quote! { #var };
//                         is_keeping = true;
//                     }
//                     vars.push(var);
//                 }
//                 FormatElement::Token(token_vec, span) => {
//                     let mut iter = token_vec.iter();
//                     let Some(first_token) = iter.next() else {
//                         let msg = "Format string cannot be empty";
//                         return syn::Error::new(Span::call_site(), msg).to_compile_error();
//                     };
//                     let mut expr = quote! { just(#first_token) };
//                     for t in iter {
//                         expr = quote! { #expr.then(just(#t)) };
//                     }
//                     if is_keeping {
//                         chain = quote! { #chain.then_ignore(#expr) };
//                     } else {
//                         chain = quote! { #chain.ignore_then(#expr) };
//                     }
//                 }
//             }
//         }

//         let name = fields.source_ident();
//         quote! {
//             #chain.map(move |#pattern| {
//                 #name {
//                     #(#vars),*
//                 }
//             })
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use crate::ir::{HasFields, Input, ScanInto};

//     use super::*;

//     #[test]
//     fn test_format_parser() {
//         let input = "load {value:type} from {address}";
//         let format = Format::parse(input, None).expect("Failed to parse format");

//         insta::assert_debug_snapshot!(format);
//     }

//     #[test]
//     fn test_to_tokens() {
//         let source: syn::DeriveInput = syn::parse_quote! {
//             #[chumsky(format = "test ({a}, {b}) {{ {c} }}")]
//             struct Test {
//                 a: SSAValue,
//                 b: SSAValue,
//                 c: Block,
//             }
//         };

//         let ctx = DeriveHasParser::builder().build();
//         let input = ctx.scan(&source).unwrap();

//         let Input::Struct(data) = input else {
//             panic!("Expected struct input");
//         };

//         let fmt = Format::parse("test ({a}, {b}) {{ {c} }}", None).unwrap();
//         let tokens = fmt.to_tokens(&ctx, data.fields());
//         println!("{}", tokens);
//     }
// }
