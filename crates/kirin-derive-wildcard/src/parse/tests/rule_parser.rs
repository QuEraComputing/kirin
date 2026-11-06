use syn::parse_quote;

use crate::model::{StmtKind, WalkResult};
use crate::parse::RuleParser;

use super::helpers::{error, kind_name};

// --- check_signature ---

#[test]
fn check_signature_accepts_a_plain_function() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {}
    );
    assert!(RuleParser::check_signature(&item.sig).is_ok());
}

/// A rule can be generic over the language it is built for.
#[test]
fn check_signature_accepts_generics_and_a_where_clause() {
    let item: syn::ItemFn = parse_quote!(
        fn rule<L, V: Clone>()
        where
            L: From<Arith<V>>,
        {
        }
    );
    assert!(RuleParser::check_signature(&item.sig).is_ok());
}

#[test]
fn check_signature_rejects_qualified_functions() {
    let cases: [(syn::ItemFn, &str); 5] = [
        (
            parse_quote!(
                async fn rule() {}
            ),
            "`#[rewrite_rule]` functions cannot be async",
        ),
        (
            parse_quote!(
                const fn rule() {}
            ),
            "`#[rewrite_rule]` functions cannot be const",
        ),
        (
            parse_quote!(
                unsafe fn rule() {}
            ),
            "`#[rewrite_rule]` functions cannot be unsafe",
        ),
        (
            parse_quote!(
                extern "C" fn rule() {}
            ),
            "`#[rewrite_rule]` functions cannot have an ABI",
        ),
        (
            parse_quote!(
                fn rule() -> u32 {}
            ),
            "`#[rewrite_rule]` functions cannot have a return type",
        ),
    ];
    for (item, message) in cases {
        assert_eq!(error(RuleParser::check_signature(&item.sig)), message);
    }
}

#[test]
fn check_signature_rejects_variadic_functions() {
    let item: syn::ItemFn = parse_quote!(
        fn rule(x: i32, ...) {}
    );
    assert_eq!(
        error(RuleParser::check_signature(&item.sig)),
        "`#[rewrite_rule]` functions cannot be variadic"
    );
}

// --- parse ---

#[test]
fn parse_lowers_a_whole_rule() {
    let model = RuleParser::parse(&parse_quote!(
        fn strength_reduction() {
            let c = op![Constant::constant(val)];
            let r = op![Arith::mul(x, c)];
            is_power_of_two(val);
            let shift = shift_amount(val);
            rewrite!(r, {
                let log_val = log2(shift);
                let new_c = op![Constant::constant(log_val)];
                let new_r = op![Bitwise::shl(x, new_c)];
                replace!(r, new_r);
                erase!(r);
            })
        }
    ))
    .unwrap();

    assert_eq!(model.name(), "strength_reduction");
    let kinds: Vec<_> = model
        .body()
        .iter()
        .map(|&id| kind_name(&model, id))
        .collect();
    // Each hole comes just before the statement first using it.
    assert_eq!(
        kinds,
        [
            "hole",
            "op",
            "hole",
            "op",
            "compute",
            "constraint",
            "compute",
            "rewrite"
        ]
    );
    // The replacement ops are in the rewrite, not the match, and the root is
    // the `mul`.
    let StmtKind::Rewrite { root, body, .. } = &model.statement(model.body()[7]).kind else {
        panic!("expected a rewrite");
    };
    assert_eq!(*root, model.body()[3]);
    let kinds: Vec<_> = body.iter().map(|&id| kind_name(&model, id)).collect();
    assert_eq!(kinds, ["compute", "op", "op", "replace", "erase"]);

    // `val` and `x` are holes shared by every use; `c` is the first op's result.
    let StmtKind::Op { args: c_args, .. } = &model.statement(model.body()[1]).kind else {
        panic!("expected an op");
    };
    let StmtKind::Op { args: r_args, .. } = &model.statement(model.body()[3]).kind else {
        panic!("expected an op");
    };
    assert!(model.is_hole(c_args[0].value));
    assert!(model.is_hole(r_args[0].value));
    assert_eq!(model.value(c_args[0].value).stmt, model.body()[0]);
    assert_eq!(model.value(r_args[0].value).stmt, model.body()[2]);
    assert_eq!(r_args[1].value, model.statement(model.body()[1]).results[0]);
}

