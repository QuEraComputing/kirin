use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::data::{
    Compile, ContainsWrapper, Context, Dialect, DialectEnum, DialectStruct, Statement,
};

/// Alternative between two implementations.
///
/// If the source node is `Statement`:
/// - wrapper statement, use `A`
/// - normal statement, use `B`
///
/// If the source node is `Dialect`:
/// - struct dialect, use `A`
/// - enum dialect, use `B`
pub struct Alt<A, B> {
    tokens: TokenStream,
    marker: std::marker::PhantomData<(A, B)>,
}

impl<A, B> Alt<A, B> {
    pub fn new(tokens: TokenStream) -> Self {
        Self {
            tokens,
            marker: std::marker::PhantomData,
        }
    }
}

impl<A, B> ToTokens for Alt<A, B> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.tokens.to_tokens(tokens);
    }
}

impl<A, B> From<TokenStream> for Alt<A, B> {
    fn from(tokens: TokenStream) -> Self {
        Self {
            tokens,
            marker: std::marker::PhantomData,
        }
    }
}

impl<A, B> From<Alt<A, B>> for TokenStream {
    fn from(either: Alt<A, B>) -> Self {
        either.tokens
    }
}

impl<'src, Src, W, R, Ctx> Compile<'src, Statement<'src, Src, Ctx>, Alt<W, R>> for Ctx
where
    Ctx: Context<'src>
        + Compile<'src, Statement<'src, Src, Ctx>, W>
        + Compile<'src, Statement<'src, Src, Ctx>, R>,
    W: ToTokens + Into<TokenStream>,
    R: ToTokens + Into<TokenStream>,
{
    fn compile(&self, node: &Statement<'src, Src, Ctx>) -> Alt<W, R> {
        if node.contains_wrapper() {
            let s: W = self.compile(node);
            Alt::new(s.into())
        } else {
            let e: R = self.compile(node);
            Alt::new(e.into())
        }
    }
}

impl<'src, W, R, Ctx> Compile<'src, DialectStruct<'src, Ctx>, Alt<W, R>> for Ctx
where
    Ctx: Context<'src>
        + Compile<'src, DialectStruct<'src, Ctx>, W>
        + Compile<'src, DialectStruct<'src, Ctx>, R>,
    W: ToTokens + Into<TokenStream>,
    R: ToTokens + Into<TokenStream>,
{
    fn compile(&self, node: &DialectStruct<'src, Ctx>) -> Alt<W, R> {
        if node.contains_wrapper() {
            let s: W = self.compile(node);
            Alt::new(s.into())
        } else {
            let e: R = self.compile(node);
            Alt::new(e.into())
        }
    }
}

impl<'src, S, E, Ctx> Compile<'src, Dialect<'src, Ctx>, Alt<S, E>> for Ctx
where
    Ctx: Context<'src>
        + Compile<'src, DialectStruct<'src, Ctx>, S>
        + Compile<'src, DialectEnum<'src, Ctx>, E>,
    S: ToTokens + Into<TokenStream>,
    E: ToTokens + Into<TokenStream>,
{
    fn compile(&self, node: &Dialect<'src, Ctx>) -> Alt<S, E> {
        match node {
            Dialect::Struct(s) => {
                let s: S = self.compile(s);
                Alt::new(s.into())
            }
            Dialect::Enum(e) => {
                let e: E = self.compile(e);
                Alt::new(e.into())
            }
        }
    }
}
