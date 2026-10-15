//! The checks that keep a rewrite from using what it has already redirected
//! or erased, one test per check.

use syn::parse_quote;

use crate::parse::RuleParser;

use super::helpers::error;

/// The error `RuleParser::parse` reports for `item`.
fn rejection(item: syn::ItemFn) -> String {
    error(RuleParser::parse(&item))
}

#[test]
fn a_replaced_value_is_never_used_again() {
    let dead = "`r` was redirected by `replace!` above and cannot be used afterwards";
    let cases: [syn::ItemFn; 4] = [
        // As an operand of an op the rewrite builds.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    replace!(r, x);
                    let n = op![Arith::neg(r)];
                })
            }
        ),
        // As an argument of a native call.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    replace!(r, x);
                    let n = build(r);
                })
            }
        ),
        // As a replacement.
        parse_quote!(
            fn rule() {
                let a = op![Arith::neg(x)];
                let r = op![Arith::neg(a)];
                rewrite!(r, {
                    replace!(r, x);
                    replace!(a, r);
                })
            }
        ),
        // Replaced a second time.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    replace!(r, x);
                    replace!(r, x);
                })
            }
        ),
    ];
    for item in cases {
        assert_eq!(rejection(item), dead);
    }
}

#[test]
fn replacing_along_a_chain_is_accepted() {
    // `b` takes over `a`'s uses, then hands them to `x` before it dies.
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let a = op![Arith::neg(x)];
            let b = op![Arith::neg(a)];
            let r = op![Arith::add(a, b)];
            rewrite!(r, {
                replace!(a, b);
                replace!(b, x);
                replace!(r, x);
            })
        }
    );
    assert!(RuleParser::parse(&item).is_ok());
}

/// An erase need not replace the op's results first: a rule can instead
/// check that they are unused, as dead-code elimination does. If a result is
/// still used when the erase runs, the rewrite fails and the pass
/// quarantines the stage.
#[test]
fn erase_does_not_need_the_results_replaced() {
    for item in [
        // Dead-code elimination: a constraint guards that the result is unused.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                has_no_uses(r);
                erase!(r)
            }
        ),
        // Every result replaced first.
        parse_quote!(
            fn rule() {
                let dm @ (q, rem) = op![Arith::divmod(x, y)];
                rewrite!(dm, {
                    replace!(q, x);
                    replace!(rem, y);
                    erase!(dm);
                })
            }
        ),
        // Only some of them, or one that is `_` and cannot be replaced.
        parse_quote!(
            fn rule() {
                let dm @ (q, rem) = op![Arith::divmod(x, y)];
                rewrite!(dm, {
                    replace!(q, x);
                    erase!(dm);
                })
            }
        ),
        parse_quote!(
            fn rule() {
                let dm @ (q, _) = op![Arith::divmod(x, y)];
                rewrite!(dm, {
                    replace!(q, x);
                    erase!(dm);
                })
            }
        ),
        // Through one of its results instead of the op's name.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    replace!(r, x);
                    erase!(r);
                })
            }
        ),
        // An op without results.
        parse_quote!(
            fn rule() {
                let k @ () = op![Sink::sink(x)];
                erase!(k)
            }
        ),
    ] {
        let item: syn::ItemFn = item;
        assert!(RuleParser::parse(&item).is_ok(), "{}", quote::quote!(#item));
    }
}

#[test]
fn erase_takes_the_op_and_its_results_out_of_scope() {
    let erased_result = |name: &str| {
        format!(
            "`{name}` is a result of an op erased by `erase!` above and cannot be used afterwards"
        )
    };
    let cases: [(syn::ItemFn, String); 4] = [
        // The op cannot be erased again.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        erase!(r);
                        erase!(r);
                    })
                }
            ),
            "the op this refers to was already erased by `erase!` above".to_string(),
        ),
        // A result cannot be replaced after its op is gone.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        erase!(r);
                        replace!(r, x);
                    })
                }
            ),
            erased_result("r"),
        ),
        // Nor can a sibling be used, replaced or not.
        (
            parse_quote!(
                fn rule() {
                    let dm @ (q, rem) = op![Arith::divmod(x, y)];
                    rewrite!(dm, {
                        replace!(q, x);
                        erase!(dm);
                        let n = op![Arith::neg(rem)];
                    })
                }
            ),
            erased_result("rem"),
        ),
        (
            parse_quote!(
                fn rule() {
                    let dm @ (q, rem) = op![Arith::divmod(x, y)];
                    rewrite!(dm, {
                        replace!(q, x);
                        replace!(rem, y);
                        erase!(dm);
                        let n = build(q);
                    })
                }
            ),
            erased_result("q"),
        ),
    ];
    for (item, message) in cases {
        assert_eq!(rejection(item), message);
    }
}

/// Liveness is checked once the whole rule is parsed, so every use of
/// something removed is reported at once, each at that use.
#[test]
fn every_use_of_something_removed_is_reported_at_once() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            let dm @ (q, rem) = op![Arith::divmod(x, y)];
            rewrite!(dm, {
                replace!(q, x);
                let n = op![Arith::add(q, rem)];
                erase!(dm);
                replace!(rem, n);
                erase!(dm);
            })
        }
    );
    let messages: Vec<String> = RuleParser::parse(&item)
        .unwrap_err()
        .into_iter()
        .map(|error| error.to_string())
        .collect();
    assert_eq!(
        messages,
        [
            "`q` was redirected by `replace!` above and cannot be used afterwards",
            "`rem` is a result of an op erased by `erase!` above and cannot be used afterwards",
            "the op this refers to was already erased by `erase!` above",
        ]
    );
}

#[test]
fn replace_cannot_redirect_a_value_to_itself() {
    assert_eq!(
        rejection(parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                replace!(r, r)
            }
        )),
        "`replace!` cannot redirect a value to itself"
    );
}

#[test]
fn a_built_op_cannot_take_a_native_result_from_the_match() {
    assert_eq!(
        rejection(parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                let k = log2(x);
                rewrite!(r, {
                    let n = op![Arith::neg(k)];
                    replace!(r, n);
                })
            }
        )),
        "an `op![..]` cannot take the result of a native function called before the rewrite: only native functions in the `rewrite!` body can build IR"
    );
}