#[test]
fn parse_accepts_ops_in_the_rewrite_body() {
    let model = RuleParser::parse(&parse_quote!(
        fn strength_reduction() {
            let c = op![Constant::constant(val)];
            let r = op![Arith::mul(x, c)];
            is_power_of_two(val);
            rewrite!(r, {
                let k = log2(val);
                let new_c = op![Constant::constant(k)];
                let new_r = op![Bitwise::shl(x, new_c)];
                replace!(r, new_r);
                erase!(r);
            })
        }
    ))
    .unwrap();

    let rewrite = *model.body().last().unwrap();
    let StmtKind::Rewrite { body, .. } = &model.statement(rewrite).kind else {
        panic!("expected a rewrite");
    };
    let kinds: Vec<_> = body.iter().map(|&id| kind_name(&model, id)).collect();
    assert_eq!(kinds, ["compute", "op", "op", "replace", "erase"]);
}

/// A native call may act on the IR itself, so it is enough for a body.
#[test]
fn parse_accepts_a_body_whose_only_action_is_a_native_call() {
    let model = RuleParser::parse(&parse_quote!(
        fn fold() {
            let r = op![Arith::neg(x)];
            rewrite!(r, {
                let _ = fold_neg(r);
            })
        }
    ))
    .unwrap();

    let (root, body) = model.rewrite();
    assert_eq!(kind_name(&model, root), "op");
    let kinds: Vec<_> = body.iter().map(|&id| kind_name(&model, id)).collect();
    assert_eq!(kinds, ["compute"]);
}

#[test]
fn parse_accepts_a_rule_ending_in_a_lone_action() {
    let model = RuleParser::parse(&parse_quote!(
        fn add_zero() {
            let zero = op![Constant::constant(value)];
            let r = op![Arith::add(x, zero)];
            is_zero(value);
            replace!(r, x)
        }
    ))
    .unwrap();

    let kinds: Vec<_> = model
        .body()
        .iter()
        .map(|&id| kind_name(&model, id))
        .collect();
    assert_eq!(
        kinds,
        [
            "hole",
            "op",
            "hole",
            "op",
            "compute",
            "constraint",
            "rewrite"
        ]
    );
    let StmtKind::Rewrite { root, body, .. } = &model.statement(model.body()[6]).kind else {
        panic!("expected a rewrite");
    };
    assert_eq!(*root, model.body()[3]);
    assert_eq!(body.len(), 1);
}

#[test]
fn parse_rejects_a_rule_without_a_final_rewrite() {
    let cases: [(syn::ItemFn, &str); 5] = [
        (
            parse_quote!(
                fn rule() {}
            ),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::add(x, y)];
                }
            ),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::add(x, y)];
                    replace!(r, x);
                    erase!(r)
                }
            ),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::add(x, y)];
                    rewrite!(r, {});
                }
            ),
            "a `#[rewrite_rule]` function must end with `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`",
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::add(x, y)];
                    rewrite!(r, {});
                    is_zero(r);
                }
            ),
            "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
        ),
    ];
    for (item, message) in cases {
        assert_eq!(error(RuleParser::parse(&item)), message);
    }
}

#[test]
fn parse_checks_the_signature() {
    assert_eq!(
        error(RuleParser::parse(&parse_quote!(
            async fn rule() {}
        ))),
        "`#[rewrite_rule]` functions cannot be async"
    );
}

#[test]
fn parse_rejects_a_hole_no_matched_op_reads() {
    let cases: [(syn::ItemFn, &str); 3] = [
        // Only passed to a native call.
        (
            parse_quote!(
                fn rule() {
                    is_power_of_two(y);
                    let r = op![Arith::neg(x)];
                    replace!(r, x)
                }
            ),
            "y",
        ),
        // First used in the rewrite.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, y);
                    })
                }
            ),
            "y",
        ),
        // `_` in an op of the rewrite.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        let n = op![Arith::neg(_)];
                        replace!(r, n);
                    })
                }
            ),
            "_",
        ),
    ];
    for (item, hole) in cases {
        assert_eq!(
            error(RuleParser::parse(&item)),
            format!(
                "the hole `{hole}` is not an argument of any `op![..]`, so matching will never bind it"
            )
        );
    }
}

