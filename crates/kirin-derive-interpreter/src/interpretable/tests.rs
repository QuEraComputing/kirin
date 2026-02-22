use super::DeriveInterpretable;
use kirin_test_utils::rustfmt;

fn derive_interpretable(input: &syn::DeriveInput) -> String {
    let mut derive = DeriveInterpretable::default();
    let tokens = derive.emit(input).unwrap();
    rustfmt(tokens.to_string())
}

fn derive_interpretable_err(input: &syn::DeriveInput) -> String {
    let mut derive = DeriveInterpretable::default();
    let err = derive.emit(input).unwrap_err();
    err.to_string()
}

macro_rules! case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_interpretable(&input)
    }};
}

macro_rules! err_case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_interpretable_err(&input)
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
fn test_non_wrapper_struct_error() {
    insta::assert_snapshot!(err_case! {
        #[kirin(type = TestType)]
        struct Regular {
            a: i32,
            b: i32,
        }
    });
}

#[test]
fn test_mixed_enum_error() {
    insta::assert_snapshot!(err_case! {
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
