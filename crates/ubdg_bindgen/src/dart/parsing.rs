use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use uniffi_bindgen::interface::{AsType, ComponentInterface, Method, Type, UniffiTraitMethods};

use super::*;

fn is_udl(source: &Path) -> bool {
    matches!(
        source.extension().and_then(|e| e.to_str()),
        Some(ext) if ext.eq_ignore_ascii_case("udl")
    )
}

pub(super) fn namespace_from_source(source: &Path) -> Result<String> {
    if let Some(namespace) = extract_namespace_from_udl(source) {
        return Ok(namespace);
    }

    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("source path must have a valid UTF-8 file stem"))?;
    if stem.trim().is_empty() {
        bail!("source path stem cannot be empty");
    }
    Ok(stem)
}

pub(super) fn extract_namespace_from_udl(source: &Path) -> Option<String> {
    if !is_udl(source) {
        return None;
    }

    let udl = fs::read_to_string(source).ok()?;
    let marker = "namespace";
    let start = udl.find(marker)?;
    let mut chars = udl[start + marker.len()..].chars().peekable();

    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }

    let mut ns = String::new();
    while matches!(chars.peek(), Some(c) if c.is_ascii_alphanumeric() || *c == '_') {
        ns.push(chars.next()?);
    }

    if ns.is_empty() {
        None
    } else {
        Some(ns)
    }
}

pub(super) fn parse_udl_metadata(source: &Path, crate_name: Option<&str>) -> Result<UdlMetadata> {
    if !is_udl(source) {
        // Auto-detect: non-.udl files are treated as compiled cdylibs (library mode),
        // matching upstream UniFFI behavior where mode is inferred from file extension.
        let source_str = source
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("library source path must be valid UTF-8"))?;
        let source_utf8 = Utf8Path::new(source_str);
        let cis = uniffi_bindgen::library_mode::find_cis(
            source_utf8,
            &uniffi_bindgen::EmptyCrateConfigSupplier,
        )
        .with_context(|| format!("failed to parse library metadata: {}", source.display()))?;
        let ci = if let Some(crate_name) = crate_name {
            cis.into_iter()
                .find(|ci| ci.crate_name() == crate_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("crate '{crate_name}' not found in library metadata")
                })?
        } else {
            cis.into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("no UniFFI components found in library metadata"))?
        };
        return component_interface_to_metadata(ci, None, true);
    }

    let udl = fs::read_to_string(source)
        .with_context(|| format!("failed to read UDL source: {}", source.display()))?;
    let module_path = crate_name.unwrap_or("crate_name");
    let ci = ComponentInterface::from_webidl(&udl, module_path)
        .with_context(|| format!("failed to parse UDL: {}", source.display()))?;
    component_interface_to_metadata(ci, Some(&udl), false)
}

