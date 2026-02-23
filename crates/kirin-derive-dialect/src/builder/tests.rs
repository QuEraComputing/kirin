use super::DeriveBuilder;
use kirin_test_utils::rustfmt;

fn derive_builder(input: &syn::DeriveInput) -> String {
    let mut builder = DeriveBuilder::default();
    rustfmt(builder.emit(input).unwrap().to_string())
}

macro_rules! case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_builder(&input)
    }};
}

#[test]
fn test_regular_named_struct() {
    insta::assert_snapshot!(case! {
        #[kirin(constant, fn = new, type = L)]
        pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
            #[kirin(into)]
            pub value: T,
            #[kirin(type = value.type_of())]
            pub result: ResultValue,
            #[kirin(default = std::marker::PhantomData)]
            pub marker: std::marker::PhantomData<L>,
        }
    });
}

#[test]
fn test_regular_struct_with_ssa() {
    insta::assert_snapshot!(case! {
        #[kirin(fn = new, type = L)]
        struct TestStruct<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
            #[kirin(into)]
            value: T,
            #[kirin(type = value.type_of())]
            result: ResultValue,
            #[kirin(into)]
            input_ssa: SSAValue,
            #[kirin(default = std::marker::PhantomData)]
            marker: std::marker::PhantomData<L>,
        }
    });
}

#[test]
fn test_regular_unnamed_struct() {
    insta::assert_snapshot!(case! {
        #[kirin(constant, fn = op_constant, type = L)]
        struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice>(
            #[kirin(into)]
            T,
            #[kirin(type = value.type_of())]
            ResultValue,
            #[kirin(default = std::marker::PhantomData)]
            std::marker::PhantomData<L>,
        );
    });
}

#[test]
fn test_regular_enum() {
    insta::assert_snapshot!(case! {
        #[kirin(fn, type = SomeType)]
        enum TestEnum {
            A {
                #[kirin(into)]
                value: u32,
                #[kirin(type = u32_type)]
                result: ResultValue,
            },
            B(
                #[kirin(into)]
                u64,
                #[kirin(type = u64_type)]
                ResultValue,
            ),
        }
    });
}

#[test]
fn test_wrapper_enum() {
    insta::assert_snapshot!(case! {
        #[wraps]
        #[kirin(fn, type = SomeType)]
        enum WrapperEnum {
            A(InnerA),
            B(InnerB),
        }
    });
}

#[test]
fn test_wrapper_enum_generic() {
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
fn test_either_enum() {
    insta::assert_snapshot!(case! {
        #[kirin(fn, type = SomeType)]
        enum EitherEnum {
            #[kirin(fn = op_abc)]
            A {
                #[kirin(into)]
                value: u32,
                #[kirin(type = u32_type)]
                result: ResultValue,
            },
            #[wraps]
            B(InnerB),
        }
    });
}

#[test]
fn test_multi_results_struct() {
    insta::assert_snapshot!(case! {
        #[kirin(fn, type = L)]
        struct MultiResult<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
            #[kirin(into)]
            value: T,
            #[kirin(type = value.type_of())]
            result1: ResultValue,
            #[kirin(type = value.type_of())]
            result2: ResultValue,
            #[kirin(default = std::marker::PhantomData)]
            marker: std::marker::PhantomData<L>,
        }
    });
}

#[test]
fn test_multi_results_struct_disabled() {
    insta::assert_snapshot!(case! {
        #[kirin(type = L)]
        struct MultiResult<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
            #[kirin(into)]
            value: T,
            #[kirin(type = value.type_of())]
            result1: ResultValue,
            #[kirin(type = value.type_of())]
            result2: ResultValue,
            #[kirin(default = std::marker::PhantomData)]
            marker: std::marker::PhantomData<L>,
        }
    });
}

#[test]
fn test_scf() {
    insta::assert_snapshot!(case! {
        #[kirin(type = T)]
        pub enum StructuredControlFlow {
            If {
                condition: SSAValue,
                then_block: Block,
                else_block: Block,
            },
            Loop {
                body_block: Block,
                exit_block: Block,
            },
        }
    });
}

#[test]
fn test_cf() {
    insta::assert_snapshot!(case! {
        #[kirin(terminator, fn, type = T)]
        pub enum ControlFlow<T: TypeLattice> {
            #[kirin(format = "br {target}")]
            Branch { target: Successor },
            #[kirin(format = "cond_br {condition} then={true_target} else={false_target}")]
            ConditionalBranch {
                condition: SSAValue,
                true_target: Successor,
                false_target: Successor,
                #[kirin(default = std::marker::PhantomData)]
                marker: std::marker::PhantomData<T>,
            },
            #[kirin(format = "ret {0}")]
            Return(SSAValue),
        }
    });
}

#[test]
fn test_simple() {
    insta::assert_snapshot!(case! {
        #[kirin(fn, type = SimpleType, crate = kirin_ir)]
        pub enum SimpleLanguage {
            Add(
                SSAValue,
                SSAValue,
                #[kirin(type = SimpleType::Float)] ResultValue,
            ),
            Constant(
                #[kirin(into)] Value,
                #[kirin(type = SimpleType::Float)] ResultValue,
            ),
            #[kirin(terminator)]
            Return(SSAValue),
            Function(
                Region,
                #[kirin(type = SimpleType::Float)] ResultValue,
            ),
        }
    });
}

#[test]
fn test_constant_2() {
    insta::assert_snapshot!(case! {
        #[kirin(constant, fn = new, type = L)]
        pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
            #[kirin(into)]
            pub value: T,
            #[kirin(type = value.type_of())]
            pub result: ResultValue,
            #[kirin(default = std::marker::PhantomData)]
            pub marker: std::marker::PhantomData<L>,
        }
    });
}
