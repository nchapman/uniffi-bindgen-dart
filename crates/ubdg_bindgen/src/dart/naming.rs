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
