//! Code generation for the frame-injection derives: `#[derive(FrameBuild)]`,
//! `#[derive(AbstractFrameBuild)]`, `#[derive(DenseFrameBuild)]`.
//!
//! A language that runs the interpreter declares a **total frame enum** — the
//! one Rust type the driver's `Vec<F>` stack holds. Each framework walker it
//! carries must be injectable into that enum, which is what the `*FrameBuild`
//! traits are for. The impls are pure transcription: one constructor per
//! framework frame, each body `Self::Variant(frame)`.
//!
//! These derives write that transcription. The **derive name selects the
//! family** — `FrameBuild` (concrete), `AbstractFrameBuild` (sparse forward),
//! `DenseFrameBuild` (dense backward) — so no attribute is needed, and the name
//! matches the trait it implements as the other interpreter derives do.
//!
//! Variants are matched to constructors by their **field type**, not their
//! variant name, so renaming a variant cannot silently change what is
//! generated. Variants holding a *dialect* frame (`ScfIfFrame`, …) are ignored:
//! those are injected through the dialect's own `Build*` trait, declared by the
//! dialect and implemented by hand.
//!
//! Not derivable, by design: an enum that supplies a callable-`UnGraph` policy.
//! `FrameBuild::from_ungraph_entry` is a defaulted method and a derive emits the
//! whole impl block, so such an enum keeps its hand-written impl (see
//! `UnPolicyFrame` in the workspace `body_kinds` test).

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter";

/// One constructor of an injection trait.
struct Ctor {
    /// The framework frame type this constructor accepts, matched against a
    /// variant's field type by its final path segment.
    frame: &'static str,
    /// The trait method to generate.
    method: &'static str,
    /// Whether the trait requires it. Optional constructors have a defaulted
    /// implementation in the trait and are simply omitted when no variant
    /// carries the frame.
    required: bool,
    /// Whether the method returns `Result<Self, E>` rather than `Self` — the
    /// trait lets these refuse, so the generated body wraps in `Ok`.
    fallible: bool,
}

/// A frame family: its injection trait, that trait's arity, and its
/// constructors.
pub struct Family {
    trait_name: &'static str,
    /// Number of type parameters the trait takes, which must equal the number
    /// the deriving enum declares.
    arity: usize,
    ctors: &'static [Ctor],
}

pub const CONCRETE: Family = Family {
    trait_name: "FrameBuild",
    arity: 2,
    ctors: &[
        Ctor {
            frame: "BlockFrame",
            method: "from_block",
            required: true,
            fallible: false,
        },
        Ctor {
            frame: "CFGFrame",
            method: "from_cfg",
            required: true,
            fallible: false,
        },
        Ctor {
            frame: "CallFrame",
            method: "from_call",
            required: true,
            fallible: false,
        },
        Ctor {
            frame: "DiGraphFrame",
            method: "from_digraph",
            required: true,
            fallible: false,
        },
    ],
};

pub const SPARSE_FORWARD: Family = Family {
    trait_name: "AbstractFrameBuild",
    arity: 3,
    ctors: &[
        Ctor {
            frame: "AbstractBlockFrame",
            method: "from_block",
            required: true,
            fallible: false,
        },
        Ctor {
            frame: "AbstractCallFrame",
            method: "from_call",
            required: true,
            fallible: false,
        },
        // Graph bodies are opt-in: the trait's default refuses, so an enum
        // without this variant simply inherits the refusal.
        Ctor {
            frame: "AbstractDiGraphFrame",
            method: "from_digraph",
            required: false,
            fallible: true,
        },
    ],
};

pub const DENSE_BACKWARD: Family = Family {
    trait_name: "DenseFrameBuild",
    arity: 2,
    ctors: &[Ctor {
        frame: "DenseBlockFrame",
        method: "from_block",
        required: true,
        fallible: false,
    }],
};

/// Reads the `#[interpret(crate = ...)]` override, reusing the namespace the
/// other interpreter derives already use.
fn parse_interp_crate_path(input: &DeriveInput) -> syn::Result<syn::Path> {
    let mut crate_path = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("interpret") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                crate_path = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported attribute for #[interpret(...)]"))
            }
        })?;
    }
    match crate_path {
        Some(path) => Ok(path),
        None => syn::parse_str(DEFAULT_INTERP_CRATE),
    }
}