pub(super) fn component_interface_to_metadata(
    ci: ComponentInterface,
    udl_source: Option<&str>,
    include_ffi_symbols: bool,
) -> Result<UdlMetadata> {
    let udl_interface_traits = udl_source
        .map(parse_udl_interface_traits)
        .unwrap_or_default();
    let udl_namespace_function_docstrings = udl_source
        .map(parse_udl_namespace_function_docstrings)
        .unwrap_or_default();

    let mut functions = ci
        .function_definitions()
        .iter()
        .map(|f| UdlFunction {
            name: f.name().to_string(),
            ffi_symbol: include_ffi_symbols.then(|| f.ffi_func().name().to_string()),
            ffi_arg_types: if include_ffi_symbols {
                f.ffi_func().arguments().iter().map(|a| a.type_()).collect()
            } else {
                Vec::new()
            },
            ffi_return_type: include_ffi_symbols
                .then(|| f.ffi_func().return_type().cloned())
                .flatten(),
            ffi_has_rust_call_status: if include_ffi_symbols {
                f.ffi_func().has_rust_call_status_arg()
            } else {
                false
            },
            runtime_unsupported: include_ffi_symbols
                .then(|| runtime_unsupported_reason_for_ffi_func(f.ffi_func()))
                .flatten(),
            docstring: f.docstring().map(ToString::to_string),
            is_async: f.is_async(),
            return_type: f.return_type().cloned(),
            throws_type: f.throws_type().cloned(),
            args: f
                .arguments()
                .into_iter()
                .map(|a| UdlArg {
                    name: a.name().to_string(),
                    type_: a.as_type(),
                    docstring: None,
                    default: a.default_value().cloned(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    for function in &mut functions {
        if function.docstring.is_none() {
            function.docstring = udl_namespace_function_docstrings
                .get(&function.name)
                .cloned();
        }
    }
    let records = ci
        .record_definitions()
        .iter()
        .map(|record| {
            let methods: Vec<UdlObjectMethod> = record
                .methods()
                .iter()
                .map(|m| ci_method_to_udl(m, include_ffi_symbols))
                .collect();

            let ci_trait_methods = record.uniffi_trait_methods();
            let mut trait_methods = UdlObjectTraitMethods::default();
            extract_record_enum_trait_methods(&ci_trait_methods, &mut trait_methods);

            let traits = udl_interface_traits
                .get(record.name())
                .cloned()
                .unwrap_or_default();

            UdlRecord {
                name: record.name().to_string(),
                docstring: record.docstring().map(ToString::to_string),
                fields: record
                    .fields()
                    .iter()
                    .map(|field| UdlArg {
                        name: field.name().to_string(),
                        type_: field.as_type(),
                        docstring: field.docstring().map(ToString::to_string),
                        default: field.default_value().cloned(),
                    })
                    .collect(),
                traits,
                methods,
                trait_methods,
            }
        })
        .collect::<Vec<_>>();
    let enums = ci
        .enum_definitions()
        .iter()
        .map(|enum_| {
            let has_discr = enum_.variant_discr_type().is_some();
            let methods: Vec<UdlObjectMethod> = enum_
                .methods()
                .iter()
                .map(|m| ci_method_to_udl(m, include_ffi_symbols))
                .collect();

            let ci_trait_methods = enum_.uniffi_trait_methods();
            let mut trait_methods = UdlObjectTraitMethods::default();
            extract_record_enum_trait_methods(&ci_trait_methods, &mut trait_methods);

            let traits = udl_interface_traits
                .get(enum_.name())
                .cloned()
                .unwrap_or_default();

            UdlEnum {
                name: enum_.name().to_string(),
                docstring: enum_.docstring().map(ToString::to_string),
                is_error: ci.is_name_used_as_error(enum_.name()),
                is_non_exhaustive: enum_.is_non_exhaustive(),
                has_discr_type: has_discr,
                traits,
                variants: enum_
                    .variants()
                    .iter()
                    .enumerate()
                    .map(|(i, variant)| UdlEnumVariant {
                        name: variant.name().to_string(),
                        docstring: variant.docstring().map(ToString::to_string),
                        fields: variant
                            .fields()
                            .iter()
                            .map(|field| UdlArg {
                                name: field.name().to_string(),
                                type_: field.as_type(),
                                docstring: field.docstring().map(ToString::to_string),
                                default: field.default_value().cloned(),
                            })
                            .collect(),
                        discr: if has_discr {
                            enum_.variant_discr(i).ok()
                        } else {
                            None
                        },
                    })
                    .collect(),
                methods,
                trait_methods,
            }
        })
        .collect::<Vec<_>>();
    let objects = ci
        .object_definitions()
        .iter()
        .map(|obj| {
            let mut methods = obj
                .methods()
                .into_iter()
                .map(|m| UdlObjectMethod {
                    name: m.name().to_string(),
                    ffi_symbol: include_ffi_symbols.then(|| m.ffi_func().name().to_string()),
                    ffi_arg_types: if include_ffi_symbols {
                        m.ffi_func().arguments().iter().map(|a| a.type_()).collect()
                    } else {
                        Vec::new()
                    },
                    ffi_return_type: include_ffi_symbols
                        .then(|| m.ffi_func().return_type().cloned())
                        .flatten(),
                    ffi_has_rust_call_status: if include_ffi_symbols {
                        m.ffi_func().has_rust_call_status_arg()
                    } else {
                        false
                    },
                    runtime_unsupported: include_ffi_symbols
                        .then(|| runtime_unsupported_reason_for_ffi_func(m.ffi_func()))
                        .flatten(),
                    docstring: m.docstring().map(ToString::to_string),
                    is_async: m.is_async(),
                    return_type: m.return_type().cloned(),
                    throws_type: m.throws_type().cloned(),
                    args: m
                        .arguments()
                        .into_iter()
                        .map(|a| UdlArg {
                            name: a.name().to_string(),
                            type_: a.as_type(),
                            docstring: None,
                            default: a.default_value().cloned(),
                        })
                        .collect(),
                })
                .collect::<Vec<_>>();
            let mut trait_methods = UdlObjectTraitMethods::default();
            for method in &methods {
                match uniffi_trait_method_kind(&method.name) {
                    Some("display") => trait_methods.display = Some(method.name.clone()),
                    Some("debug") => trait_methods.debug = Some(method.name.clone()),
                    Some("hash") => trait_methods.hash = Some(method.name.clone()),
                    Some("eq") => trait_methods.eq = Some(method.name.clone()),
                    Some("ne") => trait_methods.ne = Some(method.name.clone()),
                    Some("ord_cmp") => trait_methods.ord_cmp = Some(method.name.clone()),
                    _ => {}
                }
            }
            let traits_for_object = udl_interface_traits
                .get(obj.name())
                .cloned()
                .unwrap_or_default();
            if traits_for_object.iter().any(|t| t == "Display") && trait_methods.display.is_none() {
                trait_methods.display = Some("uniffi_trait_display".to_string());
                methods.push(UdlObjectMethod {
                    name: "uniffi_trait_display".to_string(),
                    ffi_symbol: None,
                    ffi_arg_types: Vec::new(),
                    ffi_return_type: None,
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: None,
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::String),
                    throws_type: None,
                    args: Vec::new(),
                });
            }
            if traits_for_object.iter().any(|t| t == "Debug") && trait_methods.debug.is_none() {
                trait_methods.debug = Some("uniffi_trait_debug".to_string());
                methods.push(UdlObjectMethod {
                    name: "uniffi_trait_debug".to_string(),
                    ffi_symbol: None,
                    ffi_arg_types: Vec::new(),
                    ffi_return_type: None,
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: None,
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::String),
                    throws_type: None,
                    args: Vec::new(),
                });
            }
            if traits_for_object.iter().any(|t| t == "Hash") && trait_methods.hash.is_none() {
                trait_methods.hash = Some("uniffi_trait_hash".to_string());
                methods.push(UdlObjectMethod {
                    name: "uniffi_trait_hash".to_string(),
                    ffi_symbol: None,
                    ffi_arg_types: Vec::new(),
                    ffi_return_type: None,
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: None,
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::UInt64),
                    throws_type: None,
                    args: Vec::new(),
                });
            }
            if traits_for_object.iter().any(|t| t == "Eq") && trait_methods.eq.is_none() {
                trait_methods.eq = Some("uniffi_trait_eq".to_string());
                methods.push(UdlObjectMethod {
                    name: "uniffi_trait_eq".to_string(),
                    ffi_symbol: None,
                    ffi_arg_types: Vec::new(),
                    ffi_return_type: None,
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: None,
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::Boolean),
                    throws_type: None,
                    args: vec![UdlArg {
                        name: "other".to_string(),
                        type_: Type::UInt64,
                        docstring: None,
                        default: None,
                    }],
                });
            }
            if traits_for_object.iter().any(|t| t == "Eq") && trait_methods.ne.is_none() {
                trait_methods.ne = Some("uniffi_trait_ne".to_string());
                methods.push(UdlObjectMethod {
                    name: "uniffi_trait_ne".to_string(),
                    ffi_symbol: None,
                    ffi_arg_types: Vec::new(),
                    ffi_return_type: None,
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: None,
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::Boolean),
                    throws_type: None,
                    args: vec![UdlArg {
                        name: "other".to_string(),
                        type_: Type::UInt64,
                        docstring: None,
                        default: None,
                    }],
                });
            }
            if traits_for_object.iter().any(|t| t == "Ord") && trait_methods.ord_cmp.is_none() {
                trait_methods.ord_cmp = Some("uniffi_trait_ord_cmp".to_string());
                methods.push(UdlObjectMethod {
                    name: "uniffi_trait_ord_cmp".to_string(),
                    ffi_symbol: None,
                    ffi_arg_types: Vec::new(),
                    ffi_return_type: None,
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: None,
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::Int8),
                    throws_type: None,
                    args: vec![UdlArg {
                        name: "other".to_string(),
                        type_: Type::UInt64,
                        docstring: None,
                        default: None,
                    }],
                });
            }
            methods.sort_by(|a, b| a.name.cmp(&b.name));
            methods.dedup_by(|a, b| a.name == b.name);

            UdlObject {
                name: obj.name().to_string(),
                docstring: obj.docstring().map(ToString::to_string),
                is_error: ci.is_name_used_as_error(obj.name()),
                has_callback_interface: obj.has_callback_interface(),
                ffi_free_symbol: include_ffi_symbols
                    .then(|| obj.ffi_object_free().name().to_string()),
                ffi_clone_symbol: include_ffi_symbols
                    .then(|| obj.ffi_object_clone().name().to_string()),
                constructors: obj
                    .constructors()
                    .into_iter()
                    .map(|ctor| UdlObjectConstructor {
                        name: ctor.name().to_string(),
                        ffi_symbol: include_ffi_symbols.then(|| ctor.ffi_func().name().to_string()),
                        ffi_arg_types: if include_ffi_symbols {
                            ctor.ffi_func()
                                .arguments()
                                .iter()
                                .map(|a| a.type_())
                                .collect()
                        } else {
                            Vec::new()
                        },
                        ffi_return_type: include_ffi_symbols
                            .then(|| ctor.ffi_func().return_type().cloned())
                            .flatten(),
                        ffi_has_rust_call_status: if include_ffi_symbols {
                            ctor.ffi_func().has_rust_call_status_arg()
                        } else {
                            false
                        },
                        runtime_unsupported: include_ffi_symbols
                            .then(|| runtime_unsupported_reason_for_ffi_func(ctor.ffi_func()))
                            .flatten(),
                        docstring: ctor.docstring().map(ToString::to_string),
                        is_async: ctor.is_async(),
                        args: ctor
                            .arguments()
                            .into_iter()
                            .map(|a| UdlArg {
                                name: a.name().to_string(),
                                type_: a.as_type(),
                                docstring: None,
                                default: a.default_value().cloned(),
                            })
                            .collect(),
                        throws_type: ctor.throws_type().cloned(),
                    })
                    .collect(),
                methods,
                trait_methods,
            }
        })
        .collect::<Vec<_>>();
    let callback_interfaces = ci
        .callback_interface_definitions()
        .iter()
        .map(|cb| UdlCallbackInterface {
            name: cb.name().to_string(),
            docstring: cb.docstring().map(ToString::to_string),
            methods: cb
                .methods()
                .into_iter()
                .map(|m| UdlCallbackMethod {
                    name: m.name().to_string(),
                    docstring: m.docstring().map(ToString::to_string),
                    is_async: m.is_async(),
                    return_type: m.return_type().cloned(),
                    throws_type: m.throws_type().cloned(),
                    args: m
                        .arguments()
                        .into_iter()
                        .map(|a| UdlArg {
                            name: a.name().to_string(),
                            type_: a.as_type(),
                            docstring: None,
                            default: a.default_value().cloned(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    Ok(UdlMetadata {
        namespace: Some(ci.namespace().to_string()),
        local_module_path: ci.crate_name().to_string(),
        namespace_docstring: ci.namespace_docstring().map(ToString::to_string),
        uniffi_contract_version: include_ffi_symbols.then(|| ci.uniffi_contract_version()),
        ffi_uniffi_contract_version_symbol: include_ffi_symbols
            .then(|| ci.ffi_uniffi_contract_version().name().to_string()),
        api_checksums: if include_ffi_symbols {
            ci.iter_checksums()
                .map(|(symbol, expected)| UdlApiChecksum { symbol, expected })
                .collect()
        } else {
            Vec::new()
        },
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    })
}

pub(super) fn parse_udl_interface_traits(
    udl: &str,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut out = std::collections::HashMap::new();
    let mut pending_traits: Option<Vec<String>> = None;

    for raw_line in udl.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            if let Some(traits) = parse_traits_from_attribute_line(line) {
                pending_traits = Some(traits);
            }
            if let Some(interface_name) = parse_interface_name_from_line(line) {
                if let Some(traits) = pending_traits.take() {
                    out.insert(interface_name, traits);
                }
            }
            continue;
        }

        if let Some(interface_name) = parse_interface_name_from_line(line) {
            if let Some(traits) = pending_traits.take() {
                out.insert(interface_name, traits);
            }
            continue;
        }

        if let Some(dict_name) = parse_dictionary_name_from_line(line) {
            if let Some(traits) = pending_traits.take() {
                out.insert(dict_name, traits);
            }
            continue;
        }

        if let Some(enum_name) = parse_enum_name_from_line(line) {
            if let Some(traits) = pending_traits.take() {
                out.insert(enum_name, traits);
            }
            continue;
        }

        if line.starts_with("namespace ") || line.starts_with("callback interface ") {
            pending_traits = None;
        }
    }

    out
}

pub(super) fn parse_udl_namespace_function_docstrings(
    udl: &str,
) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let mut pending_doc_lines: Vec<String> = Vec::new();
    let mut in_namespace = false;
    let mut brace_depth = 0_i32;

    for raw_line in udl.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("///") {
            let text = line.trim_start_matches("///").trim().to_string();
            pending_doc_lines.push(text);
            continue;
        }

        let open_count = line.matches('{').count() as i32;
        let close_count = line.matches('}').count() as i32;
        if line.starts_with("namespace ") {
            in_namespace = true;
            brace_depth += open_count - close_count;
            if brace_depth <= 0 {
                in_namespace = false;
                brace_depth = 0;
            }
            pending_doc_lines.clear();
            continue;
        }

        if in_namespace {
            brace_depth += open_count - close_count;
            if line.ends_with(';')
                && line.contains('(')
                && !line.starts_with('[')
                && !line.starts_with("interface ")
                && !line.starts_with("dictionary ")
                && !line.starts_with("enum ")
                && !line.starts_with("callback interface ")
            {
                if let Some(idx) = line.find('(') {
                    let head = &line[..idx];
                    let name = head
                        .split_whitespace()
                        .last()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    if let Some(name) = name {
                        if !pending_doc_lines.is_empty() {
                            let doc = pending_doc_lines.join("\n");
                            out.insert(name.clone(), doc.clone());
                            out.insert(to_lower_camel(&name), doc);
                        }
                    }
                }
            }
            if brace_depth <= 0 {
                in_namespace = false;
                brace_depth = 0;
            }
        }

        if !line.starts_with('[') {
            pending_doc_lines.clear();
        }
    }

    out
}

pub(super) fn parse_traits_from_attribute_line(line: &str) -> Option<Vec<String>> {
    let marker = "Traits=(";
    let start = line.find(marker)?;
    let tail = &line[start + marker.len()..];
    let end = tail.find(')')?;
    let inner = &tail[..end];
    let traits = inner
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if traits.is_empty() {
        None
    } else {
        Some(traits)
    }
}

pub(super) fn parse_interface_name_from_line(line: &str) -> Option<String> {
    let marker = "interface ";
    let start = line.find(marker)?;
    let rest = &line[start + marker.len()..];
    let name = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

pub(super) fn parse_dictionary_name_from_line(line: &str) -> Option<String> {
    let marker = "dictionary ";
    if !line.starts_with(marker) {
        return None;
    }
    let rest = &line[marker.len()..];
    let name = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

pub(super) fn parse_enum_name_from_line(line: &str) -> Option<String> {
    let marker = "enum ";
    if !line.starts_with(marker) {
        return None;
    }
    let rest = &line[marker.len()..];
    let name = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Convert a `uniffi_bindgen::interface::Method` to a `UdlObjectMethod`.
fn ci_method_to_udl(m: &Method, include_ffi_symbols: bool) -> UdlObjectMethod {
    UdlObjectMethod {
        name: m.name().to_string(),
        ffi_symbol: include_ffi_symbols.then(|| m.ffi_func().name().to_string()),
        ffi_arg_types: if include_ffi_symbols {
            m.ffi_func().arguments().iter().map(|a| a.type_()).collect()
        } else {
            Vec::new()
        },
        ffi_return_type: include_ffi_symbols
            .then(|| m.ffi_func().return_type().cloned())
            .flatten(),
        ffi_has_rust_call_status: if include_ffi_symbols {
            m.ffi_func().has_rust_call_status_arg()
        } else {
            false
        },
        runtime_unsupported: include_ffi_symbols
            .then(|| runtime_unsupported_reason_for_ffi_func(m.ffi_func()))
            .flatten(),
        docstring: m.docstring().map(ToString::to_string),
        is_async: m.is_async(),
        return_type: m.return_type().cloned(),
        throws_type: m.throws_type().cloned(),
        args: m
            .arguments()
            .into_iter()
            .map(|a| UdlArg {
                name: a.name().to_string(),
                type_: a.as_type(),
                docstring: None,
                default: a.default_value().cloned(),
            })
            .collect(),
    }
}

/// Extract trait-synthesised method names from `UniffiTraitMethods` into
/// the `trait_methods` struct.  These are stored as metadata only — records
/// and enums render structural trait implementations (toString, ==, hashCode)
/// via the `traits: Vec<String>` field, so we do NOT push them into the
/// regular `methods` list.
fn extract_record_enum_trait_methods(
    ci_trait_methods: &UniffiTraitMethods,
    trait_methods: &mut UdlObjectTraitMethods,
) {
    if let Some(ref m) = ci_trait_methods.display_fmt {
        trait_methods.display = Some(m.name().to_string());
    }
    if let Some(ref m) = ci_trait_methods.debug_fmt {
        trait_methods.debug = Some(m.name().to_string());
    }
    if let Some(ref m) = ci_trait_methods.hash_hash {
        trait_methods.hash = Some(m.name().to_string());
    }
    if let Some(ref m) = ci_trait_methods.eq_eq {
        trait_methods.eq = Some(m.name().to_string());
    }
    if let Some(ref m) = ci_trait_methods.eq_ne {
        trait_methods.ne = Some(m.name().to_string());
    }
    if let Some(ref m) = ci_trait_methods.ord_cmp {
        trait_methods.ord_cmp = Some(m.name().to_string());
    }
}
