use crate::ir::*;

pub trait Compile<'src, Context: Layout, T> {
    fn compile(&self, ctx: &Context) -> T;
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

impl<'src, L, S, E, Ctx> Compile<'src, Ctx, Alt<S, E>> for Input<'src, L>
where
    L: Layout + 'src,
    S: ToTokens,
    E: ToTokens,
    Ctx: Layout,
    Struct<'src, L>: Compile<'src, Ctx, S>,
    Enum<'src, L>: Compile<'src, Ctx, E>,
{
    fn compile(&self, ctx: &Ctx) -> Alt<S, E> {
        match self {
            Input::Struct(s) => Alt::new(s.compile(ctx).into_token_stream()),
            Input::Enum(e) => Alt::new(e.compile(ctx).into_token_stream()),
        }
    }
}

impl<'src, W, R, L> Compile<'src, L, Alt<W, R>> for Struct<'src, L>
where
    L: Layout,
    W: ToTokens,
    R: ToTokens,
    Struct<'src, L>: Compile<'src, L, W> + Compile<'src, L, R>,
{
    fn compile(&self, ctx: &L) -> Alt<W, R> {
        if self.is_wrapper() {
            let s: W = self.compile(ctx);
            Alt::new(s.to_token_stream())
        } else {
            let e: R = self.compile(ctx);
            Alt::new(e.to_token_stream())
        }
    }
}

impl<'a, 'src, W, R, L> Compile<'src, L, Alt<W, R>> for Variant<'a, 'src, L>
where
    L: Layout,
    W: ToTokens,
    R: ToTokens,
    Variant<'a, 'src, L>: Compile<'src, L, W> + Compile<'src, L, R>,
{
    fn compile(&self, ctx: &L) -> Alt<W, R> {
        if self.is_wrapper() {
            let s: W = self.compile(ctx);
            Alt::new(s.to_token_stream())
        } else {
            let e: R = self.compile(ctx);
            Alt::new(e.to_token_stream())
        }
    }
}
