use kirin_lexer::Token;
use quote::quote;

use crate::{
    kirin::{
        attrs::*,
        extra::{FieldKind, FieldMeta},
    },
    prelude::*,
};

pub struct Format {
    crate_path: syn::Path,
    trait_path: syn::Path,
}

impl Layout for Format {
    type EnumAttr = KirinEnumOptions;
    type StructAttr = KirinStructOptions;
    type VariantAttr = KirinVariantOptions;
    type FieldAttr = KirinFieldOptions;
    type FieldExtra = FieldMeta;
    type StatementExtra = ();
}

impl DeriveTrait for Format {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl DeriveWithCratePath for Format {
    fn crate_path(&self) -> &syn::Path {
        &self.crate_path
    }
}

target! {
    pub struct Name
}

impl<'src> Compile<'src, Struct<'src, Format>, TokenStream> for Format {
    fn compile(&self, node: &Struct<'src, Format>) -> TokenStream {
        node.attrs().format.clone().map(|s| {
            let tokens = kirin_lexer::lex(&s)
                .collect::<Result<Vec<_>, String>>()
                .map_err(|e| syn::Error::new_spanned(node.input(), e));
        });
        quote! {}.into()
    }
}

/// Generate a default format if none is provided
/// the default format roughly corresponds to:
///
/// ```ignore
/// struct <name> {
///     field_0: SSAValue,
///     field_1: Vec<SSAValue>
///     field_2: Option<SSAValue>,
///     field_3: ResultValue,
///     field_4: Vec<ResultValue>,
///     field_5: Option<ResultValue>,
///     field_6: Block,
///     field_7: Vec<Block>,
///     field_8: Option<Block>,
///     field_9: Region,
///     field_10: Vec<Region>,
///     field_11: Option<Region>,
///     field_12: Successor,
///     field_13: Vec<Successor>,
///     field_14: Option<Successor>,
/// }
/// """
/// ${name:snake_case}(${field_0}, (${${field_1},}*), ${field_2}?) -> (${field_3}, (${${field_4},}*), ${field_5}?)
/// {{
/// ${field_6}
/// ${field_7}*
/// ${field_8}?
/// }}
/// {{ ${field_9:block}* }}
/// ${ {{ ${field_10:block}* }} }*
/// ${ {{ ${field_11:block}* }} }?
/// [field_12=${field_12}, field_13=(${${field_13},}*), field_14=${field_14}?]
/// """
/// ```
fn default_format<'src>(node: &Struct<'src, Format>) -> Vec<kirin_lexer::Token<'src>> {
    let mut tokens = Vec::new();
    // tokens.push(Token::Identifier(node.source().ident.to_string().as_str()));

    // for f in node.fields().iter() {
    //     tokens.push(Token::Identifier(f.source_ident().to_string().as_str()));
    //     let token = match f.extra().kind {
    //         FieldKind::ResultValue => Token::Quote(name),
    //         FieldKind::Block => Token::Quote(name),
    //         FieldKind::Region => Token::Quote(name),
    //     };
    // }
    tokens
}