#[test]
fn parse_accepts_anonymous_holes_in_matched_ops() {
    let model = RuleParser::parse(&parse_quote!(
        fn rule() {
            let r = op![Arith::add(x, _)];
            replace!(r, x)
        }
    ))
    .unwrap();

    let kinds: Vec<_> = model
        .body()
        .iter()
        .map(|&id| kind_name(&model, id))
        .collect();
    assert_eq!(kinds, ["hole", "hole", "op", "rewrite"]);
}

#[test]
fn parse_rejects_rewrites_that_act_on_unmatched_values() {
    let root =
        "the root of the rewrite must be an `op![..]` matched before it, or one of its results";
    let erase = "`erase!` can only act on an `op![..]` before the rewrite, or one of its results";
    let replace = "`replace!` can only act on the result of an `op![..]` before the rewrite";
    let native = "cannot `replace!` with the result of a native function called before the rewrite: only native functions in the `rewrite!` body can build a replacement";
    let no_action =
        "the body of `rewrite!` must contain an `erase!`, a `replace!` or a native call";
    let cases: [(syn::ItemFn, &str); 9] = [
        // The root is a hole, or a native function's result.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(x, {
                        erase!(r);
                    })
                }
            ),
            root,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    let k = f(x);
                    rewrite!(k, {
                        erase!(r);
                    })
                }
            ),
            root,
        ),
        // `erase!` on a hole, a native result, or an op the rewrite builds.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    erase!(x)
                }
            ),
            erase,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    let k = f(x);
                    erase!(k)
                }
            ),
            erase,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        let n = op![Arith::neg(x)];
                        erase!(n);
                    })
                }
            ),
            erase,
        ),
        // `replace!` on a hole.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    replace!(x, r)
                }
            ),
            replace,
        ),
        // `replace!` with a native result from the match.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    let k = f(x);
                    replace!(r, k)
                }
            ),
            native,
        ),
        // A body that only builds an op.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        let n = op![Arith::neg(x)];
                    })
                }
            ),
            no_action,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {})
                }
            ),
            no_action,
        ),
    ];
    for (item, message) in cases {
        assert_eq!(
            error(RuleParser::parse(&item)),
            message,
            "{}",
            quote::quote!(#item)
        );
    }
}

#[test]
fn parse_accepts_replacing_with_a_hole_a_matched_value_or_a_built_value() {
    for item in [
        // `add_zero`: redirect to whatever the hole matched.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                replace!(r, x)
            }
        ),
        parse_quote!(
            fn rule() {
                let a = op![Arith::neg(x)];
                let r = op![Arith::neg(a)];
                replace!(r, a)
            }
        ),
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    let n = op![Arith::neg(x)];
                    replace!(r, n);
                })
            }
        ),
        // A native function in the rewrite can build the replacement.
        parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    let n = build_neg(x);
                    replace!(r, n);
                })
            }
        ),
    ] {
        let item: syn::ItemFn = item;
        assert!(RuleParser::parse(&item).is_ok(), "{}", quote::quote!(#item));
    }
}

#[test]
fn parse_rejects_a_match_that_is_not_the_roots_producers() {
    let unreachable =
        "this `op![..]` does not feed the root of the rewrite, so it is never matched";
    let native = "a matched `op![..]` cannot take a native function's result as an operand";
    let cases: [(syn::ItemFn, &str); 4] = [
        // An op beside the root's producers.
        (
            parse_quote!(
                fn rule() {
                    let a = op![Arith::neg(x)];
                    let r = op![Arith::neg(y)];
                    replace!(r, y)
                }
            ),
            unreachable,
        ),
        // An op consuming the root: matching only reads the root's producers.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    let u = op![Arith::neg(r)];
                    replace!(r, x)
                }
            ),
            unreachable,
        ),
        // A native result as an operand of a matched op.
        (
            parse_quote!(
                fn rule() {
                    let k = f(x);
                    let r = op![Arith::add(x, k)];
                    replace!(r, x)
                }
            ),
            native,
        ),
        // The same, deeper in the match.
        (
            parse_quote!(
                fn rule() {
                    let k = f(x);
                    let a = op![Arith::add(x, k)];
                    let r = op![Arith::neg(a)];
                    replace!(r, x)
                }
            ),
            native,
        ),
    ];
    for (item, message) in cases {
        assert_eq!(
            error(RuleParser::parse(&item)),
            message,
            "{}",
            quote::quote!(#item)
        );
    }
}

