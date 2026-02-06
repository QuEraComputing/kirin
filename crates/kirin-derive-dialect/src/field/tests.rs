use std::process::{Command, Stdio};

use super::{DeriveFieldIter, FieldIterKind};

fn rustfmt<S: ToString>(src: S) -> String {
    let mut child = Command::new("rustfmt")
        .arg("--emit")
        .arg("stdout")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(src.to_string().as_bytes())
            .unwrap();
    }

    let output = child.wait_with_output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

fn derive_fields(input: &syn::DeriveInput) -> String {
    let mut tokens = proc_macro2::TokenStream::new();
    let mut args = DeriveFieldIter::new(
        FieldIterKind::Arguments,
        false,
        "kirin::ir",
        "HasArguments",
        "SSAValue",
        "arguments",
        "Iter",
    );
    let mut args_mut = DeriveFieldIter::new(
        FieldIterKind::Arguments,
        true,
        "kirin::ir",
        "HasArgumentsMut",
        "SSAValue",
        "arguments_mut",
        "IterMut",
    );
    tokens.extend(args.emit(input).unwrap());
    tokens.extend(args_mut.emit(input).unwrap());
    rustfmt(tokens.to_string())
}

macro_rules! case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_fields(&input)
    }};
}

#[test]
fn test_enum_either() {
    insta::assert_snapshot!(case! {
        #[kirin(type = SomeType)]
        enum TestEnum<T> {
            VariantA { #[wraps] wrapped: InnerStructA<T> },
            #[wraps]
            VariantB(InnerStructB<T>),
            VariantC { a: SSAValue, b: T, c: SSAValue },
            VariantD(SSAValue, f64, SSAValue),
        }
    });
}

#[test]
fn test_enum_global_wrapper() {
    insta::assert_snapshot!(case! {
        #[wraps]
        #[kirin(type = AnotherType)]
        enum TestEnum<T> {
            VariantA { wrapped: InnerStructA<T> },
            VariantB(InnerStructB),
        }
    });
}

#[test]
fn test_enum_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type = RegularType)]
        enum TestEnum<T> {
            VariantA { a: SSAValue, b: T, c: SSAValue },
            VariantB(SSAValue, f64, SSAValue),
        }
    });
}

#[test]
fn test_enum_arith() {
    insta::assert_snapshot!(case! {
        #[kirin(type = ArithType)]
        pub enum ArithInstruction<T> {
            Add(SSAValue, Vec<SSAValue>, ResultValue, T),
            Sub(SSAValue, Vec<SSAValue>, ResultValue, T),
            Mul(SSAValue, Vec<SSAValue>, ResultValue),
            Div(SSAValue, Vec<SSAValue>, ResultValue),
        }
    });
}

#[test]
fn test_enum_named() {
    insta::assert_snapshot!(case! {
        #[kirin(type = CFlowType)]
        pub enum ControlFlowInstruction {
            Branch {
                target: Block,
            },
            ConditionalBranch {
                condition: SSAValue,
                true_target: Block,
                false_target: Block,
            },
            Return(SSAValue),
        }
    });
}

#[test]
fn test_enum_wraps() {
    insta::assert_snapshot!(case! {
        #[wraps]
        #[kirin(fn, type = T)]
        pub enum StructuredControlFlow<T: TypeLattice> {
            If(If<T>),
            For(For<T>),
        }
    });
}

#[test]
fn test_struct_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type = T)]
        struct TestStruct<T> {
            a: SSAValue,
            b: f64,
            c: T,
        }
    })
}

#[test]
fn test_struct_vec() {
    insta::assert_snapshot!(case! {
        #[kirin(type = T)]
        struct TestStruct<T> {
            a: SSAValue,
            b: SSAValue,
            c: Vec<SSAValue>,
            d: T,
        }
    })
}

#[test]
fn test_struct_named_wrapper() {
    insta::assert_snapshot!(case! {
        #[wraps]
        #[kirin(fn, type = T)]
        struct NamedWrapper<T> {
            wrapped: InnerStruct<T>,
        }
    })
}

#[test]
fn test_struct_wrapper_iter_uses_lifetime() {
    let generated = case! {
        #[wraps]
        #[kirin(fn, type = T)]
        struct NamedWrapper<T> {
            wrapped: InnerStruct<T>,
        }
    };
    assert!(
        generated.contains("HasArguments<'a>") && generated.contains("HasArgumentsMut<'a>"),
        "wrapper iter types must include trait lifetime:\n{}",
        generated
    );
}

#[test]
fn test_struct_unnamed_wrapper() {
    insta::assert_snapshot!(case! {
        #[kirin(type = T)]
        struct TestStruct<T>(SSAValue, #[wraps] T, SSAValue, String, f64);
    })
}

#[test]
fn test_struct_unnamed_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type = T)]
        struct TestStruct<T>(SSAValue, T, SSAValue, String, f64);
    })
}

#[test]
fn test_simple() {
    insta::assert_snapshot!(case! {
        #[kirin(fn, type = SimpleIRType, crate = kirin_ir)]
        pub enum SimpleLanguage {
            Add(
                SSAValue,
                SSAValue,
                #[kirin(type = SimpleIRType::Float)] ResultValue,
            ),
            Constant(
                #[kirin(into)] Value,
                #[kirin(type = SimpleIRType::Float)] ResultValue,
            ),
            #[kirin(terminator)]
            Return(SSAValue),
            Function(
                Region,
                #[kirin(type = SimpleIRType::Float)] ResultValue,
            ),
        }
    });
}
