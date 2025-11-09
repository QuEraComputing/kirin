use super::*;
use crate::DeriveContext;
use proc_macro2::TokenStream;
use quote::quote;

#[test]
fn test_struct_derivation() {
    let input = quote! {
        #[derive(Instruction)]
        #[kirin(is_terminator = true, is_constant = true, is_pure = true)]
        struct TestInst {
            a: SSAValue,
            b: SSAValue,
            c: ResultValue,
            d: Block,
            e: Region,
        }
    };
    insta::assert_snapshot!(check(input));
}

#[test]
fn test_enum_derivation() {
    let input = quote! {
        #[derive(Instruction)]
        enum TestInst {
            #[kirin(is_terminator = true)]
            Terminate {
                target: Block,
            },
            #[kirin(is_constant = true)]
            Constant {
                value: i32,
            },
            #[kirin(is_pure = true)]
            PureOp {
                arg1: SSAValue,
                arg2: SSAValue,
                result: ResultValue,
            },
        }
    };
    insta::assert_snapshot!(check(input));
}

#[test]
fn test_global_enum_derivation() {
    let input = quote! {
        #[derive(Instruction)]
        #[kirin(is_pure = true)]
        enum TestInst {
            #[kirin(is_terminator = true)]
            Terminate {
                target: Block,
            },
            #[kirin(is_constant = true)]
            Constant {
                value: i32,
            },
            #[kirin(is_pure = false)]
            NotPureOp {
                arg1: SSAValue,
                arg2: SSAValue,
                result: ResultValue,
            },
        }
    };
    insta::assert_snapshot!(check(input));
}

#[test]
fn test_global_struct_wrapper() {
    let input = quote! {
        #[derive(Instruction)]
        #[kirin(wraps)]
        struct WrapperInst(TestInst);
    };
    insta::assert_snapshot!(check(input));
}

#[test]
fn test_global_enum_wrapper() {
    let input = quote! {
        #[derive(Instruction)]
        #[kirin(wraps)]
        enum WrapperInst {
            WrapA(TestInstA),
            WrapB(TestInstB),
        }
    };
    insta::assert_snapshot!(check(input));
}

#[test]
fn test_enum_wrapper() {
    let input = quote! {
        #[derive(Instruction)]
        enum TestInst {
            #[kirin(wraps)]
            WrapA(TestInstA),
            #[kirin(wraps)]
            WrapB(TestInstB),
            #[kirin(is_pure = true)]
            Regular {
                arg: SSAValue,
                result: ResultValue,
            },
        }
    };
    insta::assert_snapshot!(check(input));
}

#[test]
#[should_panic]
fn test_vec_resultvalue_field() {
    let input = quote! {
        #[derive(Instruction)]
        struct TestInst {
            args: Vec<SSAValue>,
            results: Vec<ResultValue>,
        }
    };
    insta::assert_snapshot!(check(input));
}

#[test]
#[should_panic]
fn test_enum_vec() {
    let input = quote! {
        #[derive(Instruction)]
        enum TestInst {
            OpA {
                args: Vec<SSAValue>,
                results: Vec<ResultValue>,
            },
            OpB {
                arg: SSAValue,
                result: ResultValue,
            },
        }
    };
    insta::assert_snapshot!(check(input));
}

// test panic on vec<ResultValue> fields
#[test]
#[should_panic]
fn test_scf() {
    let input = quote! {
        pub enum SCFInstruction {
            If {
                condition: SSAValue,
                then_block: Block,
                else_block: Block,
                results: Vec<ResultValue>,
            },
            For {
                lower_bound: SSAValue,
                upper_bound: SSAValue,
                step: SSAValue,
                body_block: Block,
                results: Vec<ResultValue>,
            },
        }
    };
    check(input);
}

fn check(src: TokenStream) -> String {
    let input = syn::parse2(src).unwrap();
    let mut ctx = DeriveContext::new(quote! {::kirin_ir::Instruction}, input);
    let mut pass = DeriveInstruction::new(&ctx);
    pass.generate(&mut ctx).unwrap();
    rustfmt(ctx.generate().to_string().as_str())
}

use std::process::{Command, Stdio};

fn rustfmt(src: &str) -> String {
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
            .write_all(src.as_bytes())
            .unwrap();
    }

    let output = child.wait_with_output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}