#[test]
fn parse_accepts_operands_shared_in_the_match() {
    let model = RuleParser::parse(&parse_quote!(
        fn rule() {
            let a = op![Arith::neg(x)];
            let b = op![Arith::add(a, a)];
            let r = op![Arith::add(b, b)];
            replace!(r, a)
        }
    ))
    .unwrap();
    let kinds: Vec<_> = model
        .body()
        .iter()
        .map(|&id| kind_name(&model, id))
        .collect();
    assert_eq!(kinds, ["hole", "op", "op", "op", "rewrite"]);
}

/// Every message of a combined error, in order.
fn errors(result: syn::Result<crate::model::RuleModel>) -> Vec<String> {
    result
        .unwrap_err()
        .into_iter()
        .map(|error| error.to_string())
        .collect()
}

#[test]
fn parse_reports_every_unbound_hole_at_once() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            is_power_of_two(a);
            is_power_of_two(b);
            let r = op![Arith::neg(x)];
            replace!(r, x)
        }
    );
    assert_eq!(
        errors(RuleParser::parse(&item)),
        [
            "the hole `a` is not an argument of any `op![..]`, so matching will never bind it",
            "the hole `b` is not an argument of any `op![..]`, so matching will never bind it",
        ]
    );
}

#[test]
fn parse_reports_an_unreachable_op_but_not_the_holes_only_it_reads() {
    let item: syn::ItemFn = parse_quote!(
        fn rule() {
            is_power_of_two(a);
            let unused = op![Arith::neg(z)];
            let r = op![Arith::neg(x)];
            replace!(r, x)
        }
    );
    // `z` is unbound only because its op is unreachable; `a` is a mistake
    // of its own.
    assert_eq!(
        errors(RuleParser::parse(&item)),
        [
            "this `op![..]` does not feed the root of the rewrite, so it is never matched",
            "the hole `a` is not an argument of any `op![..]`, so matching will never bind it",
        ]
    );
}

// --- the walk result: the tail expression of the `rewrite!` body ---

/// The kind of the walk result the rule's `rewrite!` body ends in.
fn walk_result_of(item: syn::ItemFn) -> (crate::model::RuleModel, Option<&'static str>) {
    let model = RuleParser::parse(&item).unwrap();
    let walk_result =
        model
            .walk_result()
            .map(|walk_result| match &model.statement(walk_result).kind {
                StmtKind::WalkResult(WalkResult::Advance) => "advance",
                StmtKind::WalkResult(WalkResult::Skip) => "skip",
                StmtKind::WalkResult(WalkResult::Interrupt) => "interrupt",
                StmtKind::WalkResult(WalkResult::Value(_)) => "value",
                _ => panic!("expected a walk result"),
            });
    (model, walk_result)
}

#[test]
fn parse_accepts_a_fixed_walk_result() {
    for (item, expected) in [
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        advance!()
                    })
                }
            ),
            "advance",
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        skip!()
                    })
                }
            ),
            "skip",
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        interrupt!()
                    })
                }
            ),
            "interrupt",
        ),
    ] {
        let (model, walk_result) = walk_result_of(item);
        assert_eq!(walk_result, Some(expected));
        let (_, body) = model.rewrite();
        let kinds: Vec<_> = body.iter().map(|&id| kind_name(&model, id)).collect();
        assert_eq!(kinds, ["replace"], "the walk result is not an action");
    }
}

