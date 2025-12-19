use crate::kirin::field::context::FieldsIter;
use crate::prelude::*;

pub fn test_ctx(
    mutable: bool,
    trait_name: &str,
    trait_method: &str,
    trait_type_iter: &str,
) -> FieldsIter {
    FieldsIter::builder()
        .mutable(mutable)
        .trait_lifetime("'a")
        .matching_type("SSAValue")
        .default_crate_path("kirin::ir")
        .trait_path(trait_name)
        .trait_method(trait_method)
        .trait_type_iter(trait_type_iter)
        .build()
}

macro_rules! case {
    ($($tt:tt)*) => {{
            let input: syn::DeriveInput = syn::parse_quote! {
                $($tt)*
            };
            test_ctx(
                false,
                "HasArguments",
                "arguments",
                "Iter"
            )
            .print(&input)
            + &test_ctx(
                true,
                "HasArgumentsMut",
                "arguments_mut",
                "IterMut"
            ).print(&input)
    }};
}

#[test]
fn test_enum_either() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = SomeLattice)]
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
        #[kirin(type_lattice = AnotherLattice)]
        enum TestEnum<T> {
            VariantA { wrapped: InnerStructA<T> },
            VariantB(InnerStructB),
        }
    });
}

#[test]
fn test_enum_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = RegularLattice)]
        enum TestEnum<T> {
            VariantA { a: SSAValue, b: T, c: SSAValue },
            VariantB(SSAValue, f64, SSAValue),
        }
    });
}

#[test]
fn test_enum_arith() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = ArithLattice)]
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
        #[kirin(type_lattice = CFlowLattice)]
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
        #[kirin(fn, type_lattice = T)]
        pub enum StructuredControlFlow<T: TypeLattice> {
            If(If<T>),
            For(For<T>),
        }
    });
}

#[test]
fn test_struct_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = T)]
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
        #[kirin(type_lattice = T)]
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
        #[kirin(fn, type_lattice = T)]
        struct NamedWrapper<T> {
            wrapped: InnerStruct<T>,
        }
    })
}

#[test]
fn test_struct_unnamed_wrapper() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = T)]
        struct TestStruct<T>(SSAValue, #[wraps] T, SSAValue, String, f64);
    })
}

#[test]
fn test_struct_unnamed_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = T)]
        struct TestStruct<T>(SSAValue, T, SSAValue, String, f64);
    })
}
