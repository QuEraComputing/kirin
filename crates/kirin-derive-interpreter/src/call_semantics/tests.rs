use super::DeriveCallSemantics;
use kirin_test_utils::rustfmt;

fn derive_call_semantics(input: &syn::DeriveInput) -> String {
    let mut derive = DeriveCallSemantics::default();
    let tokens = derive.emit(input).unwrap();
    rustfmt(tokens.to_string())
}

macro_rules! case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_call_semantics(&input)
    }};
}

#[test]
fn test_all_wrapper_enum() {
    insta::assert_snapshot!(case! {
        #[kirin(type = ArithType)]
        #[wraps]
        enum TestDialect {
            Arith(Arith<ArithType>),
            ControlFlow(ControlFlow<ArithType>),
            Constant(Constant<ArithValue, ArithType>),
        }
    });
}

#[test]
fn test_mixed_enum() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        enum TestDialect {
            #[wraps]
            Arith(Arith<TestType>),
            #[wraps]
            ControlFlow(ControlFlow<TestType>),
            Custom { a: i32, b: i32 },
        }
    });
}

#[test]
fn test_generic_wrapper_enum() {
    insta::assert_snapshot!(case! {
        #[kirin(type = T)]
        #[wraps]
        enum TestDialect<T> {
            Arith(Arith<T>),
            ControlFlow(ControlFlow<T>),
        }
    });
}

#[test]
fn test_wrapper_struct() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        struct Wrapper {
            #[wraps]
            inner: InnerType,
        }
    });
}

#[test]
fn test_non_wrapper_struct() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        struct Regular {
            a: i32,
            b: i32,
        }
    });
}

#[test]
fn test_callable_per_variant() {
    // #[wraps] on all, #[callable] only on FunctionBody → only that one forwards
    insta::assert_snapshot!(case! {
        #[kirin(type = ArithType)]
        #[wraps]
        enum TestDialect {
            Arith(Arith<ArithType>),
            ControlFlow(ControlFlow<ArithType>),
            #[callable]
            FunctionBody(FunctionBody<ArithType>),
        }
    });
}

#[test]
fn test_callable_global() {
    // #[callable] on enum + #[wraps] on all → all forward
    insta::assert_snapshot!(case! {
        #[kirin(type = ArithType)]
        #[wraps]
        #[callable]
        enum TestDialect {
            Arith(Arith<ArithType>),
            ControlFlow(ControlFlow<ArithType>),
            FunctionBody(FunctionBody<ArithType>),
        }
    });
}

#[test]
fn test_callable_global_mixed_wraps() {
    // #[callable] on enum, #[wraps] only on some → only #[wraps] variants forward
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        #[callable]
        enum TestDialect {
            #[wraps]
            Arith(Arith<TestType>),
            ControlFlow(ControlFlow<TestType>),
            #[wraps]
            FunctionBody(FunctionBody<TestType>),
        }
    });
}
