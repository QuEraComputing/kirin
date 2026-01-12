use std::process::{Command, Stdio};

use super::{DeriveProperty, PropertyKind};

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

fn derive_constant(input: &syn::DeriveInput) -> String {
    let mut tokens = proc_macro2::TokenStream::new();
    let mut derive = DeriveProperty::new(
        PropertyKind::Constant,
        "::kirin::ir",
        "IsConstant",
        "is_constant",
        "bool",
    );
    tokens.extend(derive.emit(input).unwrap());
    rustfmt(tokens.to_string())
}

macro_rules! case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_constant(&input)
    }};
}

#[test]
fn test_struct_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(constant, type_lattice = Lattice)]
        struct MyStruct {
            a: i32,
            b: i32,
        }
    });
}

#[test]
fn test_struct_wrapper() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = Lattice)]
        struct Wrapper<T> {
            #[wraps]
            inner: InnerStruct<T>,
        }
    });
}

#[test]
fn test_enum_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = Lattice)]
        enum MyEnum<T> {
            VariantA { a: i32, b: T },
            #[kirin(constant)]
            VariantB(i32, T),
        }
    });
}

#[test]
fn test_enum_wrapper() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = Lattice, constant)]
        #[wraps]
        enum MyEnum<T> {
            VariantA { inner: InnerStructA<T> },
            VariantB(InnerStructB),
        }
    });
}

#[test]
fn test_enum_mixed() {
    insta::assert_snapshot!(case! {
        #[kirin(type_lattice = Lattice)]
        enum MyEnum<T> {
            VariantA { #[wraps] inner: InnerStructA<T> },
            #[wraps]
            VariantB(InnerStructB),
            VariantC { a: i32, b: T },
            #[kirin(constant)]
            VariantD(i32, T),
        }
    });
}
