mod attrs;
pub mod fields;
mod input;
mod layout;
mod statement;

pub use attrs::{BuilderOptions, DefaultValue};
pub use input::{Data, DataEnum, DataStruct, Input};
pub use layout::{Layout, StandardLayout};
pub use statement::Statement;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_struct_basic() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(crate = "my_crate", type_lattice = MyLattice)]
            struct MyAST {
                #[kirin(type = "CustomType")]
                field1: SSAValue,
                #[kirin(type = "CustomType")]
                field2: Vec<SSAValue>,
                #[kirin(type = "CustomType")]
                field3: Option<SSAValue>,
                #[kirin(type = "ResultType")]
                field4: ResultValue,
                #[kirin(type = "ResultType")]
                field5: Vec<ResultValue>,
                #[kirin(type = "ResultType")]
                field6: Option<ResultValue>,
                #[kirin(type = "BlockType")]
                field8: Block,
                #[kirin(type = "BlockType")]
                field9: Vec<Block>,
                #[kirin(type = "BlockType")]
                field10: Option<Block>,
                #[kirin(type = "SuccessorType")]
                field11: Successor,
                #[kirin(type = "SuccessorType")]
                field12: Vec<Successor>,
                #[kirin(type = "SuccessorType")]
                field13: Option<Successor>,
                #[kirin(type = "RegionType")]
                field14: Region,
                #[kirin(type = "RegionType")]
                field15: Vec<Region>,
                #[kirin(type = "RegionType")]
                field16: Option<Region>,
                field7: String,
            }
        };

        let ast: Input<StandardLayout> = Input::from_derive_input(&input).unwrap();
        insta::assert_debug_snapshot!(ast);
    }

    #[test]
    fn test_input_enum_basic() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(crate = "my_crate", type_lattice = MyLattice)]
            enum MyEnumAST {
                VariantA {
                    #[kirin(type = "CustomType")]
                    field1: SSAValue,
                },
                VariantB {
                    #[kirin(type = "ResultType")]
                    field2: ResultValue,
                },
            }
        };
        let ast: Input<StandardLayout> = Input::from_derive_input(&input).unwrap();
        insta::assert_debug_snapshot!(ast);
    }

    #[test]
    fn test_input_enum_global_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[wraps]
            #[kirin(crate = "my_crate", type_lattice = MyLattice)]
            enum MyEnumAST {
                VariantA(AnotherA),
                VariantB(AnotherB),
            }
        };
        let ast: Input<StandardLayout> = Input::from_derive_input(&input).unwrap();
        insta::assert_debug_snapshot!(ast);
    }

    #[test]
    fn test_input_enum_variant_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(crate = "my_crate", type_lattice = MyLattice)]
            enum MyEnumAST {
                #[wraps]
                VariantA(AnotherA),
                VariantB {
                    #[kirin(type = "ResultType")]
                    field2: ResultValue,
                },
            }
        };
        let ast: Input<StandardLayout> = Input::from_derive_input(&input).unwrap();
        insta::assert_debug_snapshot!(ast);
    }

    #[test]
    fn test_input_enum_variant_field_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(crate = "my_crate", type_lattice = MyLattice)]
            enum MyEnumAST {
                VariantA(#[wraps] AnotherA, String),
                VariantB {
                    #[kirin(type = "ResultType")]
                    field2: ResultValue,
                },
            }
        };
        let ast: Input<StandardLayout> = Input::from_derive_input(&input).unwrap();
        insta::assert_debug_snapshot!(ast);
    }
}