/// The final path segment of a type, used to recognize a framework frame.
fn type_head(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

pub fn generate(input: &DeriveInput, family: &Family) -> syn::Result<TokenStream> {
    let trait_ident: syn::Ident = syn::parse_str(family.trait_name)?;
    let interp_crate = parse_interp_crate_path(input)?;

    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            format!(
                "`{}` can only be derived for a total frame enum",
                family.trait_name
            ),
        ));
    };

    // The trait's type arguments are the enum's own type parameters, in order —
    // `enum ToyFrame<V, E>` implements `FrameBuild<V, E>`. Reject a mismatch
    // here rather than emitting an impl that fails to resolve later.
    let type_params: Vec<&syn::Ident> = input.generics.type_params().map(|p| &p.ident).collect();
    if type_params.len() != family.arity {
        return Err(syn::Error::new_spanned(
            input,
            format!(
                "`{}` expects an enum with {} type parameter(s) to match `{}<{}>`, found {}. \
                 An enum whose trait arguments are not its own type parameters must implement the trait by hand.",
                family.trait_name,
                family.arity,
                family.trait_name,
                vec!["_"; family.arity].join(", "),
                type_params.len(),
            ),
        ));
    }

    // Match each variant to a constructor by its field type.
    let mut methods = Vec::new();
    for ctor in family.ctors {
        let mut matched = None;
        for variant in &data.variants {
            let syn::Fields::Unnamed(fields) = &variant.fields else {
                continue;
            };
            if fields.unnamed.len() != 1 {
                continue;
            }
            let field_ty = &fields.unnamed[0].ty;
            if type_head(field_ty).as_deref() == Some(ctor.frame) {
                if matched.is_some() {
                    return Err(syn::Error::new_spanned(
                        variant,
                        format!(
                            "two variants hold a `{}`; `{}::{}` would be ambiguous",
                            ctor.frame, family.trait_name, ctor.method
                        ),
                    ));
                }
                matched = Some((&variant.ident, field_ty));
            }
        }

        let Some((variant_ident, field_ty)) = matched else {
            if ctor.required {
                return Err(syn::Error::new_spanned(
                    input,
                    format!(
                        "no variant holds a `{}`, which `{}` requires for `{}`. \
                         Add such a variant, or implement the trait by hand.",
                        ctor.frame, family.trait_name, ctor.method
                    ),
                ));
            }
            continue;
        };

        let method: syn::Ident = syn::parse_str(ctor.method)?;
        // The parameter type is copied verbatim from the variant, so the frame
        // type's own generic arity never has to be reconstructed here.
        methods.push(if ctor.fallible {
            let err = type_params[1];
            quote! {
                fn #method(frame: #field_ty) -> ::core::result::Result<Self, #err> {
                    ::core::result::Result::Ok(Self::#variant_ident(frame))
                }
            }
        } else {
            quote! {
                fn #method(frame: #field_ty) -> Self {
                    Self::#variant_ident(frame)
                }
            }
        });
    }

    let enum_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::#trait_ident<#(#type_params),*>
            for #enum_ident #ty_generics #where_clause
        {
            #(#methods)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn emit(input: syn::DeriveInput, family: &Family) -> String {
        rustfmt(
            generate(&input, family)
                .expect("codegen failed")
                .to_string(),
        )
    }

    #[test]
    fn concrete_frame_enum_with_dialect_variants() {
        // The two scf variants are ignored — they are injected through
        // `BuildScfIf`/`BuildScfFor`, which the dialect declares.
        let input: syn::DeriveInput = syn::parse_quote! {
            enum ToyFrame<V, E> {
                Block(BlockFrame<V, E>),
                CFG(CFGFrame<V, E>),
                Call(CallFrame<V>),
                DiGraph(DiGraphFrame<V, E>),
                ScfIf(ScfIfFrame<V, E>),
                ScfFor(ScfForFrame<V, E>),
            }
        };
        insta::assert_snapshot!(emit(input, &CONCRETE));
    }

    #[test]
    fn sparse_forward_omits_optional_digraph_when_absent() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum ToyAbstractFrame<V, E, K> {
                Block(AbstractBlockFrame<V, E, K>),
                Call(AbstractCallFrame<V, E, K>),
                ScfIf(AbstractScfIfFrame<V, E, K>),
            }
        };
        insta::assert_snapshot!(emit(input, &SPARSE_FORWARD));
    }

    #[test]
    fn sparse_forward_digraph_is_fallible_when_present() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum StandardAbstractFrame<V, E, K> {
                Block(AbstractBlockFrame<V, E, K>),
                Call(AbstractCallFrame<V, E, K>),
                DiGraph(AbstractDiGraphFrame<V, E, K>),
            }
        };
        insta::assert_snapshot!(emit(input, &SPARSE_FORWARD));
    }

    #[test]
    fn dense_backward_single_constructor() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum ToyDenseBackwardFrame<V, E> {
                Block(DenseBlockFrame<V, E>),
                ScfIf(DenseScfIfFrame<V, E>),
            }
        };
        insta::assert_snapshot!(emit(input, &DENSE_BACKWARD));
    }

    #[test]
    fn crate_path_override_is_honoured() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[interpret(crate = crate)]
            enum StandardFrame<V, E> {
                Block(BlockFrame<V, E>),
                CFG(CFGFrame<V, E>),
                Call(CallFrame<V>),
                DiGraph(DiGraphFrame<V, E>),
            }
        };
        insta::assert_snapshot!(emit(input, &CONCRETE));
    }

    #[test]
    fn rejects_missing_required_frame() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum Incomplete<V, E> {
                Block(BlockFrame<V, E>),
            }
        };
        let err = generate(&input, &CONCRETE).unwrap_err().to_string();
        assert!(err.contains("no variant holds a `CFGFrame`"), "{err}");
    }

    #[test]
    fn rejects_wrong_type_param_count() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum Weird<V> {
                Block(BlockFrame<V, V>),
                CFG(CFGFrame<V, V>),
                Call(CallFrame<V>),
                DiGraph(DiGraphFrame<V, V>),
            }
        };
        let err = generate(&input, &CONCRETE).unwrap_err().to_string();
        assert!(
            err.contains("expects an enum with 2 type parameter(s)"),
            "{err}"
        );
    }

    #[test]
    fn rejects_ambiguous_duplicate_frame() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum Dup<V, E> {
                Block(BlockFrame<V, E>),
                AlsoBlock(BlockFrame<V, E>),
                CFG(CFGFrame<V, E>),
                Call(CallFrame<V>),
                DiGraph(DiGraphFrame<V, E>),
            }
        };
        let err = generate(&input, &CONCRETE).unwrap_err().to_string();
        assert!(err.contains("would be ambiguous"), "{err}");
    }

    #[test]
    fn rejects_non_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct NotAnEnum<V, E>(BlockFrame<V, E>);
        };
        let err = generate(&input, &CONCRETE).unwrap_err().to_string();
        assert!(err.contains("total frame enum"), "{err}");
    }
}