#[test]
fn parse_accepts_a_body_without_a_walk_result() {
    let (_, walk_result) = walk_result_of(parse_quote!(
        fn rule() {
            let r = op![Arith::neg(x)];
            rewrite!(r, {
                replace!(r, x);
            })
        }
    ));
    assert_eq!(walk_result, None);
}

/// A native call as the tail runs as the body's last action, and the walk
/// reads its one result.
#[test]
fn parse_accepts_a_native_call_as_the_walk_result() {
    let (model, walk_result) = walk_result_of(parse_quote!(
        fn rule() {
            let r = op![Arith::neg(x)];
            rewrite!(r, {
                replace!(r, x);
                decide(x)
            })
        }
    ));
    assert_eq!(walk_result, Some("value"));
    let (_, body) = model.rewrite();
    let kinds: Vec<_> = body.iter().map(|&id| kind_name(&model, id)).collect();
    assert_eq!(kinds, ["replace", "compute"]);
    let StmtKind::WalkResult(WalkResult::Value(value)) =
        &model.statement(model.walk_result().unwrap()).kind
    else {
        panic!("expected a walk result value");
    };
    assert_eq!(model.value(value.value).stmt, body[1]);
}

#[test]
fn parse_accepts_a_native_result_bound_in_the_body_as_the_walk_result() {
    let (model, walk_result) = walk_result_of(parse_quote!(
        fn rule() {
            let r = op![Arith::neg(x)];
            rewrite!(r, {
                let w = decide(x);
                replace!(r, x);
                w
            })
        }
    ));
    assert_eq!(walk_result, Some("value"));
    let StmtKind::WalkResult(WalkResult::Value(value)) =
        &model.statement(model.walk_result().unwrap()).kind
    else {
        panic!("expected a walk result value");
    };
    let (_, body) = model.rewrite();
    assert_eq!(model.value(value.value).stmt, body[0]);
}

#[test]
fn parse_rejects_misplaced_or_malformed_walk_results() {
    let misplaced =
        "`skip!()` can only be the last expression of the body of `rewrite!`, without a semicolon";
    let not_native = "the walk result must be computed by a native function in the `rewrite!` body";
    let no_action =
        "the body of `rewrite!` must contain an `erase!`, a `replace!` or a native call";
    let cases: [(syn::ItemFn, &str); 8] = [
        // With a semicolon, or before another statement.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        skip!();
                    })
                }
            ),
            misplaced,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        skip!();
                        replace!(r, x);
                    })
                }
            ),
            misplaced,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        skip!(r)
                    })
                }
            ),
            "`skip!` takes no arguments",
        ),
        // A value no native function in the body computed: a native result
        // from the match, or a matched op's result.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    let w = decide(x);
                    rewrite!(r, {
                        replace!(r, x);
                        w
                    })
                }
            ),
            not_native,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        erase!(r);
                        x
                    })
                }
            ),
            not_native,
        ),
        // Any other expression.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, {
                        replace!(r, x);
                        true
                    })
                }
            ),
            "the last expression of the body of `rewrite!` must be `advance!()`, `skip!()`, `interrupt!()`, a native call or the name of a native call's result",
        ),
        // A walk result alone changes nothing, even from a native call.
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, { skip!() })
                }
            ),
            no_action,
        ),
        (
            parse_quote!(
                fn rule() {
                    let r = op![Arith::neg(x)];
                    rewrite!(r, { decide(r) })
                }
            ),
            no_action,
        ),
    ];
    for (item, message) in cases {
        assert_eq!(error(RuleParser::parse(&item)), message);
    }
}

#[test]
fn parse_rejects_a_walk_result_reading_an_erased_value() {
    assert_eq!(
        error(RuleParser::parse(&parse_quote!(
            fn rule() {
                let r = op![Arith::neg(x)];
                rewrite!(r, {
                    replace!(r, x);
                    decide(r)
                })
            }
        ))),
        "`r` was redirected by `replace!` above and cannot be used afterwards"
    );
}
