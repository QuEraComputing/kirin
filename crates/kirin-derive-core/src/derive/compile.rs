use crate::ir::*;

pub trait Compile<'src, N, T> {
    fn compile(&self, node: &N) -> T;
}

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

impl<'src, L, S, E, Ctx> Compile<'src, Input<'src, L>, Alt<S, E>> for Ctx
where
    L: Layout + 'src,
    S: ToTokens,
    E: ToTokens,
    Ctx: Compile<'src, Struct<'src, L>, S> + Compile<'src, Enum<'src, L>, E>,
{
    fn compile(&self, node: &Input<'src, L>) -> Alt<S, E> {
        match node {
            Input::Struct(s) => Alt::new(self.compile(s).into_token_stream()),
            Input::Enum(e) => Alt::new(self.compile(e).into_token_stream()),
        }
    }
}

impl<'src, W, R, L> Compile<'src, Struct<'src, L>, Alt<W, R>> for L
where
    L: Layout + Compile<'src, Struct<'src, L>, W> + Compile<'src, Struct<'src, L>, R>,
    W: ToTokens,
    R: ToTokens,
{
    fn compile(&self, node: &Struct<'src, L>) -> Alt<W, R> {
        if node.is_wrapper() {
            let s: W = self.compile(node);
            Alt::new(s.to_token_stream())
        } else {
            let e: R = self.compile(node);
            Alt::new(e.to_token_stream())
        }
    }
}

impl<'a, 'src, W, R, L> Compile<'src, Variant<'a, 'src, L>, Alt<W, R>> for L
where
    L: Layout + Compile<'src, Variant<'a, 'src, L>, W> + Compile<'src, Variant<'a, 'src, L>, R>,
    W: ToTokens,
    R: ToTokens,
{
    fn compile(&self, node: &Variant<'a, 'src, L>) -> Alt<W, R> {
        if node.is_wrapper() {
            let s: W = self.compile(node);
            Alt::new(s.to_token_stream())
        } else {
            let e: R = self.compile(node);
            Alt::new(e.to_token_stream())
        }
    }
}
