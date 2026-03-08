//! Miscellaneous utilities: case conversion, type inspection, attribute parsing.

/// Dumps token stream to stderr when `KIRIN_EXPAND_DEBUG` is set.
pub fn debug_dump(tokens: &proc_macro2::TokenStream) {
    if std::env::var("KIRIN_EXPAND_DEBUG").is_ok() {
        eprintln!("{}", tokens);
    }
}

/// Extracts the last segment of a path as an `Ident`.
pub fn strip_path(path: &syn::Path) -> syn::Ident {
    path.segments
        .last()
        .expect("matching_type_path must have at least one segment")
        .ident
        .clone()
}

/// Parses a string into any `syn::Parse` type.
pub fn from_str<T: syn::parse::Parse>(s: impl Into<String>) -> T {
    syn::parse_str(&s.into()).unwrap()
}

/// Converts a string to CamelCase.
pub fn to_camel_case(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Converts a string to snake_case.
pub fn to_snake_case(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                result.push('_');
            }
            for lower_c in c.to_lowercase() {
                result.push(lower_c);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Checks if a type's last path segment matches the given name.
pub fn is_type<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
{
    if let syn::Type::Path(syn::TypePath { path, .. }) = ty
        && let Some(seg) = path.segments.last()
    {
        return seg.ident == *name;
    }
    false
}

/// Checks if a type is `Vec<T>` where `T` matches the given name.
pub fn is_vec_type<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I> + PartialEq<str>,
{
    is_type_in(ty, name, |seg| seg.ident == "Vec")
}

/// Checks if a type appears as a generic argument of another type.
pub fn is_type_in_generic<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
{
    is_type_in(ty, name, |_| true)
}

/// Checks if a type matches with a custom segment predicate.
pub fn is_type_in<I, F>(ty: &syn::Type, name: &I, f: F) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
    F: Fn(&syn::PathSegment) -> bool,
{
    if let syn::Type::Path(type_path) = ty
        && let Some(seg) = type_path.path.segments.last()
        && f(seg)
        && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
    {
        for each in &args.args {
            if let syn::GenericArgument::Type(inner_ty) = &each
                && is_type(inner_ty, name)
            {
                return true;
            }
        }
    }
    false
}

/// Parses a named attribute's nested meta items.
pub fn parse_attribute(
    name: &str,
    attrs: &[syn::Attribute],
    logic: impl FnMut(syn::meta::ParseNestedMeta) -> syn::Result<()>,
) -> syn::Result<()> {
    for attr in attrs {
        if attr.path().is_ident(name) {
            return attr.parse_nested_meta(logic);
        }
    }
    Ok(())
}

/// Creates an "unknown attribute" error for use in attribute parsers.
pub fn error_unknown_attribute(meta: &syn::meta::ParseNestedMeta) -> syn::Error {
    if ["crate_path", "type"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        meta.error(format!(
            "the '{}' attribute is only allowed on the type level #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ))
    } else if meta.path.is_ident("callable") {
        meta.error(
            "the 'callable' attribute is not part of #[kirin(...)]; use #[callable] with #[derive(CallSemantics)]",
        )
    } else if [
        "constant",
        "pure",
        "speculatable",
        "terminator",
        "fn",
        "text",
    ]
    .iter()
    .any(|name| meta.path.is_ident(name))
    {
        meta.error(format!(
            "the '{}' attribute is only allowed on the per statement #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ))
    } else if ["into", "default", "type"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        meta.error(format!(
            "the '{}' attribute is only allowed on fields inside statements",
            meta.path.get_ident().unwrap()
        ))
    } else {
        meta.error(format!(
            "unknown attribute '{}' for #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("my_struct_name"), "MyStructName");
        assert_eq!(to_camel_case("another_example"), "AnotherExample");
        assert_eq!(to_camel_case("simple"), "Simple");
        assert_eq!(
            to_camel_case("with__double__underscores"),
            "WithDoubleUnderscores"
        );
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("MyStructName"), "my_struct_name");
        assert_eq!(to_snake_case("AnotherExample"), "another_example");
        assert_eq!(to_snake_case("Simple"), "simple");
        assert_eq!(
            to_snake_case("WithDoubleUnderscores"),
            "with_double_underscores"
        );
    }

    #[test]
    fn test_is_type() {
        let ty: syn::Type = syn::parse_str("String").unwrap();
        assert!(is_type(&ty, "String"));
        assert!(!is_type(&ty, "i32"));
    }

    #[test]
    fn test_is_type_phantom() {
        let ty: syn::Type = syn::parse_str("std::marker::PhantomData<T>").unwrap();
        assert!(is_type(&ty, "PhantomData"));
    }

    #[test]
    fn test_is_vec_type() {
        let ty: syn::Type = syn::parse_str("Vec<String>").unwrap();
        assert!(is_vec_type(&ty, "String"));
        assert!(!is_vec_type(&ty, "i32"));
    }

    #[test]
    fn test_is_type_in_generic() {
        let ty: syn::Type = syn::parse_str("Option<i32>").unwrap();
        assert!(is_type_in_generic(&ty, "i32"));
        assert!(!is_type_in_generic(&ty, "String"));
    }

    #[test]
    fn test_is_type_in() {
        let ty: syn::Type = syn::parse_str("Result<String, i32>").unwrap();
        assert!(is_type_in(&ty, "String", |seg| seg.ident == "Result"));
        assert!(!is_type_in(&ty, "f64", |seg| seg.ident == "Result"));
    }

    #[test]
    fn test_to_camel_case_empty_string() {
        assert_eq!(to_camel_case(""), "");
    }

    #[test]
    fn test_to_camel_case_leading_underscore() {
        assert_eq!(to_camel_case("_leading"), "Leading");
    }

    #[test]
    fn test_to_camel_case_trailing_underscore() {
        assert_eq!(to_camel_case("trailing_"), "Trailing");
    }

    #[test]
    fn test_to_camel_case_single_char() {
        assert_eq!(to_camel_case("a"), "A");
    }

    #[test]
    fn test_to_snake_case_empty_string() {
        assert_eq!(to_snake_case(""), "");
    }

    #[test]
    fn test_to_snake_case_all_uppercase() {
        assert_eq!(to_snake_case("ABC"), "a_b_c");
    }

    #[test]
    fn test_to_snake_case_single_char() {
        assert_eq!(to_snake_case("X"), "x");
    }

    #[test]
    fn test_to_snake_case_already_snake() {
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn test_is_type_non_path() {
        // A reference type should not match
        let ty: syn::Type = syn::parse_str("&str").unwrap();
        assert!(!is_type(&ty, "str"));
    }

    #[test]
    fn test_is_type_empty_path() {
        // A tuple type should not match anything
        let ty: syn::Type = syn::parse_str("(i32, i32)").unwrap();
        assert!(!is_type(&ty, "i32"));
    }

    #[test]
    fn test_is_vec_type_not_vec() {
        let ty: syn::Type = syn::parse_str("Option<String>").unwrap();
        assert!(!is_vec_type(&ty, "String"));
    }

    #[test]
    fn test_is_type_in_generic_bare_type() {
        // Bare type without generics should not match
        let ty: syn::Type = syn::parse_str("i32").unwrap();
        assert!(!is_type_in_generic(&ty, "i32"));
    }

    #[test]
    fn test_is_type_in_no_type_arg() {
        // A type with a lifetime argument, not a type argument
        let ty: syn::Type = syn::parse_str("Cow<'static, str>").unwrap();
        // "str" as an inner type: this should match because Cow<'static, str>
        // has GenericArgument::Type(str) as its second arg
        assert!(is_type_in_generic(&ty, "str"));
    }
}
