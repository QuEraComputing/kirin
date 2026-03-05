use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Builder for `match` expressions.
///
/// ```ignore
/// let m = MatchExpr {
///     subject: quote!(self),
///     arms: vec![
///         MatchArm {
///             pattern: quote!(Self::Add { .. }),
///             guard: None,
///             body: quote! { true },
///         },
///     ],
/// };
/// // m implements ToTokens → `match self { Self::Add { .. } => { true } }`
/// ```
pub struct MatchExpr {
    /// The expression being matched on.
    pub subject: TokenStream,
    /// Match arms.
    pub arms: Vec<MatchArm>,
}

/// A single arm in a [`MatchExpr`].
pub struct MatchArm {
    /// The pattern to match against.
    pub pattern: TokenStream,
    /// Optional `if` guard expression.
    pub guard: Option<TokenStream>,
    /// The arm body expression.
    pub body: TokenStream,
}

impl ToTokens for MatchExpr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let subject = &self.subject;
        let arms = &self.arms;
        tokens.extend(quote! {
            match #subject {
                #(#arms)*
            }
        });
    }
}

impl ToTokens for MatchArm {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let pattern = &self.pattern;
        let body = &self.body;
        match &self.guard {
            Some(guard) => tokens.extend(quote! { #pattern if #guard => #body, }),
            None => tokens.extend(quote! { #pattern => #body, }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::rustfmt_tokens;

    #[test]
    fn match_expr_simple() {
        let m = MatchExpr {
            subject: quote! { x },
            arms: vec![
                MatchArm {
                    pattern: quote! { 1 },
                    guard: None,
                    body: quote! { "one" },
                },
                MatchArm {
                    pattern: quote! { _ },
                    guard: None,
                    body: quote! { "other" },
                },
            ],
        };

        let output = rustfmt_tokens(&m.to_token_stream());
        assert!(output.contains("match x"));
        assert!(output.contains("\"one\""));
        assert!(output.contains("\"other\""));
    }

    #[test]
    fn match_arm_with_guard() {
        let arm = MatchArm {
            pattern: quote! { x },
            guard: Some(quote! { x > 0 }),
            body: quote! { x },
        };

        let output = arm.to_token_stream().to_string();
        assert!(output.contains("if"));
    }
}
