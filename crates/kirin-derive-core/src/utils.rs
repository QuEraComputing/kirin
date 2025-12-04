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
