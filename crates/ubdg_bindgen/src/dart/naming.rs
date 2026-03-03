pub(super) fn to_upper_camel(input: &str) -> String {
    let mut out = String::new();
    for segment in input.split(|c: char| !c.is_ascii_alphanumeric()) {
        if segment.is_empty() {
            continue;
        }
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for c in chars {
                out.push(c);
            }
        }
    }
    if out.is_empty() {
        "Uniffi".to_string()
    } else {
        out
    }
}

pub(super) fn dart_identifier(input: &str) -> String {
    let parts = input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "uniffi_bindings".to_string()
    } else {
        parts.join("_")
    }
}

pub(super) fn to_lower_camel(input: &str) -> String {
    let mut out = String::new();
    for (i, segment) in input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .enumerate()
    {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.push(first.to_ascii_lowercase());
            } else {
                out.push(first.to_ascii_uppercase());
            }
            for c in chars {
                out.push(c);
            }
        }
    }
    if out.is_empty() {
        "value".to_string()
    } else {
        out
    }
}

pub(super) fn safe_dart_identifier(input: &str) -> String {
    if is_dart_keyword(input) {
        format!("{input}_")
    } else {
        input.to_string()
    }
}

pub(super) fn uniffi_trait_method_kind(name: &str) -> Option<&'static str> {
    let normalized = name.replace('_', "").to_ascii_lowercase();
    match normalized.as_str() {
        "uniffitraitdisplay" => Some("display"),
        "uniffitraitdebug" => Some("debug"),
        "uniffitraithash" => Some("hash"),
        "uniffitraiteq" | "uniffitraiteqeq" => Some("eq"),
        "uniffitraitne" => Some("ne"),
        "uniffitraitordcmp" => Some("ord_cmp"),
        _ => None,
    }
}

pub(super) fn is_uniffi_trait_method_name(name: &str) -> bool {
    uniffi_trait_method_kind(name).is_some()
}

pub(super) fn is_dart_keyword(input: &str) -> bool {
    matches!(
        input,
        "abstract"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "base"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "covariant"
            | "default"
            | "deferred"
            | "do"
            | "dynamic"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "extension"
            | "external"
            | "factory"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "Function"
            | "get"
            | "hide"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "interface"
            | "is"
            | "late"
            | "library"
            | "mixin"
            | "new"
            | "null"
            | "on"
            | "operator"
            | "part"
            | "required"
            | "rethrow"
            | "return"
            | "sealed"
            | "set"
            | "show"
            | "static"
            | "super"
            | "switch"
            | "sync"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typedef"
            | "var"
            | "void"
            | "when"
            | "while"
            | "with"
            | "yield"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // to_upper_camel tests
    #[test]
    fn upper_camel_snake_case() {
        assert_eq!(to_upper_camel("simple_fn"), "SimpleFn");
    }

    #[test]
    fn upper_camel_kebab_case() {
        assert_eq!(to_upper_camel("hello-world"), "HelloWorld");
    }

    #[test]
    fn upper_camel_empty() {
        assert_eq!(to_upper_camel(""), "Uniffi");
    }

    #[test]
    fn upper_camel_single_word() {
        assert_eq!(to_upper_camel("already"), "Already");
    }

    #[test]
    fn upper_camel_all_caps() {
        assert_eq!(to_upper_camel("ALL_CAPS"), "ALLCAPS");
    }

    #[test]
    fn upper_camel_single_chars() {
        assert_eq!(to_upper_camel("a_b_c"), "ABC");
    }

    // to_lower_camel tests
    #[test]
    fn lower_camel_snake_case() {
        assert_eq!(to_lower_camel("simple_fn"), "simpleFn");
    }

    #[test]
    fn lower_camel_kebab_case() {
        assert_eq!(to_lower_camel("hello-world"), "helloWorld");
    }

    #[test]
    fn lower_camel_empty() {
        assert_eq!(to_lower_camel(""), "value");
    }

    #[test]
    fn lower_camel_already_camel() {
        assert_eq!(to_lower_camel("CamelCase"), "camelCase");
    }

    // dart_identifier tests
    #[test]
    fn dart_identifier_kebab() {
        assert_eq!(dart_identifier("hello-world"), "hello_world");
    }

    #[test]
    fn dart_identifier_upper() {
        assert_eq!(dart_identifier("MyClass"), "myclass");
    }

    #[test]
    fn dart_identifier_empty() {
        assert_eq!(dart_identifier(""), "uniffi_bindings");
    }

    #[test]
    fn dart_identifier_single_char() {
        assert_eq!(dart_identifier("a"), "a");
    }

    // safe_dart_identifier tests
    #[test]
    fn safe_identifier_keyword() {
        assert_eq!(safe_dart_identifier("class"), "class_");
        assert_eq!(safe_dart_identifier("void"), "void_");
    }

    #[test]
    fn safe_identifier_non_keyword() {
        assert_eq!(safe_dart_identifier("name"), "name");
        assert_eq!(safe_dart_identifier("myVar"), "myVar");
    }

    // is_dart_keyword tests
    #[test]
    fn keyword_detection() {
        assert!(is_dart_keyword("class"));
        assert!(is_dart_keyword("abstract"));
        assert!(is_dart_keyword("yield"));
        assert!(is_dart_keyword("Function"));
        assert!(!is_dart_keyword("myVar"));
    }

    // uniffi_trait_method_kind tests
    #[test]
    fn trait_method_kinds() {
        assert_eq!(
            uniffi_trait_method_kind("uniffi_trait_display"),
            Some("display")
        );
        assert_eq!(
            uniffi_trait_method_kind("uniffi_trait_debug"),
            Some("debug")
        );
        assert_eq!(uniffi_trait_method_kind("uniffi_trait_hash"), Some("hash"));
        assert_eq!(uniffi_trait_method_kind("uniffi_trait_eq"), Some("eq"));
        assert_eq!(uniffi_trait_method_kind("random_name"), None);
    }

    // is_uniffi_trait_method_name tests
    #[test]
    fn trait_method_name_detection() {
        assert!(is_uniffi_trait_method_name("uniffi_trait_display"));
        assert!(!is_uniffi_trait_method_name("some_other_method"));
    }
}
