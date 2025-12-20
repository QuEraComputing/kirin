mod build_fn;
mod build_result;
mod context;
mod enum_impl;
mod initialization;
mod input;
mod name;
mod from;
mod result;
mod struct_impl;

pub use context::Builder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn test_regular_named_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(constant, fn = new, type_lattice = L)]
            pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
                #[kirin(into)]
                pub value: T,
                #[kirin(type = value.type_of())]
                pub result: ResultValue,
                #[kirin(default = std::marker::PhantomData)]
                pub marker: std::marker::PhantomData<L>,
            }
        };

        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_regular_struct_with_ssa() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(fn = new, type_lattice = L)]
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
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_regular_unnamed_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(constant, fn = op_constant, type_lattice = L)]
            struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice>(
                #[kirin(into)]
                T,
                #[kirin(type = value.type_of())]
                ResultValue,
                #[kirin(default = std::marker::PhantomData)]
                std::marker::PhantomData<L>,
            );
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_regular_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(fn, type_lattice = SomeLattice)]
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
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_wrapper_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            #[kirin(fn, type_lattice = SomeLattice)]
            enum WrapperEnum {
                A(InnerA),
                B(InnerB),
            }
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_wrapper_enum_generic() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            #[kirin(fn, type_lattice = T)]
            pub enum StructuredControlFlow<T: TypeLattice> {
                If(If<T>),
                For(For<T>),
            }
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_either_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(fn, type_lattice = SomeLattice)]
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
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_multi_results_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(fn, type_lattice = L)]
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
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_multi_results_struct_disabled() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = L)]
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
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }

    #[test]
    fn test_scf() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = T)]
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
        };
        insta::assert_snapshot!(Builder::default().print(&input));
    }
}
