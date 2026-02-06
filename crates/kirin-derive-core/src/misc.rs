pub fn strip_path(path: &syn::Path) -> syn::Ident {
    path.segments
        .last()
        .expect("matching_type_path must have at least one segment")
        .ident
        .clone()
}

pub fn from_str<T: syn::parse::Parse>(s: impl Into<String>) -> T {
    syn::parse_str(&s.into()).unwrap()
}

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

pub fn is_type<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
{
    if let syn::Type::Path(syn::TypePath { path, .. }) = ty {
        if let Some(seg) = path.segments.last() {
            return seg.ident == *name;
        }
    }
    false
}

pub fn is_vec_type<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I> + PartialEq<str>,
{
    is_type_in(ty, name, |seg| seg.ident == "Vec")
}

pub fn is_type_in_generic<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
{
    is_type_in(ty, name, |_| true)
}

/// check if the type `ty` is a generic type containing `name` as one of its generic arguments
/// for which the predicate `f` on the generic type's last path segment holds
///
/// # Example
///
/// ```
/// use kirin_derive_core::misc::is_type_in;
/// let expr = syn::parse_str::<syn::Type>("Result<String, i32>").unwrap();
/// assert!(is_type_in(&expr, "String", |seg| seg.ident == "Result"));
/// ```
pub fn is_type_in<I, F>(ty: &syn::Type, name: &I, f: F) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
    F: Fn(&syn::PathSegment) -> bool,
{
    if let syn::Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if f(seg) {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    for each in &args.args {
                        if let syn::GenericArgument::Type(inner_ty) = &each {
                            if is_type(inner_ty, name) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

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

pub fn error_unknown_attribute(meta: &syn::meta::ParseNestedMeta) -> syn::Error {
    if ["crate_path", "type"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        return meta.error(format!(
            "the '{}' attribute is only allowed on the type level #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ));
    } else if ["constant", "pure", "terminator", "fn", "text"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        return meta.error(format!(
            "the '{}' attribute is only allowed on the per statement #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ));
    } else if ["into", "default", "type"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        return meta.error(format!(
            "the '{}' attribute is only allowed on fields inside statements",
            meta.path.get_ident().unwrap()
        ));
    } else {
        return meta.error(format!(
            "unknown attribute '{}' for #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ));
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
}
