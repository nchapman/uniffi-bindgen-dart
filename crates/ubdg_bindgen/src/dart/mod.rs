use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use uniffi_bindgen::interface::{
    ffi::FfiType, AsType, ComponentInterface, DefaultValue, Literal, Radix, Type,
};

use crate::GenerateArgs;

pub mod callback_interface;
pub mod compounds;
pub mod config;
pub mod custom;
pub mod enum_;
pub mod error;
pub mod object;
pub mod oracle;
pub mod primitives;
pub mod record;

pub fn generate_bindings(args: &GenerateArgs) -> Result<()> {
    let cfg = config::load(args)?;
    let metadata = parse_udl_metadata(&args.source, args.crate_name.as_deref(), args.library)?;
    let namespace = metadata
        .namespace
        .clone()
        .unwrap_or(namespace_from_source(&args.source)?);

    let module_name = cfg
        .module_name
        .clone()
        .unwrap_or_else(|| dart_identifier(&namespace));
    let ffi_class_name = cfg
        .ffi_class_name
        .clone()
        .unwrap_or_else(|| format!("{}Ffi", to_upper_camel(&namespace)));
    let library_name = cfg
        .library_name
        .clone()
        .or_else(|| args.crate_name.clone())
        .unwrap_or_else(|| format!("uniffi_{}", namespace.replace('-', "_")));

    fs::create_dir_all(&args.out_dir)?;

    let output_file = args.out_dir.join(format!("{namespace}.dart"));
    let content = render_dart_scaffold(
        &module_name,
        &ffi_class_name,
        &library_name,
        metadata.namespace_docstring.as_deref(),
        &metadata.local_module_path,
        metadata.uniffi_contract_version,
        metadata.ffi_uniffi_contract_version_symbol.as_deref(),
        &metadata.api_checksums,
        &cfg.external_packages,
        &cfg.rename,
        &cfg.exclude,
        &metadata.functions,
        &metadata.objects,
        &metadata.callback_interfaces,
        &metadata.records,
        &metadata.enums,
    );
    fs::write(&output_file, content).with_context(|| {
        format!(
            "failed to write generated dart bindings: {}",
            output_file.display()
        )
    })?;

    Ok(())
}

fn namespace_from_source(source: &Path) -> Result<String> {
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

fn extract_namespace_from_udl(source: &Path) -> Option<String> {
    if source.extension().and_then(|e| e.to_str()) != Some("udl") {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlFunction {
    name: String,
    ffi_symbol: Option<String>,
    ffi_arg_types: Vec<FfiType>,
    ffi_return_type: Option<FfiType>,
    ffi_has_rust_call_status: bool,
    runtime_unsupported: Option<String>,
    docstring: Option<String>,
    is_async: bool,
    return_type: Option<Type>,
    throws_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlArg {
    name: String,
    type_: Type,
    docstring: Option<String>,
    default: Option<DefaultValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlObject {
    name: String,
    docstring: Option<String>,
    constructors: Vec<UdlObjectConstructor>,
    methods: Vec<UdlObjectMethod>,
    trait_methods: UdlObjectTraitMethods,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlObjectConstructor {
    name: String,
    ffi_symbol: Option<String>,
    ffi_arg_types: Vec<FfiType>,
    ffi_return_type: Option<FfiType>,
    ffi_has_rust_call_status: bool,
    runtime_unsupported: Option<String>,
    docstring: Option<String>,
    is_async: bool,
    args: Vec<UdlArg>,
    throws_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlObjectMethod {
    name: String,
    ffi_symbol: Option<String>,
    ffi_arg_types: Vec<FfiType>,
    ffi_return_type: Option<FfiType>,
    ffi_has_rust_call_status: bool,
    runtime_unsupported: Option<String>,
    docstring: Option<String>,
    is_async: bool,
    return_type: Option<Type>,
    throws_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct UdlObjectTraitMethods {
    display: Option<String>,
    debug: Option<String>,
    hash: Option<String>,
    eq: Option<String>,
    ne: Option<String>,
    ord_cmp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlCallbackInterface {
    name: String,
    docstring: Option<String>,
    methods: Vec<UdlCallbackMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlCallbackMethod {
    name: String,
    docstring: Option<String>,
    is_async: bool,
    return_type: Option<Type>,
    throws_type: Option<Type>,
    args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlRecord {
    name: String,
    docstring: Option<String>,
    fields: Vec<UdlArg>,
    methods: Vec<UdlObjectMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlEnum {
    name: String,
    docstring: Option<String>,
    is_error: bool,
    variants: Vec<UdlEnumVariant>,
    methods: Vec<UdlObjectMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlEnumVariant {
    name: String,
    docstring: Option<String>,
    fields: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdlApiChecksum {
    symbol: String,
    expected: u16,
}

impl UdlFunction {
    fn uses_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(uniffi_type_uses_bytes)
            || self.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
    }

    fn uses_runtime_string(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_string_like_type)
            || self
                .args
                .iter()
                .any(|a| is_runtime_string_like_type(&a.type_))
    }

    fn returns_runtime_string(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_string_like_type)
    }

    fn uses_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_like_type)
            || self
                .args
                .iter()
                .any(|a| is_runtime_bytes_like_type(&a.type_))
    }

    fn returns_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_like_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct UdlMetadata {
    namespace: Option<String>,
    local_module_path: String,
    namespace_docstring: Option<String>,
    uniffi_contract_version: Option<u32>,
    ffi_uniffi_contract_version_symbol: Option<String>,
    api_checksums: Vec<UdlApiChecksum>,
    functions: Vec<UdlFunction>,
    objects: Vec<UdlObject>,
    callback_interfaces: Vec<UdlCallbackInterface>,
    records: Vec<UdlRecord>,
    enums: Vec<UdlEnum>,
}

fn parse_udl_metadata(
    source: &Path,
    crate_name: Option<&str>,
    library_mode: bool,
) -> Result<UdlMetadata> {
    if source.extension().and_then(|e| e.to_str()) != Some("udl") {
        if !library_mode {
            return Ok(UdlMetadata {
                namespace: None,
                local_module_path: String::new(),
                namespace_docstring: None,
                uniffi_contract_version: None,
                ffi_uniffi_contract_version_symbol: None,
                api_checksums: Vec::new(),
                ..UdlMetadata::default()
            });
        }
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

fn component_interface_to_metadata(
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
        .map(|record| UdlRecord {
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
            methods: record
                .methods()
                .iter()
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
                .collect(),
        })
        .collect::<Vec<_>>();
    let enums = ci
        .enum_definitions()
        .iter()
        .map(|enum_| UdlEnum {
            name: enum_.name().to_string(),
            docstring: enum_.docstring().map(ToString::to_string),
            is_error: ci.is_name_used_as_error(enum_.name()),
            variants: enum_
                .variants()
                .iter()
                .map(|variant| UdlEnumVariant {
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
                })
                .collect(),
            methods: enum_
                .methods()
                .iter()
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
                .collect(),
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

fn ffi_type_contains_rust_buffer(type_: &FfiType) -> bool {
    match type_ {
        FfiType::RustBuffer(_) => true,
        FfiType::Reference(inner) | FfiType::MutReference(inner) => {
            ffi_type_contains_rust_buffer(inner)
        }
        _ => false,
    }
}

fn runtime_unsupported_reason_for_ffi_func(
    ffi_func: &uniffi_bindgen::interface::FfiFunction,
) -> Option<String> {
    if ffi_func.has_rust_call_status_arg() {
        return Some("runtime invocation for this UniFFI ABI (RustCallStatus out-arg) is not implemented yet".to_string());
    }
    if ffi_func
        .arguments()
        .iter()
        .any(|arg| ffi_type_contains_rust_buffer(&arg.type_()))
    {
        return Some(
            "runtime invocation for this UniFFI ABI (RustBuffer argument) is not implemented yet"
                .to_string(),
        );
    }
    if ffi_func
        .return_type()
        .is_some_and(ffi_type_contains_rust_buffer)
    {
        return Some(
            "runtime invocation for this UniFFI ABI (RustBuffer return) is not implemented yet"
                .to_string(),
        );
    }
    None
}

#[allow(dead_code)]
fn is_ffibuffer_supported_ffi_type(type_: &FfiType) -> bool {
    match type_ {
        FfiType::UInt8
        | FfiType::Int8
        | FfiType::UInt16
        | FfiType::Int16
        | FfiType::UInt32
        | FfiType::Int32
        | FfiType::UInt64
        | FfiType::Int64
        | FfiType::Float32
        | FfiType::Float64
        | FfiType::Handle
        | FfiType::RustBuffer(_)
        | FfiType::RustCallStatus => true,
        FfiType::Reference(inner) | FfiType::MutReference(inner) => {
            matches!(inner.as_ref(), FfiType::VoidPointer)
        }
        _ => false,
    }
}

#[allow(dead_code)]
fn is_ffibuffer_eligible_function(function: &UdlFunction) -> bool {
    function.ffi_symbol.is_some() && !function.is_async
}

fn is_runtime_unsupported_async_ffibuffer_eligible_function(function: &UdlFunction) -> bool {
    if function.runtime_unsupported.is_none()
        || !function.is_async
        || function.throws_type.is_some()
        || function.ffi_symbol.is_none()
    {
        return false;
    }
    async_rust_future_spec_from_uniffi_return_type(function.return_type.as_ref()).is_some()
}

fn ffibuffer_symbol_name(ffi_symbol: &str) -> String {
    if let Some(rest) = ffi_symbol.strip_prefix("uniffi_") {
        format!("uniffi_ffibuffer_{rest}")
    } else {
        format!("uniffi_ffibuffer_{ffi_symbol}")
    }
}

fn ffibuffer_element_count(ffi_type: &FfiType) -> Option<usize> {
    match ffi_type {
        FfiType::UInt8
        | FfiType::Int8
        | FfiType::UInt16
        | FfiType::Int16
        | FfiType::UInt32
        | FfiType::Int32
        | FfiType::UInt64
        | FfiType::Int64
        | FfiType::Float32
        | FfiType::Float64
        | FfiType::Handle
        | FfiType::Reference(_)
        | FfiType::MutReference(_) => Some(1),
        FfiType::RustBuffer(_) => Some(3),
        FfiType::RustCallStatus => Some(4),
        _ => None,
    }
}

fn ffibuffer_primitive_union_field(ffi_type: &FfiType) -> Option<&'static str> {
    match ffi_type {
        FfiType::UInt8 => Some("u8"),
        FfiType::Int8 => Some("i8"),
        FfiType::UInt16 => Some("u16"),
        FfiType::Int16 => Some("i16"),
        FfiType::UInt32 => Some("u32"),
        FfiType::Int32 => Some("i32"),
        FfiType::UInt64 | FfiType::Int64 | FfiType::Handle => Some("u64"),
        FfiType::Float32 => Some("float32"),
        FfiType::Float64 => Some("float64"),
        FfiType::Reference(inner) | FfiType::MutReference(inner)
            if matches!(inner.as_ref(), FfiType::VoidPointer) =>
        {
            Some("ptr")
        }
        _ => None,
    }
}

fn ffibuffer_ffi_type_from_uniffi_type(type_: &Type) -> Option<FfiType> {
    if let Type::Custom { builtin, .. } = type_ {
        return ffibuffer_ffi_type_from_uniffi_type(builtin);
    }
    match type_ {
        Type::UInt8 => Some(FfiType::UInt8),
        Type::Int8 => Some(FfiType::Int8),
        Type::UInt16 => Some(FfiType::UInt16),
        Type::Int16 => Some(FfiType::Int16),
        Type::UInt32 => Some(FfiType::UInt32),
        Type::Int32 => Some(FfiType::Int32),
        Type::UInt64 => Some(FfiType::UInt64),
        Type::Int64 => Some(FfiType::Int64),
        Type::Float32 => Some(FfiType::Float32),
        Type::Float64 => Some(FfiType::Float64),
        Type::Boolean => Some(FfiType::Int8),
        Type::Object { .. } | Type::CallbackInterface { .. } => Some(FfiType::Handle),
        Type::String
        | Type::Bytes
        | Type::Timestamp
        | Type::Duration
        | Type::Optional { .. }
        | Type::Sequence { .. }
        | Type::Map { .. }
        | Type::Record { .. }
        | Type::Enum { .. } => Some(FfiType::RustBuffer(None)),
        _ => None,
    }
}

fn is_ffibuffer_eligible_object_member(method: &UdlObjectMethod) -> bool {
    method.ffi_symbol.is_some() && !method.is_async
}

fn is_ffibuffer_eligible_object_constructor(ctor: &UdlObjectConstructor) -> bool {
    ctor.ffi_symbol.is_some() && !ctor.is_async
}

fn is_runtime_unsupported_async_ffibuffer_eligible_method(method: &UdlObjectMethod) -> bool {
    if method.runtime_unsupported.is_none()
        || !method.is_async
        || method.throws_type.is_some()
        || method.ffi_symbol.is_none()
    {
        return false;
    }
    async_rust_future_spec_from_uniffi_return_type(method.return_type.as_ref()).is_some()
}

fn parse_udl_interface_traits(udl: &str) -> std::collections::HashMap<String, Vec<String>> {
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

        if line.starts_with("dictionary ")
            || line.starts_with("enum ")
            || line.starts_with("namespace ")
            || line.starts_with("callback interface ")
        {
            pending_traits = None;
        }
    }

    out
}

fn parse_udl_namespace_function_docstrings(udl: &str) -> std::collections::HashMap<String, String> {
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

fn parse_traits_from_attribute_line(line: &str) -> Option<Vec<String>> {
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

fn parse_interface_name_from_line(line: &str) -> Option<String> {
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

#[allow(clippy::too_many_arguments)]
fn render_dart_scaffold(
    module_name: &str,
    ffi_class_name: &str,
    library_name: &str,
    namespace_docstring: Option<&str>,
    local_module_path: &str,
    uniffi_contract_version: Option<u32>,
    ffi_uniffi_contract_version_symbol: Option<&str>,
    api_checksums: &[UdlApiChecksum],
    external_packages: &HashMap<String, String>,
    rename: &HashMap<String, String>,
    exclude: &[String],
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let api_overrides = ApiOverrides::new(rename, exclude);
    let external_import_uris =
        collect_external_import_uris(local_module_path, external_packages, functions, objects);
    let needs_callback_runtime =
        has_runtime_callback_support(functions, objects, callback_interfaces, records, enums);
    let needs_async_rust_future =
        has_runtime_async_rust_future_support(
            functions,
            objects,
            callback_interfaces,
            records,
            enums,
        ) || has_runtime_unsupported_async_ffibuffer_support(functions, records, enums);
    let needs_rust_call_status = needs_async_rust_future || needs_callback_runtime;
    let has_runtime_unsupported = functions.iter().any(|f| f.runtime_unsupported.is_some())
        || objects.iter().any(|o| {
            o.constructors
                .iter()
                .any(|c| c.runtime_unsupported.is_some())
                || o.methods.iter().any(|m| m.runtime_unsupported.is_some())
        })
        || records
            .iter()
            .flat_map(|r| r.methods.iter())
            .any(|m| m.runtime_unsupported.is_some())
        || enums
            .iter()
            .flat_map(|e| e.methods.iter())
            .any(|m| m.runtime_unsupported.is_some());
    let needs_typed_data = has_runtime_unsupported
        || functions.iter().any(UdlFunction::uses_bytes)
        || objects.iter().any(|o| {
            o.methods.iter().any(|m| {
                m.return_type.as_ref().is_some_and(uniffi_type_uses_bytes)
                    || m.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
            })
        })
        || records.iter().any(|r| {
            r.methods.iter().any(|m| {
                m.return_type.as_ref().is_some_and(uniffi_type_uses_bytes)
                    || m.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
            })
        })
        || enums.iter().any(|e| {
            e.methods.iter().any(|m| {
                m.return_type.as_ref().is_some_and(uniffi_type_uses_bytes)
                    || m.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
            })
        });
    let needs_json_convert = !records.is_empty()
        || !enums.is_empty()
        || functions.iter().any(|f| {
            f.throws_type.is_some()
                || f.return_type.as_ref().is_some_and(uniffi_type_uses_json)
                || f.args.iter().any(|a| uniffi_type_uses_json(&a.type_))
        })
        || objects.iter().any(|o| {
            o.constructors.iter().any(|c| {
                c.throws_type.is_some() || c.args.iter().any(|a| uniffi_type_uses_json(&a.type_))
            }) || o.methods.iter().any(|m| {
                m.throws_type.is_some()
                    || m.return_type.as_ref().is_some_and(uniffi_type_uses_json)
                    || m.args.iter().any(|a| uniffi_type_uses_json(&a.type_))
            })
        });
    let needs_ffi_helpers = needs_async_rust_future
        || needs_callback_runtime
        || functions.iter().any(|f| {
            is_runtime_ffi_compatible_function(f, records, enums)
                && (f.uses_runtime_string()
                    || f.uses_runtime_bytes()
                    || f.return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
                    || f.args
                        .iter()
                        .any(|a| is_runtime_utf8_pointer_marshaled_type(&a.type_, records, enums))
                    || f.return_type
                        .as_ref()
                        .is_some_and(|t| is_runtime_record_or_enum_string_type(t, enums))
                    || f.args
                        .iter()
                        .any(|a| is_runtime_record_or_enum_string_type(&a.type_, enums)))
        })
        || !objects.is_empty()
        || records.iter().any(|r| !r.methods.is_empty())
        || enums.iter().any(|e| !e.methods.is_empty());
    let needs_runtime_bytes =
        functions.iter().any(|f| {
            is_runtime_ffi_compatible_function(f, records, enums) && f.uses_runtime_bytes()
        }) || objects.iter().any(|o| {
            o.methods.iter().any(|m| {
                m.return_type
                    .as_ref()
                    .is_some_and(is_runtime_bytes_like_type)
                    || m.args.iter().any(|a| is_runtime_bytes_like_type(&a.type_))
            })
        }) || records.iter().any(|r| {
            r.methods.iter().any(|m| {
                m.return_type
                    .as_ref()
                    .is_some_and(is_runtime_bytes_like_type)
                    || m.args.iter().any(|a| is_runtime_bytes_like_type(&a.type_))
            })
        }) || enums.iter().any(|e| {
            e.methods.iter().any(|m| {
                m.return_type
                    .as_ref()
                    .is_some_and(is_runtime_bytes_like_type)
                    || m.args.iter().any(|a| is_runtime_bytes_like_type(&a.type_))
            })
        });
    let needs_runtime_optional_bytes = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && (f
                .return_type
                .as_ref()
                .is_some_and(is_runtime_optional_bytes_type)
                || f.args
                    .iter()
                    .any(|a| is_runtime_optional_bytes_type(&a.type_)))
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_optional_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_optional_bytes_type(&a.type_))
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_optional_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_optional_bytes_type(&a.type_))
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_optional_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_optional_bytes_type(&a.type_))
        })
    });
    let needs_runtime_sequence_bytes = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && (f
                .return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
                || f.args
                    .iter()
                    .any(|a| is_runtime_sequence_bytes_type(&a.type_)))
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_sequence_bytes_type(&a.type_))
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_sequence_bytes_type(&a.type_))
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
                || m.args
                    .iter()
                    .any(|a| is_runtime_sequence_bytes_type(&a.type_))
        })
    });
    let mut out = String::new();
    out.push_str("// Generated by uniffi-bindgen-dart. DO NOT EDIT.\n");
    if has_runtime_unsupported {
        out.push_str("// ignore_for_file: unused_element, unused_import, unused_field\n");
    } else {
        out.push_str("// ignore_for_file: unused_element\n");
    }
    out.push_str(&render_doc_comment(namespace_docstring, ""));
    out.push_str(&format!("library {module_name};\n\n"));
    if needs_async_rust_future {
        out.push_str("import 'dart:async';\n");
    }
    if needs_json_convert {
        out.push_str("import 'dart:convert';\n");
    }
    out.push_str("import 'dart:ffi' as ffi;\n");
    if needs_ffi_helpers {
        out.push_str("import 'package:ffi/ffi.dart';\n");
    }
    if needs_typed_data {
        out.push_str("import 'dart:typed_data';\n");
    }
    for uri in &external_import_uris {
        out.push_str(&format!("import '{uri}';\n"));
    }
    out.push('\n');
    if needs_runtime_bytes {
        out.push_str("final class _RustBuffer extends ffi.Struct {\n");
        out.push_str("  external ffi.Pointer<ffi.Uint8> data;\n\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int len;\n");
        out.push_str("}\n\n");
    }
    if needs_runtime_optional_bytes {
        out.push_str("final class _RustBufferOpt extends ffi.Struct {\n");
        out.push_str("  @ffi.Uint8()\n");
        out.push_str("  external int isSome;\n\n");
        out.push_str("  external _RustBuffer value;\n");
        out.push_str("}\n\n");
    }
    if needs_runtime_sequence_bytes {
        out.push_str("final class _RustBufferVec extends ffi.Struct {\n");
        out.push_str("  external ffi.Pointer<_RustBuffer> data;\n\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int len;\n");
        out.push_str("}\n\n");
    }
    if has_runtime_unsupported {
        out.push_str("final class _UniFfiFfiBufferElement extends ffi.Union {\n");
        out.push_str("  @ffi.Uint8()\n");
        out.push_str("  external int u8;\n\n");
        out.push_str("  @ffi.Int8()\n");
        out.push_str("  external int i8;\n\n");
        out.push_str("  @ffi.Uint16()\n");
        out.push_str("  external int u16;\n\n");
        out.push_str("  @ffi.Int16()\n");
        out.push_str("  external int i16;\n\n");
        out.push_str("  @ffi.Uint32()\n");
        out.push_str("  external int u32;\n\n");
        out.push_str("  @ffi.Int32()\n");
        out.push_str("  external int i32;\n\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int u64;\n\n");
        out.push_str("  @ffi.Int64()\n");
        out.push_str("  external int i64;\n\n");
        out.push_str("  @ffi.Float()\n");
        out.push_str("  external double float32;\n\n");
        out.push_str("  @ffi.Double()\n");
        out.push_str("  external double float64;\n\n");
        out.push_str("  external ffi.Pointer<ffi.Void> ptr;\n");
        out.push_str("}\n\n");
        out.push_str("final class _UniFfiRustBuffer extends ffi.Struct {\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int capacity;\n\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int len;\n\n");
        out.push_str("  external ffi.Pointer<ffi.Uint8> data;\n");
        out.push_str("}\n\n");
        out.push_str("final class _UniFfiForeignBytes extends ffi.Struct {\n");
        out.push_str("  @ffi.Int32()\n");
        out.push_str("  external int len;\n\n");
        out.push_str("  external ffi.Pointer<ffi.Uint8> data;\n");
        out.push_str("}\n\n");
        out.push_str("final class _UniFfiRustCallStatus extends ffi.Struct {\n");
        out.push_str("  @ffi.Int8()\n");
        out.push_str("  external int code;\n\n");
        out.push_str("  external _UniFfiRustBuffer errorBuf;\n");
        out.push_str("}\n\n");
        out.push_str("const int _uniFfiRustCallStatusSuccess = 0;\n");
        out.push_str("const int _uniFfiRustCallStatusError = 1;\n");
        out.push_str("const int _uniFfiRustCallStatusUnexpectedError = 2;\n");
        out.push_str("const int _uniFfiRustCallStatusCancelled = 3;\n\n");
    }
    if needs_rust_call_status {
        out.push_str("final class _RustCallStatus extends ffi.Struct {\n");
        out.push_str("  @ffi.Int8()\n");
        out.push_str("  external int code;\n\n");
        out.push_str("  external ffi.Pointer<Utf8> errorBuf;\n");
        out.push_str("}\n\n");
        out.push_str("const int _rustCallStatusSuccess = 0;\n");
        out.push_str("const int _rustCallStatusError = 1;\n");
        out.push_str("const int _rustCallStatusUnexpectedError = 2;\n");
        out.push_str("const int _rustCallStatusCancelled = 3;\n");
    }
    if needs_async_rust_future {
        out.push_str("const int _rustFuturePollReady = 0;\n");
        out.push_str("const int _rustFuturePollWake = 1;\n\n");
    }
    out.push_str(&render_data_models(
        records,
        enums,
        callback_interfaces,
        has_runtime_unsupported,
    ));
    if has_runtime_unsupported {
        out.push_str(&render_uniffi_binary_helpers(records, enums));
    }
    out.push_str(&render_callback_interfaces(callback_interfaces));
    out.push_str(&render_callback_bridges(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    ));
    out.push_str(&format!(
        "class {ffi_class_name} {{\n  {ffi_class_name}({{ffi.DynamicLibrary? dynamicLibrary, String? libraryPath}})\n      : _dynamicLibrary = dynamicLibrary,\n        _libraryPath = libraryPath;\n\n"
    ));
    out.push_str("  final ffi.DynamicLibrary? _dynamicLibrary;\n");
    out.push_str("  final String? _libraryPath;\n\n");
    out.push_str(&format!(
        "  static const String libraryName = '{library_name}';\n\n"
    ));
    out.push_str("  ffi.DynamicLibrary open() {\n");
    out.push_str("    final provided = _dynamicLibrary;\n");
    out.push_str("    if (provided != null) {\n");
    out.push_str("      return provided;\n");
    out.push_str("    }\n");
    out.push_str("    return ffi.DynamicLibrary.open(_libraryPath ?? libraryName);\n");
    out.push_str("  }\n\n");
    let has_uniffi_init_checks =
        uniffi_contract_version.is_some() && ffi_uniffi_contract_version_symbol.is_some();
    if has_uniffi_init_checks {
        let bindings_contract_version = uniffi_contract_version.unwrap_or_default();
        let contract_symbol = ffi_uniffi_contract_version_symbol.unwrap_or_default();
        out.push_str("  void _ensureApiIntegrity(ffi.DynamicLibrary lib) {\n");
        out.push_str(&format!(
            "    const int bindingsContractVersion = {bindings_contract_version};\n"
        ));
        out.push_str("    final int scaffoldingContractVersion;\n");
        out.push_str("    try {\n");
        out.push_str(&format!(
            "      final int Function() ffiContractVersion = lib.lookupFunction<ffi.Uint32 Function(), int Function()>('{contract_symbol}');\n"
        ));
        out.push_str("      scaffoldingContractVersion = ffiContractVersion();\n");
        out.push_str("    } catch (err) {\n");
        out.push_str(&format!(
            "      throw StateError('Missing or invalid UniFFI contract-version symbol `{contract_symbol}`: $err');\n"
        ));
        out.push_str("    }\n");
        out.push_str("    if (bindingsContractVersion != scaffoldingContractVersion) {\n");
        out.push_str(
            "      throw StateError('UniFFI contract version mismatch: expected $bindingsContractVersion, got $scaffoldingContractVersion');\n",
        );
        out.push_str("    }\n");
        for checksum in api_checksums {
            let checksum_field =
                safe_dart_identifier(&format!("_checksum_{}", dart_identifier(&checksum.symbol)));
            out.push_str(&format!("    final int {checksum_field};\n"));
            out.push_str("    try {\n");
            out.push_str(&format!(
                "      final int Function() checksumFn = lib.lookupFunction<ffi.Uint16 Function(), int Function()>('{}');\n",
                checksum.symbol
            ));
            out.push_str(&format!("      {checksum_field} = checksumFn();\n"));
            out.push_str("    } catch (err) {\n");
            out.push_str(&format!(
                "      throw StateError('Missing or invalid UniFFI checksum symbol `{}`: $err');\n",
                checksum.symbol
            ));
            out.push_str("    }\n");
            out.push_str(&format!(
                "    if ({checksum_field} != {}) {{\n",
                checksum.expected
            ));
            out.push_str(&format!(
                "      throw StateError('UniFFI API checksum mismatch for `{}`: expected {}, got ${checksum_field}');\n",
                checksum.symbol, checksum.expected
            ));
            out.push_str("    }\n");
        }
        out.push_str("  }\n\n");
        out.push_str("  late final ffi.DynamicLibrary _lib = (() {\n");
        out.push_str("    final ffi.DynamicLibrary lib = open();\n");
        out.push_str("    _ensureApiIntegrity(lib);\n");
        out.push_str("    return lib;\n");
        out.push_str("  })();\n");
    } else {
        out.push_str("  late final ffi.DynamicLibrary _lib = open();\n");
    }
    out.push_str(&render_bound_methods(
        functions,
        objects,
        callback_interfaces,
        library_name,
        local_module_path,
        records,
        enums,
    ));
    out.push_str("}\n");
    out.push_str(&render_object_classes(
        objects,
        callback_interfaces,
        ffi_class_name,
        &api_overrides,
        records,
        enums,
    ));
    out.push_str(&render_function_stubs(
        functions,
        objects,
        callback_interfaces,
        ffi_class_name,
        &api_overrides,
        records,
        enums,
    ));
    out
}

fn render_doc_comment(docstring: Option<&str>, indent: &str) -> String {
    let Some(raw) = docstring.map(str::trim) else {
        return String::new();
    };
    if raw.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for line in raw.lines() {
        let clean = line.trim();
        if clean.is_empty() {
            out.push_str(&format!("{indent}///\n"));
        } else {
            out.push_str(&format!("{indent}/// {clean}\n"));
        }
    }
    out
}

fn escape_dart_string_literal(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn render_default_value_expr(
    default: &DefaultValue,
    type_: &Type,
    enums: &[UdlEnum],
) -> Option<String> {
    match default {
        DefaultValue::Default => render_type_default_expr(type_, enums),
        DefaultValue::Literal(lit) => render_literal_default_expr(lit, type_, enums),
    }
}

fn render_type_default_expr(type_: &Type, enums: &[UdlEnum]) -> Option<String> {
    match type_ {
        Type::Boolean => Some("false".to_string()),
        Type::String => Some("''".to_string()),
        Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64 => Some("0".to_string()),
        Type::Float32 | Type::Float64 => Some("0.0".to_string()),
        Type::Bytes => Some("Uint8List(0)".to_string()),
        Type::Timestamp => Some("DateTime.fromMicrosecondsSinceEpoch(0, isUtc: true)".to_string()),
        Type::Duration => Some("Duration.zero".to_string()),
        Type::Optional { .. } => Some("null".to_string()),
        Type::Sequence { .. } => Some("const []".to_string()),
        Type::Map { .. } => Some("const {}".to_string()),
        Type::Custom { builtin, .. } => render_type_default_expr(builtin, enums),
        Type::Enum { name, .. } => {
            let enum_name = to_upper_camel(name);
            let enum_def = enums
                .iter()
                .find(|e| to_upper_camel(&e.name) == enum_name)?;
            let variant = enum_def.variants.first()?;
            if variant.fields.is_empty() && !enum_def.is_error {
                Some(format!(
                    "{enum_name}.{}",
                    safe_dart_identifier(&to_lower_camel(&variant.name))
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn render_literal_default_expr(lit: &Literal, type_: &Type, enums: &[UdlEnum]) -> Option<String> {
    match lit {
        Literal::Boolean(v) => Some(v.to_string()),
        Literal::String(v) => Some(format!("'{}'", escape_dart_string_literal(v))),
        Literal::UInt(v, radix, _) => Some(match radix {
            Radix::Decimal => v.to_string(),
            Radix::Octal => format!("0{o:o}", o = v),
            Radix::Hexadecimal => format!("0x{v:x}"),
        }),
        Literal::Int(v, _radix, _) => Some(v.to_string()),
        Literal::Float(v, _) => Some(v.to_string()),
        Literal::Enum(variant, enum_type) => {
            let enum_name = match enum_type {
                Type::Enum { name, .. } => to_upper_camel(name),
                _ => match type_ {
                    Type::Enum { name, .. } => to_upper_camel(name),
                    _ => return None,
                },
            };
            Some(format!(
                "{enum_name}.{}",
                safe_dart_identifier(&to_lower_camel(variant))
            ))
        }
        Literal::EmptySequence => Some("const []".to_string()),
        Literal::EmptyMap => Some("const {}".to_string()),
        Literal::None => Some("null".to_string()),
        Literal::Some { inner } => render_default_value_expr(inner, type_, enums),
    }
}

fn render_callable_args_signature(args: &[UdlArg], enums: &[UdlEnum]) -> String {
    let defaults = args
        .iter()
        .map(|a| {
            a.default
                .as_ref()
                .and_then(|d| render_default_value_expr(d, &a.type_, enums))
        })
        .collect::<Vec<_>>();
    let has_defaults = defaults.iter().any(|d| d.is_some());
    if !has_defaults {
        return args
            .iter()
            .map(|a| {
                format!(
                    "{} {}",
                    map_uniffi_type_to_dart(&a.type_),
                    safe_dart_identifier(&to_lower_camel(&a.name))
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
    }

    let params = args
        .iter()
        .zip(defaults.iter())
        .map(|(a, default_expr)| {
            let field_type = map_uniffi_type_to_dart(&a.type_);
            let field_name = safe_dart_identifier(&to_lower_camel(&a.name));
            if let Some(default_expr) = default_expr {
                format!("{field_type} {field_name} = {default_expr}")
            } else {
                format!("required {field_type} {field_name}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{params}}}")
}

fn render_callable_arg_names(args: &[UdlArg]) -> String {
    args.iter()
        .map(|a| safe_dart_identifier(&to_lower_camel(&a.name)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_data_models(
    records: &[UdlRecord],
    enums: &[UdlEnum],
    callback_interfaces: &[UdlCallbackInterface],
    emit_uniffi_error_lift_helpers: bool,
) -> String {
    let mut out = String::new();

    for record in records {
        let class_name = to_upper_camel(&record.name);
        out.push_str(&render_doc_comment(record.docstring.as_deref(), ""));
        out.push_str(&format!("class {class_name} {{\n"));
        out.push_str(&format!("  const {class_name}({{\n"));
        for field in &record.fields {
            out.push_str(&render_doc_comment(field.docstring.as_deref(), "    "));
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            if let Some(default_expr) = field
                .default
                .as_ref()
                .and_then(|d| render_default_value_expr(d, &field.type_, enums))
            {
                out.push_str(&format!("    this.{field_name} = {default_expr},\n"));
            } else {
                out.push_str(&format!("    required this.{field_name},\n"));
            }
        }
        out.push_str("  });\n\n");
        for field in &record.fields {
            out.push_str(&render_doc_comment(field.docstring.as_deref(), "  "));
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            out.push_str(&format!(
                "  final {} {field_name};\n",
                map_uniffi_type_to_dart(&field.type_)
            ));
        }
        out.push('\n');
        out.push_str("  Map<String, dynamic> toJson() {\n");
        out.push_str("    return {\n");
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let expr = render_json_encode_expr(&format!("this.{field_name}"), &field.type_);
            out.push_str(&format!("      '{field_name}': {expr},\n"));
        }
        out.push_str("    };\n");
        out.push_str("  }\n\n");
        out.push_str(&format!(
            "  static {class_name} fromJson(Map<String, dynamic> json) {{\n"
        ));
        out.push_str(&format!("    return {class_name}(\n"));
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let decode = render_json_decode_expr(&format!("json['{field_name}']"), &field.type_);
            if let Some(default_expr) = field
                .default
                .as_ref()
                .and_then(|d| render_default_value_expr(d, &field.type_, enums))
            {
                out.push_str(&format!(
                    "      {field_name}: json.containsKey('{field_name}') ? {decode} : {default_expr},\n"
                ));
            } else {
                out.push_str(&format!("      {field_name}: {decode},\n"));
            }
        }
        out.push_str("    );\n");
        out.push_str("  }\n");
        if !record.fields.is_empty() {
            out.push('\n');
            out.push_str(&format!("  {class_name} copyWith({{\n"));
            for field in &record.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let field_type = map_uniffi_type_to_dart(&field.type_);
                let copy_with_type = if field_type.ends_with('?') {
                    field_type
                } else {
                    format!("{field_type}?")
                };
                out.push_str(&format!("    {copy_with_type} {field_name},\n"));
            }
            out.push_str("  }) {\n");
            out.push_str(&format!("    return {class_name}(\n"));
            for field in &record.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                out.push_str(&format!(
                    "      {field_name}: {field_name} ?? this.{field_name},\n"
                ));
            }
            out.push_str("    );\n");
            out.push_str("  }\n");
        }
        for method in &record.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let invoke_name = safe_dart_identifier(&to_lower_camel(&format!(
                "{}_{}",
                dart_identifier(&record.name),
                dart_identifier(&method.name)
            )));
            let return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return = if method.is_async {
                format!("Future<{return_type}>")
            } else {
                return_type.clone()
            };
            let args = render_callable_args_signature(&method.args, enums);
            let arg_names = render_callable_arg_names(&method.args);
            let invoke_args = if arg_names.is_empty() {
                "this".to_string()
            } else {
                format!("this, {arg_names}")
            };
            out.push('\n');
            out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
            out.push_str(&format!("  {signature_return} {method_name}({args}) {{\n"));
            if method.is_async
                && !is_runtime_async_rust_future_compatible_method(
                    method,
                    callback_interfaces,
                    records,
                    enums,
                )
            {
                out.push_str(&format!(
                    "    return Future(() => _bindings().{invoke_name}({invoke_args}));\n"
                ));
            } else if method.return_type.is_some() {
                out.push_str(&format!(
                    "    return _bindings().{invoke_name}({invoke_args});\n"
                ));
            } else {
                out.push_str(&format!("    _bindings().{invoke_name}({invoke_args});\n"));
            }
            out.push_str("  }\n");
        }

        out.push_str("}\n\n");
    }

    for enum_ in enums {
        let enum_name = to_upper_camel(&enum_.name);
        let has_data = enum_.variants.iter().any(|v| !v.fields.is_empty());
        if !has_data && !enum_.is_error {
            out.push_str(&render_doc_comment(enum_.docstring.as_deref(), ""));
            out.push_str(&format!("enum {enum_name} {{\n"));
            for variant in &enum_.variants {
                out.push_str(&render_doc_comment(variant.docstring.as_deref(), "  "));
                out.push_str(&format!(
                    "  {},\n",
                    safe_dart_identifier(&to_lower_camel(&variant.name))
                ));
            }
            out.push_str("}\n\n");
            if !enum_.methods.is_empty() {
                let extension_name = format!("{enum_name}Methods");
                out.push_str(&format!("extension {extension_name} on {enum_name} {{\n"));
                for method in &enum_.methods {
                    let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
                    let invoke_name = safe_dart_identifier(&to_lower_camel(&format!(
                        "{}_{}",
                        dart_identifier(&enum_.name),
                        dart_identifier(&method.name)
                    )));
                    let return_type = method
                        .return_type
                        .as_ref()
                        .map(map_uniffi_type_to_dart)
                        .unwrap_or_else(|| "void".to_string());
                    let signature_return = if method.is_async {
                        format!("Future<{return_type}>")
                    } else {
                        return_type.clone()
                    };
                    let args = render_callable_args_signature(&method.args, enums);
                    let arg_names = render_callable_arg_names(&method.args);
                    let invoke_args = if arg_names.is_empty() {
                        "this".to_string()
                    } else {
                        format!("this, {arg_names}")
                    };
                    out.push('\n');
                    out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
                    out.push_str(&format!("  {signature_return} {method_name}({args}) {{\n"));
                    if method.is_async
                        && !is_runtime_async_rust_future_compatible_method(
                            method,
                            callback_interfaces,
                            records,
                            enums,
                        )
                    {
                        out.push_str(&format!(
                            "    return Future(() => _bindings().{invoke_name}({invoke_args}));\n"
                        ));
                    } else if method.return_type.is_some() {
                        out.push_str(&format!(
                            "    return _bindings().{invoke_name}({invoke_args});\n"
                        ));
                    } else {
                        out.push_str(&format!("    _bindings().{invoke_name}({invoke_args});\n"));
                    }
                    out.push_str("  }\n");
                }
                out.push_str("}\n\n");
            }
            continue;
        }

        out.push_str(&render_doc_comment(enum_.docstring.as_deref(), ""));
        out.push_str(&format!(
            "sealed class {enum_name} {{\n  const {enum_name}();\n"
        ));
        for method in &enum_.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let invoke_name = safe_dart_identifier(&to_lower_camel(&format!(
                "{}_{}",
                dart_identifier(&enum_.name),
                dart_identifier(&method.name)
            )));
            let return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return = if method.is_async {
                format!("Future<{return_type}>")
            } else {
                return_type.clone()
            };
            let args = render_callable_args_signature(&method.args, enums);
            let arg_names = render_callable_arg_names(&method.args);
            let invoke_args = if arg_names.is_empty() {
                "this".to_string()
            } else {
                format!("this, {arg_names}")
            };
            out.push('\n');
            out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
            out.push_str(&format!("  {signature_return} {method_name}({args}) {{\n"));
            if method.is_async
                && !is_runtime_async_rust_future_compatible_method(
                    method,
                    callback_interfaces,
                    records,
                    enums,
                )
            {
                out.push_str(&format!(
                    "    return Future(() => _bindings().{invoke_name}({invoke_args}));\n"
                ));
            } else if method.return_type.is_some() {
                out.push_str(&format!(
                    "    return _bindings().{invoke_name}({invoke_args});\n"
                ));
            } else {
                out.push_str(&format!("    _bindings().{invoke_name}({invoke_args});\n"));
            }
            out.push_str("  }\n");
        }
        out.push_str("}\n\n");
        for variant in &enum_.variants {
            let variant_name = to_upper_camel(&variant.name);
            let class_name = format!("{enum_name}{variant_name}");
            out.push_str(&render_doc_comment(variant.docstring.as_deref(), ""));
            out.push_str(&format!(
                "final class {class_name} extends {enum_name} {{\n"
            ));
            if variant.fields.is_empty() {
                out.push_str(&format!("  const {class_name}();\n"));
            } else {
                out.push_str(&format!("  const {class_name}({{\n"));
                for field in &variant.fields {
                    out.push_str(&render_doc_comment(field.docstring.as_deref(), "    "));
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    out.push_str(&format!("    required this.{field_name},\n"));
                }
                out.push_str("  });\n");
            }
            for field in &variant.fields {
                out.push_str(&render_doc_comment(field.docstring.as_deref(), "  "));
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                out.push_str(&format!(
                    "  final {} {field_name};\n",
                    map_uniffi_type_to_dart(&field.type_)
                ));
            }
            out.push_str("}\n\n");
        }
    }

    for enum_ in enums {
        if !enum_.is_error {
            continue;
        }
        let enum_name = to_upper_camel(&enum_.name);
        let exception_name = format!("{enum_name}Exception");
        out.push_str(&render_doc_comment(enum_.docstring.as_deref(), ""));
        out.push_str(&format!(
            "sealed class {exception_name} implements Exception {{\n  const {exception_name}();\n}}\n\n"
        ));
        for variant in &enum_.variants {
            let variant_name = to_upper_camel(&variant.name);
            let variant_exception = format!("{exception_name}{variant_name}");
            out.push_str(&render_doc_comment(variant.docstring.as_deref(), ""));
            out.push_str(&format!(
                "final class {variant_exception} extends {exception_name} {{\n"
            ));
            if variant.fields.is_empty() {
                out.push_str(&format!("  const {variant_exception}();\n"));
            } else {
                out.push_str(&format!("  const {variant_exception}({{\n"));
                for field in &variant.fields {
                    out.push_str(&render_doc_comment(field.docstring.as_deref(), "    "));
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    out.push_str(&format!("    required this.{field_name},\n"));
                }
                out.push_str("  });\n");
                for field in &variant.fields {
                    out.push_str(&render_doc_comment(field.docstring.as_deref(), "  "));
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    out.push_str(&format!(
                        "  final {} {field_name};\n",
                        map_uniffi_type_to_dart(&field.type_)
                    ));
                }
            }
            out.push_str("}\n\n");
        }
        if emit_uniffi_error_lift_helpers {
            out.push_str(&format!(
                "{exception_name} _uniffiLift{exception_name}(Uint8List bytes) {{\n"
            ));
            out.push_str(&format!(
                "  final {enum_name} value = _uniffiDecode{enum_name}(bytes);\n"
            ));
            for variant in &enum_.variants {
                let variant_name = to_upper_camel(&variant.name);
                let variant_class = format!("{enum_name}{variant_name}");
                let variant_exception = format!("{exception_name}{variant_name}");
                if variant.fields.is_empty() {
                    out.push_str(&format!(
                        "  if (value is {variant_class}) return const {variant_exception}();\n"
                    ));
                } else {
                    out.push_str(&format!("  if (value is {variant_class}) {{\n"));
                    out.push_str(&format!("    return {variant_exception}(\n"));
                    for field in &variant.fields {
                        let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                        out.push_str(&format!("      {field_name}: value.{field_name},\n"));
                    }
                    out.push_str("    );\n");
                    out.push_str("  }\n");
                }
            }
            out.push_str(&format!(
                "  throw StateError('Unknown {enum_name} error variant while lifting exception: $value');\n"
            ));
            out.push_str("}\n\n");
        }
    }

    for enum_ in enums {
        let enum_name = to_upper_camel(&enum_.name);
        let has_data = enum_.variants.iter().any(|v| !v.fields.is_empty());
        out.push_str(&format!(
            "String _encode{enum_name}({enum_name} value) {{\n"
        ));
        if !has_data && !enum_.is_error {
            out.push_str("  return switch (value) {\n");
            for variant in &enum_.variants {
                let variant_name = safe_dart_identifier(&to_lower_camel(&variant.name));
                out.push_str(&format!(
                    "    {enum_name}.{variant_name} => '{variant_name}',\n"
                ));
            }
            out.push_str("  };\n");
            out.push_str("}\n\n");

            out.push_str(&format!("{enum_name} _decode{enum_name}(String raw) {{\n"));
            out.push_str("  switch (raw) {\n");
            for variant in &enum_.variants {
                let variant_name = safe_dart_identifier(&to_lower_camel(&variant.name));
                out.push_str(&format!("    case '{variant_name}':\n"));
                out.push_str(&format!("      return {enum_name}.{variant_name};\n"));
            }
            out.push_str("    default:\n");
            out.push_str(&format!(
                "      throw StateError('Unknown {} variant: $raw');\n",
                enum_name
            ));
            out.push_str("  }\n");
            out.push_str("}\n\n");
            continue;
        }

        for variant in &enum_.variants {
            let variant_class = format!("{}{}", enum_name, to_upper_camel(&variant.name));
            out.push_str(&format!("  if (value is {variant_class}) {{\n"));
            out.push_str("    return jsonEncode({\n");
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            out.push_str(&format!("      'tag': '{variant_tag}',\n"));
            for field in &variant.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let expr = render_json_encode_expr(&format!("value.{field_name}"), &field.type_);
                out.push_str(&format!("      '{field_name}': {expr},\n"));
            }
            out.push_str("    });\n");
            out.push_str("  }\n");
        }
        out.push_str(&format!(
            "  throw StateError('Unknown {} variant instance: $value');\n",
            enum_name
        ));
        out.push_str("}\n\n");

        out.push_str(&format!("{enum_name} _decode{enum_name}(String raw) {{\n"));
        out.push_str(
            "  final Map<String, dynamic> map = jsonDecode(raw) as Map<String, dynamic>;\n",
        );
        out.push_str("  final String? tag = map['tag'] as String?;\n");
        out.push_str("  switch (tag) {\n");
        for variant in &enum_.variants {
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            let variant_class = format!("{}{}", enum_name, to_upper_camel(&variant.name));
            out.push_str(&format!("    case '{variant_tag}':\n"));
            out.push_str(&format!("      return {variant_class}(\n"));
            for field in &variant.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let decode = render_json_decode_expr(&format!("map['{field_name}']"), &field.type_);
                out.push_str(&format!("        {field_name}: {decode},\n"));
            }
            out.push_str("      );\n");
        }
        out.push_str("    default:\n");
        out.push_str(&format!(
            "      throw StateError('Unknown {} variant tag: $tag');\n",
            enum_name
        ));
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        if !enum_.is_error {
            continue;
        }
        let enum_name = to_upper_camel(&enum_.name);
        let exception_name = format!("{enum_name}Exception");
        out.push_str(&format!(
            "String _encode{exception_name}({exception_name} value) {{\n"
        ));
        for variant in &enum_.variants {
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            let variant_name = to_upper_camel(&variant.name);
            let variant_exception = format!("{exception_name}{variant_name}");
            out.push_str(&format!("  if (value is {variant_exception}) {{\n"));
            out.push_str("    return jsonEncode({\n");
            out.push_str(&format!("      'tag': '{variant_tag}',\n"));
            for field in &variant.fields {
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                let expr = render_json_encode_expr(&format!("value.{field_name}"), &field.type_);
                out.push_str(&format!("      '{field_name}': {expr},\n"));
            }
            out.push_str("    });\n");
            out.push_str("  }\n");
        }
        out.push_str(&format!(
            "  throw StateError('Unknown {} exception instance: $value');\n",
            exception_name
        ));
        out.push_str("}\n\n");

        out.push_str(&format!(
            "{exception_name} _decode{exception_name}(Object? raw) {{\n"
        ));
        out.push_str(
            "  final Map<String, dynamic> map = raw is String ? (jsonDecode(raw) as Map<String, dynamic>) : (raw as Map<String, dynamic>);\n",
        );
        out.push_str("  final String? tag = map['tag'] as String?;\n");
        out.push_str("  switch (tag) {\n");
        for variant in &enum_.variants {
            let variant_tag = safe_dart_identifier(&to_lower_camel(&variant.name));
            let variant_name = to_upper_camel(&variant.name);
            let variant_exception = format!("{exception_name}{variant_name}");
            out.push_str(&format!("    case '{variant_tag}':\n"));
            if variant.fields.is_empty() {
                out.push_str(&format!("      return const {variant_exception}();\n"));
            } else {
                out.push_str(&format!("      return {variant_exception}(\n"));
                for field in &variant.fields {
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    let decode =
                        render_json_decode_expr(&format!("map['{field_name}']"), &field.type_);
                    out.push_str(&format!("        {field_name}: {decode},\n"));
                }
                out.push_str("      );\n");
            }
        }
        out.push_str("    default:\n");
        out.push_str(&format!(
            "      throw StateError('Unknown {} exception tag: $tag');\n",
            exception_name
        ));
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        let enum_name = to_upper_camel(&enum_.name);
        out.push_str(&format!("final class {enum_name}FfiCodec {{\n"));
        out.push_str(&format!("  const {enum_name}FfiCodec._();\n\n"));
        out.push_str(&format!(
            "  static String encode({enum_name} value) => _encode{enum_name}(value);\n\n"
        ));
        out.push_str(&format!(
            "  static {enum_name} decode(String raw) => _decode{enum_name}(raw);\n"
        ));
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        if !enum_.is_error {
            continue;
        }
        let enum_name = to_upper_camel(&enum_.name);
        let exception_name = format!("{enum_name}Exception");
        out.push_str(&format!("final class {exception_name}FfiCodec {{\n"));
        out.push_str(&format!("  const {exception_name}FfiCodec._();\n\n"));
        out.push_str(&format!(
            "  static String encode({exception_name} value) => _encode{exception_name}(value);\n\n"
        ));
        out.push_str(&format!(
            "  static {exception_name} decode(Object? raw) => _decode{exception_name}(raw);\n"
        ));
        out.push_str("}\n\n");
    }

    out
}

fn render_json_encode_expr(value_expr: &str, type_: &Type) -> String {
    match type_ {
        Type::Timestamp => format!("{value_expr}.toUtc().microsecondsSinceEpoch"),
        Type::Duration => format!("{value_expr}.inMicroseconds"),
        Type::Bytes => format!("base64Encode({value_expr})"),
        Type::Optional { inner_type } => {
            let inner = render_json_encode_expr("__tmp", inner_type);
            format!(
                "{value_expr} == null ? null : (() {{ final __tmp = {value_expr}; return {inner}; }})()"
            )
        }
        Type::Sequence { inner_type } => {
            let inner = render_json_encode_expr("item", inner_type);
            format!("{value_expr}.map((item) => {inner}).toList()")
        }
        Type::Map {
            key_type: _,
            value_type,
        } => {
            let inner = render_json_encode_expr("value", value_type);
            format!("{value_expr}.map((key, value) => MapEntry(key, {inner}))")
        }
        Type::Custom { builtin, .. } => render_json_encode_expr(value_expr, builtin),
        Type::Record { .. } => format!("{value_expr}.toJson()"),
        Type::Enum { name, .. } => {
            format!("{}FfiCodec.encode({value_expr})", to_upper_camel(name))
        }
        _ => value_expr.to_string(),
    }
}

fn render_json_decode_expr(value_expr: &str, type_: &Type) -> String {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64 => format!("({value_expr} as num).toInt()"),
        Type::Float32 | Type::Float64 => format!("({value_expr} as num).toDouble()"),
        Type::Boolean => format!("{value_expr} as bool"),
        Type::String => format!("{value_expr} as String"),
        Type::Timestamp => format!(
            "DateTime.fromMicrosecondsSinceEpoch(({value_expr} as num).toInt(), isUtc: true)"
        ),
        Type::Duration => format!("Duration(microseconds: ({value_expr} as num).toInt())"),
        Type::Bytes => format!("base64Decode({value_expr} as String)"),
        Type::Optional { inner_type } => {
            let inner = render_json_decode_expr("__tmp", inner_type);
            format!(
                "{value_expr} == null ? null : (() {{ final __tmp = {value_expr}; return {inner}; }})()"
            )
        }
        Type::Sequence { inner_type } => {
            let inner = render_json_decode_expr("item", inner_type);
            format!("({value_expr} as List).map((item) => {inner}).toList()")
        }
        Type::Map {
            key_type: _,
            value_type,
        } => {
            let inner = render_json_decode_expr("value", value_type);
            format!("({value_expr} as Map<String, dynamic>).map((key, value) => MapEntry(key, {inner}))")
        }
        Type::Custom { builtin, .. } => render_json_decode_expr(value_expr, builtin),
        Type::Record { name, .. } => format!(
            "{}.fromJson({value_expr} as Map<String, dynamic>)",
            to_upper_camel(name)
        ),
        Type::Object { name, .. } => format!(
            "{}FfiCodec.lift(({value_expr} as num).toInt())",
            to_upper_camel(name)
        ),
        Type::Enum { name, .. } => {
            format!(
                "{}FfiCodec.decode({value_expr} as String)",
                to_upper_camel(name)
            )
        }
        _ => "throw UnimplementedError('unsupported json decode type')".to_string(),
    }
}

fn render_uniffi_binary_helpers(records: &[UdlRecord], enums: &[UdlEnum]) -> String {
    let mut out = String::new();
    out.push_str("final class _UniFfiBinaryWriter {\n");
    out.push_str("  final BytesBuilder _builder = BytesBuilder(copy: false);\n\n");
    out.push_str("  void writeU8(int value) => _builder.add([value & 0xFF]);\n");
    out.push_str("  void writeI8(int value) => _builder.add([(value) & 0xFF]);\n");
    out.push_str("  void writeU16(int value) {\n");
    out.push_str("    final data = ByteData(2)..setUint16(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeI16(int value) {\n");
    out.push_str("    final data = ByteData(2)..setInt16(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeU32(int value) {\n");
    out.push_str("    final data = ByteData(4)..setUint32(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeI32(int value) {\n");
    out.push_str("    final data = ByteData(4)..setInt32(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeU64(int value) {\n");
    out.push_str("    final data = ByteData(8)..setUint64(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeI64(int value) {\n");
    out.push_str("    final data = ByteData(8)..setInt64(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeF32(double value) {\n");
    out.push_str("    final data = ByteData(4)..setFloat32(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeF64(double value) {\n");
    out.push_str("    final data = ByteData(8)..setFloat64(0, value, Endian.big);\n");
    out.push_str("    _builder.add(data.buffer.asUint8List());\n");
    out.push_str("  }\n");
    out.push_str("  void writeBool(bool value) => writeI8(value ? 1 : 0);\n");
    out.push_str("  void writeBytes(Uint8List bytes) => _builder.add(bytes);\n");
    out.push_str("  void writeString(String value) {\n");
    out.push_str("    final bytes = Uint8List.fromList(utf8.encode(value));\n");
    out.push_str("    writeI32(bytes.length);\n");
    out.push_str("    writeBytes(bytes);\n");
    out.push_str("  }\n\n");
    out.push_str("  Uint8List toBytes() => _builder.takeBytes();\n");
    out.push_str("}\n\n");

    out.push_str("final class _UniFfiBinaryReader {\n");
    out.push_str("  _UniFfiBinaryReader(this._bytes);\n");
    out.push_str("  final Uint8List _bytes;\n");
    out.push_str("  int _offset = 0;\n\n");
    out.push_str("  bool get isDone => _offset == _bytes.length;\n\n");
    out.push_str("  ByteData _readData(int len) {\n");
    out.push_str("    if (_offset + len > _bytes.length) {\n");
    out.push_str("      throw StateError('buffer underflow while decoding UniFFI payload');\n");
    out.push_str("    }\n");
    out.push_str("    final data = ByteData.sublistView(_bytes, _offset, _offset + len);\n");
    out.push_str("    _offset += len;\n");
    out.push_str("    return data;\n");
    out.push_str("  }\n\n");
    out.push_str("  int readU8() => _readData(1).getUint8(0);\n");
    out.push_str("  int readI8() => _readData(1).getInt8(0);\n");
    out.push_str("  int readU16() => _readData(2).getUint16(0, Endian.big);\n");
    out.push_str("  int readI16() => _readData(2).getInt16(0, Endian.big);\n");
    out.push_str("  int readU32() => _readData(4).getUint32(0, Endian.big);\n");
    out.push_str("  int readI32() => _readData(4).getInt32(0, Endian.big);\n");
    out.push_str("  int readU64() => _readData(8).getUint64(0, Endian.big);\n");
    out.push_str("  int readI64() => _readData(8).getInt64(0, Endian.big);\n");
    out.push_str("  double readF32() => _readData(4).getFloat32(0, Endian.big);\n");
    out.push_str("  double readF64() => _readData(8).getFloat64(0, Endian.big);\n");
    out.push_str("  bool readBool() {\n");
    out.push_str("    final value = readI8();\n");
    out.push_str("    if (value == 0) return false;\n");
    out.push_str("    if (value == 1) return true;\n");
    out.push_str("    throw StateError('invalid boolean payload value: $value');\n");
    out.push_str("  }\n");
    out.push_str("  Uint8List readBytes(int len) {\n");
    out.push_str("    if (_offset + len > _bytes.length) {\n");
    out.push_str(
        "      throw StateError('buffer underflow while decoding UniFFI payload bytes');\n",
    );
    out.push_str("    }\n");
    out.push_str("    final out = Uint8List.fromList(_bytes.sublist(_offset, _offset + len));\n");
    out.push_str("    _offset += len;\n");
    out.push_str("    return out;\n");
    out.push_str("  }\n");
    out.push_str("  String readString() {\n");
    out.push_str("    final len = readI32();\n");
    out.push_str("    if (len < 0) {\n");
    out.push_str("      throw StateError('invalid string length in UniFFI payload: $len');\n");
    out.push_str("    }\n");
    out.push_str("    return utf8.decode(readBytes(len));\n");
    out.push_str("  }\n");
    out.push_str("}\n\n");

    for record in records {
        let type_name = to_upper_camel(&record.name);
        out.push_str(&format!(
            "Uint8List _uniffiEncode{type_name}({type_name} value) {{\n"
        ));
        out.push_str("  final writer = _UniFfiBinaryWriter();\n");
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let stmt = render_uniffi_binary_write_statement(
                &field.type_,
                &format!("value.{field_name}"),
                "writer",
                enums,
                "  ",
            );
            out.push_str(&stmt);
        }
        out.push_str("  return writer.toBytes();\n");
        out.push_str("}\n\n");

        out.push_str(&format!(
            "{type_name} _uniffiDecode{type_name}(Uint8List bytes) {{\n"
        ));
        out.push_str("  final reader = _UniFfiBinaryReader(bytes);\n");
        out.push_str(&format!("  final value = {type_name}(\n"));
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let expr = render_uniffi_binary_read_expression(&field.type_, "reader", enums);
            out.push_str(&format!("    {field_name}: {expr},\n"));
        }
        out.push_str("  );\n");
        out.push_str("  if (!reader.isDone) {\n");
        out.push_str(&format!(
            "    throw StateError('extra bytes remaining while decoding {type_name}');\n"
        ));
        out.push_str("  }\n");
        out.push_str("  return value;\n");
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        let type_name = to_upper_camel(&enum_.name);
        let is_flat_enum = !enum_.is_error && enum_.variants.iter().all(|v| v.fields.is_empty());
        out.push_str(&format!(
            "Uint8List _uniffiEncode{type_name}({type_name} value) {{\n"
        ));
        out.push_str("  final writer = _UniFfiBinaryWriter();\n");
        if is_flat_enum {
            out.push_str("  final int tag = switch (value) {\n");
            for (idx, variant) in enum_.variants.iter().enumerate() {
                out.push_str(&format!(
                    "    {type_name}.{} => {},\n",
                    safe_dart_identifier(&to_lower_camel(&variant.name)),
                    idx + 1
                ));
            }
            out.push_str("  };\n");
            out.push_str("  writer.writeI32(tag);\n");
        } else {
            for (idx, variant) in enum_.variants.iter().enumerate() {
                let variant_name = format!("{type_name}{}", to_upper_camel(&variant.name));
                if idx == 0 {
                    out.push_str(&format!("  if (value is {variant_name}) {{\n"));
                } else {
                    out.push_str(&format!("  else if (value is {variant_name}) {{\n"));
                }
                out.push_str(&format!("    writer.writeI32({});\n", idx + 1));
                for field in &variant.fields {
                    let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                    let stmt = render_uniffi_binary_write_statement(
                        &field.type_,
                        &format!("value.{field_name}"),
                        "writer",
                        enums,
                        "    ",
                    );
                    out.push_str(&stmt);
                }
                out.push_str("  }\n");
            }
            out.push_str("  else {\n");
            out.push_str(&format!(
                "    throw StateError('Unknown {type_name} variant instance: $value');\n"
            ));
            out.push_str("  }\n");
        }
        out.push_str("  return writer.toBytes();\n");
        out.push_str("}\n\n");

        out.push_str(&format!(
            "{type_name} _uniffiDecode{type_name}(Uint8List bytes) {{\n"
        ));
        out.push_str("  final reader = _UniFfiBinaryReader(bytes);\n");
        out.push_str("  final int tag = reader.readI32();\n");
        out.push_str(&format!("  final {type_name} value;\n"));
        out.push_str("  switch (tag) {\n");
        for (idx, variant) in enum_.variants.iter().enumerate() {
            out.push_str(&format!("    case {}:\n", idx + 1));
            if is_flat_enum {
                out.push_str(&format!(
                    "      value = {type_name}.{};\n",
                    safe_dart_identifier(&to_lower_camel(&variant.name))
                ));
            } else {
                let variant_name = format!("{type_name}{}", to_upper_camel(&variant.name));
                if variant.fields.is_empty() {
                    out.push_str(&format!("      value = const {variant_name}();\n"));
                } else {
                    out.push_str(&format!("      value = {variant_name}(\n"));
                    for field in &variant.fields {
                        let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                        let expr =
                            render_uniffi_binary_read_expression(&field.type_, "reader", enums);
                        out.push_str(&format!("        {field_name}: {expr},\n"));
                    }
                    out.push_str("      );\n");
                }
            }
            out.push_str("      break;\n");
        }
        out.push_str("    default:\n");
        out.push_str(&format!(
            "      throw StateError('Unknown {type_name} variant tag: $tag');\n"
        ));
        out.push_str("  }\n");
        out.push_str("  if (!reader.isDone) {\n");
        out.push_str(&format!(
            "    throw StateError('extra bytes remaining while decoding {type_name}');\n"
        ));
        out.push_str("  }\n");
        out.push_str("  return value;\n");
        out.push_str("}\n\n");
    }

    out
}

fn render_uniffi_binary_write_statement(
    type_: &Type,
    value_expr: &str,
    writer: &str,
    enums: &[UdlEnum],
    indent: &str,
) -> String {
    if let Type::Custom { builtin, .. } = type_ {
        return render_uniffi_binary_write_statement(builtin, value_expr, writer, enums, indent);
    }
    match type_ {
        Type::UInt8 => format!("{indent}{writer}.writeU8({value_expr});\n"),
        Type::Int8 => format!("{indent}{writer}.writeI8({value_expr});\n"),
        Type::UInt16 => format!("{indent}{writer}.writeU16({value_expr});\n"),
        Type::Int16 => format!("{indent}{writer}.writeI16({value_expr});\n"),
        Type::UInt32 => format!("{indent}{writer}.writeU32({value_expr});\n"),
        Type::Int32 => format!("{indent}{writer}.writeI32({value_expr});\n"),
        Type::UInt64 => format!("{indent}{writer}.writeU64({value_expr});\n"),
        Type::Int64 => format!("{indent}{writer}.writeI64({value_expr});\n"),
        Type::Float32 => format!("{indent}{writer}.writeF32({value_expr});\n"),
        Type::Float64 => format!("{indent}{writer}.writeF64({value_expr});\n"),
        Type::Boolean => format!("{indent}{writer}.writeBool({value_expr});\n"),
        Type::String => format!("{indent}{writer}.writeString({value_expr});\n"),
        Type::Bytes => format!(
            "{indent}{writer}.writeI32({value_expr}.length);\n{indent}{writer}.writeBytes({value_expr});\n"
        ),
        Type::Timestamp => format!(
            "{indent}final Duration __epochOffset = {value_expr}.difference(DateTime.fromMillisecondsSinceEpoch(0, isUtc: true));\n{indent}int __seconds = __epochOffset.inSeconds;\n{indent}int __nanos = (__epochOffset.inMicroseconds.remainder(1000000)) * 1000;\n{indent}if (__nanos < 0) {{ __nanos = -__nanos; }}\n{indent}{writer}.writeI64(__seconds);\n{indent}{writer}.writeU32(__nanos);\n"
        ),
        Type::Duration => format!(
            "{indent}{writer}.writeU64({value_expr}.inSeconds);\n{indent}{writer}.writeU32(({value_expr}.inMicroseconds.remainder(1000000)) * 1000);\n"
        ),
        Type::Optional { inner_type } => {
            let inner_stmt = render_uniffi_binary_write_statement(
                inner_type,
                &format!("{value_expr}!"),
                writer,
                enums,
                &(indent.to_string() + "  "),
            );
            format!(
                "{indent}if ({value_expr} == null) {{\n{indent}  {writer}.writeI8(0);\n{indent}}} else {{\n{indent}  {writer}.writeI8(1);\n{inner_stmt}{indent}}}\n"
            )
        }
        Type::Sequence { inner_type } => {
            let inner_stmt = render_uniffi_binary_write_statement(
                inner_type,
                "item",
                writer,
                enums,
                &(indent.to_string() + "  "),
            );
            format!(
                "{indent}{writer}.writeI32({value_expr}.length);\n{indent}for (final item in {value_expr}) {{\n{inner_stmt}{indent}}}\n"
            )
        }
        Type::Map { value_type, .. } => {
            let key_stmt =
                render_uniffi_binary_write_statement(&Type::String, "entry.key", writer, enums, &(indent.to_string() + "  "));
            let value_stmt = render_uniffi_binary_write_statement(
                value_type,
                "entry.value",
                writer,
                enums,
                &(indent.to_string() + "  "),
            );
            format!(
                "{indent}{writer}.writeI32({value_expr}.length);\n{indent}for (final entry in {value_expr}.entries) {{\n{key_stmt}{value_stmt}{indent}}}\n"
            )
        }
        Type::Record { name, .. } => {
            let record_name = to_upper_camel(name);
            format!(
                "{indent}final Uint8List __encoded = _uniffiEncode{record_name}({value_expr});\n{indent}{writer}.writeI32(__encoded.length);\n{indent}{writer}.writeBytes(__encoded);\n"
            )
        }
        Type::Enum { name, .. } if is_runtime_enum_type(type_, enums) => {
            let enum_name = to_upper_camel(name);
            format!(
                "{indent}final Uint8List __encoded = _uniffiEncode{enum_name}({value_expr});\n{indent}{writer}.writeI32(__encoded.length);\n{indent}{writer}.writeBytes(__encoded);\n"
            )
        }
        _ => format!(
            "{indent}throw UnsupportedError('UniFFI binary write not implemented for {}');\n",
            map_uniffi_type_to_dart(type_)
        ),
    }
}

fn render_uniffi_binary_read_expression(type_: &Type, reader: &str, enums: &[UdlEnum]) -> String {
    if let Type::Custom { builtin, .. } = type_ {
        return render_uniffi_binary_read_expression(builtin, reader, enums);
    }
    match type_ {
        Type::UInt8 => format!("{reader}.readU8()"),
        Type::Int8 => format!("{reader}.readI8()"),
        Type::UInt16 => format!("{reader}.readU16()"),
        Type::Int16 => format!("{reader}.readI16()"),
        Type::UInt32 => format!("{reader}.readU32()"),
        Type::Int32 => format!("{reader}.readI32()"),
        Type::UInt64 => format!("{reader}.readU64()"),
        Type::Int64 => format!("{reader}.readI64()"),
        Type::Float32 => format!("{reader}.readF32()"),
        Type::Float64 => format!("{reader}.readF64()"),
        Type::Boolean => format!("{reader}.readBool()"),
        Type::String => format!("{reader}.readString()"),
        Type::Bytes => format!(
            "(() {{ final int __len = {reader}.readI32(); return {reader}.readBytes(__len); }})()"
        ),
        Type::Optional { inner_type } => {
            let inner = render_uniffi_binary_read_expression(inner_type, reader, enums);
            format!(
                "(() {{ final int __tag = {reader}.readI8(); if (__tag == 0) return null; if (__tag != 1) throw StateError('invalid optional tag: $__tag'); return {inner}; }})()"
            )
        }
        Type::Sequence { inner_type } => {
            let inner = render_uniffi_binary_read_expression(inner_type, reader, enums);
            let inner_type_name = map_uniffi_type_to_dart(inner_type);
            format!(
                "(() {{ final int __len = {reader}.readI32(); final out = <{inner_type_name}>[]; for (var i = 0; i < __len; i++) {{ out.add({inner}); }} return out; }})()"
            )
        }
        Type::Map { value_type, .. } => {
            let value = render_uniffi_binary_read_expression(value_type, reader, enums);
            let value_type_name = map_uniffi_type_to_dart(value_type);
            format!(
                "(() {{ final int __len = {reader}.readI32(); final out = <String, {value_type_name}>{{}}; for (var i = 0; i < __len; i++) {{ final key = {reader}.readString(); final value = {value}; out[key] = value; }} return out; }})()"
            )
        }
        Type::Record { name, .. } => {
            format!(
                "(() {{ final int __len = {reader}.readI32(); final Uint8List __bytes = {reader}.readBytes(__len); return _uniffiDecode{}(__bytes); }})()",
                to_upper_camel(name)
            )
        }
        Type::Enum { name, .. } if is_runtime_enum_type(type_, enums) => {
            format!(
                "(() {{ final int __len = {reader}.readI32(); final Uint8List __bytes = {reader}.readBytes(__len); return _uniffiDecode{}(__bytes); }})()",
                to_upper_camel(name)
            )
        }
        _ => format!(
            "throw UnsupportedError('UniFFI binary read not implemented for {}')",
            map_uniffi_type_to_dart(type_)
        ),
    }
}

fn append_runtime_arg_marshalling(
    arg_name: &str,
    type_: &Type,
    enums: &[UdlEnum],
    pre_call: &mut Vec<String>,
    post_call: &mut Vec<String>,
    call_args: &mut Vec<String>,
) {
    if let Type::Custom { builtin, .. } = type_ {
        append_runtime_arg_marshalling(arg_name, builtin, enums, pre_call, post_call, call_args);
        return;
    }

    if is_runtime_string_type(type_) {
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {arg_name}.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_optional_string_type(type_) {
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {arg_name} == null ? ffi.nullptr : {arg_name}.toNativeUtf8();\n"
        ));
        post_call.push(format!(
            "    if ({native_name} != ffi.nullptr) calloc.free({native_name});\n"
        ));
        call_args.push(native_name);
    } else if is_runtime_map_with_string_key_type(type_) {
        let native_name = format!("{arg_name}Native");
        let json_name = format!("{native_name}Json");
        let payload_expr = render_json_encode_expr(arg_name, type_);
        pre_call.push(format!(
            "    final String {json_name} = jsonEncode({payload_expr});\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {json_name}.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_timestamp_type(type_) {
        call_args.push(format!("{arg_name}.toUtc().microsecondsSinceEpoch"));
    } else if is_runtime_duration_type(type_) {
        call_args.push(format!("{arg_name}.inMicroseconds"));
    } else if is_runtime_bytes_type(type_) {
        let data_name = format!("{arg_name}Data");
        let buffer_ptr_name = format!("{arg_name}BufferPtr");
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<ffi.Uint8> {data_name} = {arg_name}.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({arg_name}.length).setAll(0, {arg_name});\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
        ));
        pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
        pre_call.push(format!(
            "    {buffer_ptr_name}.ref.len = {arg_name}.length;\n"
        ));
        pre_call.push(format!(
            "    final _RustBuffer {native_name} = {buffer_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
        ));
        post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_optional_bytes_type(type_) {
        let data_name = format!("{arg_name}Data");
        let buffer_ptr_name = format!("{arg_name}BufferPtr");
        let opt_ptr_name = format!("{arg_name}OptPtr");
        let native_name = format!("{arg_name}Native");
        let value_name = format!("{arg_name}Value");
        pre_call.push(format!(
            "    final bool {arg_name}IsSome = {arg_name} != null;\n"
        ));
        pre_call.push(format!(
            "    final Uint8List {value_name} = {arg_name} ?? Uint8List(0);\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<ffi.Uint8> {data_name} = !{arg_name}IsSome || {value_name}.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({value_name}.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      {data_name}.asTypedList({value_name}.length).setAll(0, {value_name});\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {buffer_ptr_name} = calloc<_RustBuffer>();\n"
        ));
        pre_call.push(format!("    {buffer_ptr_name}.ref.data = {data_name};\n"));
        pre_call.push(format!(
            "    {buffer_ptr_name}.ref.len = {arg_name}IsSome ? {value_name}.length : 0;\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBufferOpt> {opt_ptr_name} = calloc<_RustBufferOpt>();\n"
        ));
        pre_call.push(format!(
            "    {opt_ptr_name}.ref.isSome = {arg_name}IsSome ? 1 : 0;\n"
        ));
        pre_call.push(format!(
            "    {opt_ptr_name}.ref.value = {buffer_ptr_name}.ref;\n"
        ));
        pre_call.push(format!(
            "    final _RustBufferOpt {native_name} = {opt_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) calloc.free({data_name});\n"
        ));
        post_call.push(format!("    calloc.free({buffer_ptr_name});\n"));
        post_call.push(format!("    calloc.free({opt_ptr_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_sequence_bytes_type(type_) {
        let data_name = format!("{arg_name}Data");
        let vec_ptr_name = format!("{arg_name}VecPtr");
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBuffer> {data_name} = {arg_name}.isEmpty ? ffi.nullptr : calloc<_RustBuffer>({arg_name}.length);\n"
        ));
        pre_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      for (var i = 0; i < {arg_name}.length; i++) {{\n        final item = {arg_name}[i];\n        final ffi.Pointer<ffi.Uint8> itemData = item.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>(item.length);\n        if (itemData != ffi.nullptr) {{\n          itemData.asTypedList(item.length).setAll(0, item);\n        }}\n        ({data_name} + i).ref\n          ..data = itemData\n          ..len = item.length;\n      }}\n    }}\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<_RustBufferVec> {vec_ptr_name} = calloc<_RustBufferVec>();\n"
        ));
        pre_call.push(format!(
            "    {vec_ptr_name}.ref\n      ..data = {data_name}\n      ..len = {arg_name}.length;\n"
        ));
        pre_call.push(format!(
            "    final _RustBufferVec {native_name} = {vec_ptr_name}.ref;\n"
        ));
        post_call.push(format!(
            "    if ({data_name} != ffi.nullptr) {{\n      for (var i = 0; i < {arg_name}.length; i++) {{\n        final data = ({data_name} + i).ref.data;\n        if (data != ffi.nullptr) calloc.free(data);\n      }}\n      calloc.free({data_name});\n    }}\n"
        ));
        post_call.push(format!("    calloc.free({vec_ptr_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_record_type(type_) {
        let native_name = format!("{arg_name}Native");
        pre_call.push(format!(
            "    final String {native_name}Json = jsonEncode({arg_name}.toJson());\n"
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_enum_type(type_, enums) {
        let native_name = format!("{arg_name}Native");
        let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
        pre_call.push(format!(
            "    final String {native_name}Json = {}FfiCodec.encode({arg_name});\n",
            to_upper_camel(enum_name)
        ));
        pre_call.push(format!(
            "    final ffi.Pointer<Utf8> {native_name} = {native_name}Json.toNativeUtf8();\n"
        ));
        post_call.push(format!("    calloc.free({native_name});\n"));
        call_args.push(native_name);
    } else if is_runtime_object_type(type_) {
        let handle_name = format!("{arg_name}Handle");
        let object_name = object_name_from_type(type_).unwrap_or("Object");
        pre_call.push(format!(
            "    final int {handle_name} = {}FfiCodec.lower({arg_name});\n",
            to_upper_camel(object_name)
        ));
        call_args.push(handle_name);
    } else {
        call_args.push(arg_name.to_string());
    }
}

fn render_callback_interfaces(callback_interfaces: &[UdlCallbackInterface]) -> String {
    if callback_interfaces.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for callback_interface in callback_interfaces {
        let class_name = to_upper_camel(&callback_interface.name);
        out.push_str(&render_doc_comment(
            callback_interface.docstring.as_deref(),
            "",
        ));
        out.push_str(&format!("abstract interface class {class_name} {{\n"));
        for method in &callback_interface.methods {
            let value_return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return_type = if method.is_async {
                format!("Future<{value_return_type}>")
            } else {
                value_return_type
            };
            let args = method
                .args
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        map_uniffi_type_to_dart(&a.type_),
                        safe_dart_identifier(&to_lower_camel(&a.name))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
            out.push_str(&format!(
                "  {signature_return_type} {method_name}({args});\n"
            ));
        }
        out.push_str("}\n\n");
    }
    out
}

fn render_callback_bridges(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let used = callback_interfaces_used_for_runtime(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    if used.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let has_async_callback_methods = used
        .iter()
        .any(|cb| cb.methods.iter().any(|method| method.is_async));
    if has_async_callback_methods {
        out.push_str("final class _ForeignFutureDroppedCallbackStruct extends ffi.Struct {\n");
        out.push_str("  @ffi.Uint64()\n");
        out.push_str("  external int handle;\n\n");
        out.push_str(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 handle)>> callback;\n",
        );
        out.push_str("}\n\n");
    }

    for callback_interface in used {
        let class_name = to_upper_camel(&callback_interface.name);
        let vtable_name = callback_vtable_struct_name(&callback_interface.name);
        let bridge_name = callback_bridge_class_name(&callback_interface.name);
        for method in &callback_interface.methods {
            if !method.is_async {
                continue;
            }
            let result_struct_name =
                callback_async_result_struct_name(&callback_interface.name, &method.name);
            out.push_str(&format!(
                "final class {result_struct_name} extends ffi.Struct {{\n"
            ));
            if let Some(return_type) = method.return_type.as_ref() {
                let return_field =
                    render_callback_async_result_return_field(return_type, records, enums)
                        .expect("validated runtime callback async return type");
                out.push_str(&return_field);
            }
            out.push_str("  external _RustCallStatus callStatus;\n");
            out.push_str("}\n\n");
        }
        out.push_str(&format!(
            "final class {vtable_name} extends ffi.Struct {{\n"
        ));
        out.push_str(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 handle)>> uniffiFree;\n\n",
        );
        out.push_str(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Uint64 Function(ffi.Uint64 handle)>> uniffiClone;\n\n",
        );
        for method in &callback_interface.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let mut ffi_args = vec!["ffi.Uint64 handle".to_string()];
            for arg in &method.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                let arg_native = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    .expect("validated runtime callback arg type");
                ffi_args.push(format!("{arg_native} {arg_name}"));
            }
            if method.is_async {
                let result_struct_name =
                    callback_async_result_struct_name(&callback_interface.name, &method.name);
                ffi_args.push(format!(
                    "ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, {result_struct_name} result)>> uniffiFutureCallback"
                ));
                ffi_args.push("ffi.Uint64 callbackData".to_string());
                ffi_args.push(
                    "ffi.Pointer<_ForeignFutureDroppedCallbackStruct> uniffiOutDroppedCallback"
                        .to_string(),
                );
            } else {
                if let Some(return_type) = method.return_type.as_ref() {
                    let out_type = map_runtime_native_ffi_type(return_type, records, enums)
                        .expect("validated runtime callback return type");
                    ffi_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
                } else {
                    ffi_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
                }
                ffi_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());
            }
            out.push_str(&format!(
                "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function({})>> {method_name};\n\n",
                ffi_args.join(", ")
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("final class {bridge_name} {{\n"));
        out.push_str(&format!("  {bridge_name}._();\n"));
        out.push_str(&format!(
            "  static final {bridge_name} instance = {bridge_name}._();\n\n"
        ));
        out.push_str(&format!(
            "  final Map<int, {class_name}> _callbacks = <int, {class_name}>{{}};\n"
        ));
        out.push_str("  final Map<int, int> _refCounts = <int, int>{};\n");
        out.push_str("  int _nextHandle = 1;\n\n");
        let has_async_methods = callback_interface.methods.iter().any(|m| m.is_async);
        if has_async_methods {
            out.push_str("  final Map<int, bool> _droppedFutures = <int, bool>{};\n");
            out.push_str("  int _nextDroppedFutureHandle = 1;\n\n");
            out.push_str("  int beginDroppedFutureTracking() {\n");
            out.push_str("    final int handle = _nextDroppedFutureHandle++;\n");
            out.push_str("    _droppedFutures[handle] = false;\n");
            out.push_str("    return handle;\n");
            out.push_str("  }\n\n");
            out.push_str("  void markDroppedFuture(int handle) {\n");
            out.push_str("    if (_droppedFutures.containsKey(handle)) {\n");
            out.push_str("      _droppedFutures[handle] = true;\n");
            out.push_str("    }\n");
            out.push_str("  }\n\n");
            out.push_str("  bool isDroppedFuture(int handle) {\n");
            out.push_str("    return _droppedFutures[handle] ?? true;\n");
            out.push_str("  }\n\n");
            out.push_str("  void finishDroppedFuture(int handle) {\n");
            out.push_str("    _droppedFutures.remove(handle);\n");
            out.push_str("  }\n\n");
        }
        out.push_str(&format!("  int register({class_name} callback) {{\n"));
        out.push_str("    final int handle = _nextHandle++;\n");
        out.push_str("    _callbacks[handle] = callback;\n");
        out.push_str("    _refCounts[handle] = 1;\n");
        out.push_str("    return handle;\n");
        out.push_str("  }\n\n");
        out.push_str("  void release(int handle) {\n");
        out.push_str("    final int? refs = _refCounts[handle];\n");
        out.push_str("    if (refs == null) {\n");
        out.push_str("      return;\n");
        out.push_str("    }\n");
        out.push_str("    if (refs <= 1) {\n");
        out.push_str("      _refCounts.remove(handle);\n");
        out.push_str("      _callbacks.remove(handle);\n");
        out.push_str("      return;\n");
        out.push_str("    }\n");
        out.push_str("    _refCounts[handle] = refs - 1;\n");
        out.push_str("  }\n\n");
        out.push_str("  int cloneHandle(int handle) {\n");
        out.push_str("    final int? refs = _refCounts[handle];\n");
        out.push_str("    if (refs == null) {\n");
        out.push_str("      throw StateError('Invalid callback handle: $handle');\n");
        out.push_str("    }\n");
        out.push_str("    _refCounts[handle] = refs + 1;\n");
        out.push_str("    return handle;\n");
        out.push_str("  }\n\n");
        out.push_str(&format!(
            "  {class_name}? lookup(int handle) => _callbacks[handle];\n\n"
        ));
        out.push_str(
            "  static final ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)> _freeNative = ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
        );
        out.push_str("    instance.release(handle);\n");
        out.push_str("  });\n\n");
        out.push_str(
            "  static final ffi.NativeCallable<ffi.Uint64 Function(ffi.Uint64 handle)> _cloneNative = ffi.NativeCallable<ffi.Uint64 Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
        );
        out.push_str("    return instance.cloneHandle(handle);\n");
        out.push_str("  }, exceptionalReturn: 0);\n\n");
        if has_async_methods {
            out.push_str(
                "  static final ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)> _futureDroppedNative = ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
            );
            out.push_str("    instance.markDroppedFuture(handle);\n");
            out.push_str("  });\n\n");
        }

        for method in &callback_interface.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            let native_callable_name = format!("_{}Native", method_name);
            let mut ffi_args = vec!["ffi.Uint64 handle".to_string()];
            let mut dart_args = vec!["int handle".to_string()];
            let mut callback_args = Vec::new();
            for arg in &method.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                let arg_native = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    .expect("validated runtime callback arg type");
                let arg_dart = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    .expect("validated runtime callback arg type");
                ffi_args.push(format!("{arg_native} {arg_name}"));
                dart_args.push(format!("{arg_dart} {arg_name}"));
                callback_args.push(render_callback_arg_decode_expr(
                    &arg.type_, &arg_name, records, enums,
                ));
            }
            if method.is_async {
                let result_struct_name =
                    callback_async_result_struct_name(&callback_interface.name, &method.name);
                let async_result_uses_utf8_ptr = method
                    .return_type
                    .as_ref()
                    .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums));
                ffi_args.push(format!(
                    "ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, {result_struct_name} result)>> uniffiFutureCallback"
                ));
                ffi_args.push("ffi.Uint64 callbackData".to_string());
                ffi_args.push(
                    "ffi.Pointer<_ForeignFutureDroppedCallbackStruct> uniffiOutDroppedCallback"
                        .to_string(),
                );
                dart_args.push(format!(
                    "ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, {result_struct_name} result)>> uniffiFutureCallback"
                ));
                dart_args.push("int callbackData".to_string());
                dart_args.push(
                    "ffi.Pointer<_ForeignFutureDroppedCallbackStruct> uniffiOutDroppedCallback"
                        .to_string(),
                );

                out.push_str(&format!(
                    "  static final ffi.NativeCallable<ffi.Void Function({})> {native_callable_name} = ffi.NativeCallable<ffi.Void Function({})>.isolateLocal(({}) {{\n",
                    ffi_args.join(", "),
                    ffi_args.join(", "),
                    dart_args.join(", ")
                ));
                out.push_str(&format!(
                    "    final {class_name}? callback = instance.lookup(handle);\n"
                ));
                out.push_str("    final complete = uniffiFutureCallback.asFunction<void Function(int callbackData, ");
                out.push_str(&result_struct_name);
                out.push_str(" result)>();\n");
                out.push_str(
                    "    final int droppedHandle = instance.beginDroppedFutureTracking();\n",
                );
                out.push_str("    if (uniffiOutDroppedCallback != ffi.nullptr) {\n");
                out.push_str("      uniffiOutDroppedCallback.ref\n");
                out.push_str("        ..handle = droppedHandle\n");
                out.push_str("        ..callback = _futureDroppedNative.nativeFunction;\n");
                out.push_str("    }\n");
                out.push_str("    if (callback == null) {\n");
                out.push_str(&format!(
                    "      final ffi.Pointer<{result_struct_name}> resultPtr = calloc<{result_struct_name}>();\n"
                ));
                if let Some(return_type) = method.return_type.as_ref() {
                    let default_value =
                        callback_async_default_return_expr(return_type, records, enums);
                    out.push_str(&format!(
                        "      resultPtr.ref.returnValue = {default_value};\n"
                    ));
                }
                out.push_str("      resultPtr.ref.callStatus\n");
                out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                out.push_str("        ..errorBuf = 'Invalid callback handle'.toNativeUtf8();\n");
                out.push_str(
                    "      final bool dropped = instance.isDroppedFuture(droppedHandle);\n",
                );
                out.push_str("      if (!dropped) {\n");
                out.push_str("        complete(callbackData, resultPtr.ref);\n");
                out.push_str("      } else {\n");
                out.push_str("        if (resultPtr.ref.callStatus.errorBuf != ffi.nullptr) {\n");
                out.push_str("          calloc.free(resultPtr.ref.callStatus.errorBuf);\n");
                out.push_str("        }\n");
                if async_result_uses_utf8_ptr {
                    out.push_str("        if (resultPtr.ref.returnValue != ffi.nullptr) {\n");
                    out.push_str("          calloc.free(resultPtr.ref.returnValue);\n");
                    out.push_str("        }\n");
                }
                out.push_str("      }\n");
                out.push_str("      instance.finishDroppedFuture(droppedHandle);\n");
                out.push_str("      calloc.free(resultPtr);\n");
                out.push_str("      return;\n");
                out.push_str("    }\n");
                out.push_str("    () async {\n");
                out.push_str(&format!(
                    "      final ffi.Pointer<{result_struct_name}> resultPtr = calloc<{result_struct_name}>();\n"
                ));
                if let Some(return_type) = method.return_type.as_ref() {
                    let default_value =
                        callback_async_default_return_expr(return_type, records, enums);
                    out.push_str(&format!(
                        "      resultPtr.ref.returnValue = {default_value};\n"
                    ));
                }
                out.push_str("      try {\n");
                if let Some(return_type) = method.return_type.as_ref() {
                    out.push_str(&format!(
                        "        final result = await callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                    let encoded =
                        render_callback_return_encode_expr(return_type, "result", records, enums);
                    out.push_str(&format!("        resultPtr.ref.returnValue = {encoded};\n"));
                } else {
                    out.push_str(&format!(
                        "        await callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                }
                out.push_str("        resultPtr.ref.callStatus\n");
                out.push_str("          ..code = _rustCallStatusSuccess\n");
                out.push_str("          ..errorBuf = ffi.nullptr;\n");
                out.push_str("      } catch (err) {\n");
                if method.throws_type.is_some() {
                    if let Some(exception_name) = method
                        .throws_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .map(|name| format!("{}Exception", to_upper_camel(name)))
                    {
                        out.push_str(&format!("        if (err is {exception_name}) {{\n"));
                        out.push_str("          resultPtr.ref.callStatus\n");
                        out.push_str("            ..code = _rustCallStatusError\n");
                        out.push_str(&format!(
                            "            ..errorBuf = {exception_name}FfiCodec.encode(err).toNativeUtf8();\n"
                        ));
                        out.push_str("        } else {\n");
                        out.push_str("          resultPtr.ref.callStatus\n");
                        out.push_str("            ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("            ..errorBuf = err.toString().toNativeUtf8();\n");
                        out.push_str("        }\n");
                    } else {
                        out.push_str("        resultPtr.ref.callStatus\n");
                        out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("          ..errorBuf = err.toString().toNativeUtf8();\n");
                    }
                } else {
                    out.push_str("        resultPtr.ref.callStatus\n");
                    out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                    out.push_str("          ..errorBuf = err.toString().toNativeUtf8();\n");
                }
                out.push_str("      } finally {\n");
                out.push_str(
                    "        final bool dropped = instance.isDroppedFuture(droppedHandle);\n",
                );
                out.push_str("        if (!dropped) {\n");
                out.push_str("          complete(callbackData, resultPtr.ref);\n");
                out.push_str("        } else {\n");
                out.push_str("          if (resultPtr.ref.callStatus.errorBuf != ffi.nullptr) {\n");
                out.push_str("            calloc.free(resultPtr.ref.callStatus.errorBuf);\n");
                out.push_str("          }\n");
                if async_result_uses_utf8_ptr {
                    out.push_str("          if (resultPtr.ref.returnValue != ffi.nullptr) {\n");
                    out.push_str("            calloc.free(resultPtr.ref.returnValue);\n");
                    out.push_str("          }\n");
                }
                out.push_str("        }\n");
                out.push_str("        instance.finishDroppedFuture(droppedHandle);\n");
                out.push_str("        calloc.free(resultPtr);\n");
                out.push_str("      }\n");
                out.push_str("    }();\n");
            } else {
                if let Some(return_type) = method.return_type.as_ref() {
                    let out_type = map_runtime_native_ffi_type(return_type, records, enums)
                        .expect("validated runtime callback return type");
                    ffi_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
                    dart_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
                } else {
                    ffi_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
                    dart_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
                }
                ffi_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());
                dart_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());

                out.push_str(&format!(
                    "  static final ffi.NativeCallable<ffi.Void Function({})> {native_callable_name} = ffi.NativeCallable<ffi.Void Function({})>.isolateLocal(({}) {{\n",
                    ffi_args.join(", "),
                    ffi_args.join(", "),
                    dart_args.join(", ")
                ));
                out.push_str(&format!(
                    "    final {class_name}? callback = instance.lookup(handle);\n"
                ));
                out.push_str("    if (callback == null) {\n");
                out.push_str("      outStatus.ref\n");
                out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                out.push_str("        ..errorBuf = 'Invalid callback handle'.toNativeUtf8();\n");
                out.push_str("      return;\n");
                out.push_str("    }\n");
                out.push_str("    try {\n");
                if let Some(return_type) = method.return_type.as_ref() {
                    out.push_str(&format!(
                        "      final result = callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                    let encoded =
                        render_callback_return_encode_expr(return_type, "result", records, enums);
                    out.push_str(&format!("      outReturn.value = {encoded};\n"));
                } else {
                    out.push_str(&format!(
                        "      callback.{method_name}({});\n",
                        callback_args.join(", ")
                    ));
                }
                out.push_str("      outStatus.ref\n");
                out.push_str("        ..code = _rustCallStatusSuccess\n");
                out.push_str("        ..errorBuf = ffi.nullptr;\n");
                out.push_str("    } catch (err) {\n");
                if method.throws_type.is_some() {
                    if let Some(exception_name) = method
                        .throws_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .map(|name| format!("{}Exception", to_upper_camel(name)))
                    {
                        out.push_str(&format!("      if (err is {exception_name}) {{\n"));
                        out.push_str("        outStatus.ref\n");
                        out.push_str("          ..code = _rustCallStatusError\n");
                        out.push_str(&format!(
                            "          ..errorBuf = {exception_name}FfiCodec.encode(err).toNativeUtf8();\n"
                        ));
                        out.push_str("      } else {\n");
                        out.push_str("        outStatus.ref\n");
                        out.push_str("          ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("          ..errorBuf = err.toString().toNativeUtf8();\n");
                        out.push_str("      }\n");
                    } else {
                        out.push_str("      outStatus.ref\n");
                        out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                        out.push_str("        ..errorBuf = err.toString().toNativeUtf8();\n");
                    }
                } else {
                    out.push_str("      outStatus.ref\n");
                    out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
                    out.push_str("        ..errorBuf = err.toString().toNativeUtf8();\n");
                }
                out.push_str("    }\n");
            }
            out.push_str("  });\n\n");
        }

        out.push_str(&format!(
            "  static ffi.Pointer<{vtable_name}> createVTable() {{\n"
        ));
        out.push_str(&format!(
            "    final ffi.Pointer<{vtable_name}> vtablePtr = calloc<{vtable_name}>();\n"
        ));
        out.push_str("    vtablePtr.ref\n");
        out.push_str("      ..uniffiFree = _freeNative.nativeFunction\n");
        out.push_str("      ..uniffiClone = _cloneNative.nativeFunction\n");
        for method in &callback_interface.methods {
            let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
            out.push_str(&format!(
                "      ..{method_name} = _{method_name}Native.nativeFunction\n"
            ));
        }
        out.push_str("    ;\n");
        out.push_str("    return vtablePtr;\n");
        out.push_str("  }\n");
        out.push_str("}\n\n");
    }

    out
}

fn render_bound_methods(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_namespace: &str,
    local_module_path: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let mut out = String::new();
    let mut runtime_functions = functions.to_vec();
    for record in records {
        for method in &record.methods {
            let mut args = vec![UdlArg {
                name: "self".to_string(),
                type_: Type::Record {
                    module_path: local_module_path.to_string(),
                    name: record.name.clone(),
                },
                docstring: None,
                default: None,
            }];
            args.extend(method.args.clone());
            runtime_functions.push(UdlFunction {
                name: format!(
                    "{}_{}",
                    dart_identifier(&record.name),
                    dart_identifier(&method.name)
                ),
                ffi_symbol: method.ffi_symbol.clone(),
                ffi_arg_types: method.ffi_arg_types.clone(),
                ffi_return_type: method.ffi_return_type.clone(),
                ffi_has_rust_call_status: method.ffi_has_rust_call_status,
                runtime_unsupported: method.runtime_unsupported.clone(),
                docstring: method.docstring.clone(),
                is_async: method.is_async,
                return_type: method.return_type.clone(),
                throws_type: method.throws_type.clone(),
                args,
            });
        }
    }
    for enum_ in enums {
        for method in &enum_.methods {
            let mut args = vec![UdlArg {
                name: "self".to_string(),
                type_: Type::Enum {
                    module_path: local_module_path.to_string(),
                    name: enum_.name.clone(),
                },
                docstring: None,
                default: None,
            }];
            args.extend(method.args.clone());
            runtime_functions.push(UdlFunction {
                name: format!(
                    "{}_{}",
                    dart_identifier(&enum_.name),
                    dart_identifier(&method.name)
                ),
                ffi_symbol: method.ffi_symbol.clone(),
                ffi_arg_types: method.ffi_arg_types.clone(),
                ffi_return_type: method.ffi_return_type.clone(),
                ffi_has_rust_call_status: method.ffi_has_rust_call_status,
                runtime_unsupported: method.runtime_unsupported.clone(),
                docstring: method.docstring.clone(),
                is_async: method.is_async,
                return_type: method.return_type.clone(),
                throws_type: method.throws_type.clone(),
                args,
            });
        }
    }
    let has_runtime_ffibuffer_fallback = runtime_functions.iter().any(|f| {
        f.runtime_unsupported.is_some()
            && (is_ffibuffer_eligible_function(f)
                || is_runtime_unsupported_async_ffibuffer_eligible_function(f))
    }) || objects.iter().any(|o| {
        o.constructors
            .iter()
            .any(|c| c.runtime_unsupported.is_some() && is_ffibuffer_eligible_object_constructor(c))
            || o.methods
                .iter()
                .any(|m| m.runtime_unsupported.is_some() && is_ffibuffer_eligible_object_member(m))
    });
    let callback_runtime_interfaces = callback_interfaces_used_for_runtime(
        &runtime_functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    let needs_async_rust_future = has_runtime_async_rust_future_support(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    );
    let needs_string_free =
        needs_async_rust_future
            || functions.iter().any(|f| {
                f.runtime_unsupported.is_none()
                    && is_runtime_ffi_compatible_function(f, records, enums)
                    && (f.returns_runtime_string()
                        || f.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        })
                        || is_runtime_throwing_ffi_compatible_function(
                            f,
                            callback_interfaces,
                            records,
                            enums,
                        )
                        || f.return_type
                            .as_ref()
                            .is_some_and(|t| is_runtime_record_or_enum_string_type(t, enums)))
            })
            || objects.iter().any(|o| {
                o.methods.iter().any(|m| {
                    m.runtime_unsupported.is_none()
                        && (m.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        }) || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
                })
            })
            || records.iter().any(|r| {
                r.methods.iter().any(|m| {
                    m.runtime_unsupported.is_none()
                        && (m.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        }) || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
                })
            })
            || enums.iter().any(|e| {
                e.methods.iter().any(|m| {
                    m.runtime_unsupported.is_none()
                        && (m.return_type.as_ref().is_some_and(|t| {
                            is_runtime_utf8_pointer_marshaled_type(t, records, enums)
                        }) || (m.throws_type.is_some()
                            && m.return_type
                                .as_ref()
                                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                                .unwrap_or(true)
                            && m.args
                                .iter()
                                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))))
                })
            });
    let needs_bytes_free = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums) && f.returns_runtime_bytes()
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_bytes_like_type)
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_bytes_like_type)
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_bytes_like_type)
        })
    });
    let needs_bytes_vec_free = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            && f.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.return_type
                .as_ref()
                .is_some_and(is_runtime_sequence_bytes_type)
        })
    });

    if needs_string_free {
        out.push('\n');
        out.push_str("  late final void Function(ffi.Pointer<Utf8>) _rustStringFree = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<Utf8>), void Function(ffi.Pointer<Utf8>)>('rust_string_free');\n");
    }
    if needs_bytes_free {
        out.push('\n');
        out.push_str("  late final void Function(_RustBuffer) _rustBytesFree = _lib.lookupFunction<ffi.Void Function(_RustBuffer), void Function(_RustBuffer)>('rust_bytes_free');\n");
    }
    if needs_bytes_vec_free {
        out.push('\n');
        out.push_str("  late final void Function(_RustBufferVec) _rustBytesVecFree = _lib.lookupFunction<ffi.Void Function(_RustBufferVec), void Function(_RustBufferVec)>('rust_bytes_vec_free');\n");
    }
    if has_runtime_ffibuffer_fallback {
        out.push('\n');
        out.push_str(&format!(
            "  late final _UniFfiRustBuffer Function(_UniFfiForeignBytes bytes, ffi.Pointer<_UniFfiRustCallStatus> outStatus) _uniFfiRustBufferFromBytes = _lib.lookupFunction<_UniFfiRustBuffer Function(_UniFfiForeignBytes bytes, ffi.Pointer<_UniFfiRustCallStatus> outStatus), _UniFfiRustBuffer Function(_UniFfiForeignBytes bytes, ffi.Pointer<_UniFfiRustCallStatus> outStatus)>('ffi_{ffi_namespace}_rustbuffer_from_bytes');\n"
        ));
        out.push_str(&format!(
            "  late final void Function(_UniFfiRustBuffer buf, ffi.Pointer<_UniFfiRustCallStatus> outStatus) _uniFfiRustBufferFree = _lib.lookupFunction<ffi.Void Function(_UniFfiRustBuffer buf, ffi.Pointer<_UniFfiRustCallStatus> outStatus), void Function(_UniFfiRustBuffer buf, ffi.Pointer<_UniFfiRustCallStatus> outStatus)>('ffi_{ffi_namespace}_rustbuffer_free');\n"
        ));
    }
    for callback_interface in &callback_runtime_interfaces {
        let callback_name = &callback_interface.name;
        let vtable_name = callback_vtable_struct_name(callback_name);
        let init_field = callback_init_field_name(callback_name);
        let init_done_field = callback_init_done_field_name(callback_name);
        let vtable_field = callback_vtable_field_name(callback_name);
        let bridge_name = callback_bridge_class_name(callback_name);
        let init_symbol = callback_init_symbol(callback_name);
        out.push('\n');
        out.push_str(&format!(
            "  late final void Function(ffi.Pointer<{vtable_name}>) {init_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<{vtable_name}>), void Function(ffi.Pointer<{vtable_name}>)>('{init_symbol}');\n"
        ));
        out.push_str(&format!(
            "  late final ffi.Pointer<{vtable_name}> {vtable_field} = {bridge_name}.createVTable();\n"
        ));
        out.push_str(&format!(
            "  late final bool {init_done_field} = (() {{\n    {init_field}({vtable_field});\n    return true;\n  }})();\n"
        ));
    }

    for function in &runtime_functions {
        let method_name = safe_dart_identifier(&to_lower_camel(&function.name));
        if let Some(reason) = function.runtime_unsupported.as_ref() {
            let ffibuffer_eligible =
                is_ffibuffer_eligible_function(function) && function.ffi_symbol.is_some();
            let runtime_unsupported_async_ffibuffer_eligible =
                is_runtime_unsupported_async_ffibuffer_eligible_function(function);
            if runtime_unsupported_async_ffibuffer_eligible {
                let value_return_type = function
                    .return_type
                    .as_ref()
                    .map(map_uniffi_type_to_dart)
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = format!("Future<{value_return_type}>");
                let dart_sig = function
                    .args
                    .iter()
                    .map(|a| {
                        format!(
                            "{} {}",
                            map_uniffi_type_to_dart(&a.type_),
                            safe_dart_identifier(&to_lower_camel(&a.name))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let escaped_reason = reason.replace('\'', "\\'");
                let method_field = format!("_{method_name}FfiBuffer");
                let poll_field = format!("{method_field}RustFuturePoll");
                let cancel_field = format!("{method_field}RustFutureCancel");
                let complete_field = format!("{method_field}RustFutureComplete");
                let free_field = format!("{method_field}RustFutureFree");
                let ffi_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
                let ffibuffer_symbol = ffibuffer_symbol_name(ffi_symbol);
                let ffi_start_return_type =
                    function.ffi_return_type.clone().unwrap_or(FfiType::UInt64);
                let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_start_return_type)
                else {
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                };
                let Some(async_spec) =
                    async_rust_future_spec_from_uniffi_return_type(function.return_type.as_ref())
                else {
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                };
                let ffi_arg_types = if function.ffi_arg_types.len() == function.args.len() {
                    function.ffi_arg_types.clone()
                } else {
                    function
                        .args
                        .iter()
                        .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_))
                        .collect::<Vec<_>>()
                };
                let mut arg_ffi_offsets = Vec::new();
                let mut arg_cursor = 0usize;
                let mut signature_compatible = ffi_arg_types.len() == function.args.len();
                if signature_compatible {
                    for ffi_type in &ffi_arg_types {
                        let Some(size) = ffibuffer_element_count(ffi_type) else {
                            signature_compatible = false;
                            break;
                        };
                        arg_ffi_offsets.push(arg_cursor);
                        arg_cursor += size;
                    }
                }
                let start_return_union_field =
                    ffibuffer_primitive_union_field(&ffi_start_return_type);
                if !signature_compatible || start_return_union_field.is_none() {
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                }
                let start_return_union_field = start_return_union_field.unwrap_or("u64");
                let poll_symbol =
                    format!("ffi_{ffi_namespace}_rust_future_poll_{}", async_spec.suffix);
                let cancel_symbol = format!(
                    "ffi_{ffi_namespace}_rust_future_cancel_{}",
                    async_spec.suffix
                );
                let complete_symbol = format!(
                    "ffi_{ffi_namespace}_rust_future_complete_{}",
                    async_spec.suffix
                );
                let free_symbol =
                    format!("ffi_{ffi_namespace}_rust_future_free_{}", async_spec.suffix);
                let complete_native_sig = format!(
                    "{} Function(ffi.Uint64 handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)",
                    async_spec.complete_native_type
                );
                let complete_dart_sig = format!(
                    "{} Function(int handle, ffi.Pointer<_UniFfiRustCallStatus> outStatus)",
                    async_spec.complete_dart_type
                );

                out.push('\n');
                out.push_str(&format!(
                    "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {method_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{ffibuffer_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData) {poll_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, ffi.Uint64 callbackData), void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData)>('{poll_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle) {cancel_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{cancel_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final {complete_dart_sig} {complete_field} = _lib.lookupFunction<{complete_native_sig}, {complete_dart_sig}>('{complete_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{free_symbol}');\n"
                ));
                out.push('\n');
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                    return_ffi_elements + 4
                ));
                out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                out.push_str("    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n");
                out.push_str("    try {\n");

                for ((arg, ffi_type), offset) in function
                    .args
                    .iter()
                    .zip(ffi_arg_types.iter())
                    .zip(arg_ffi_offsets.iter())
                {
                    let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                    match ffi_type {
                        FfiType::RustBuffer(_) => {
                            let encode_expr = match runtime_unwrapped_type(&arg.type_) {
                                Type::Record { name, .. } | Type::Enum { name, .. } => {
                                    format!("_uniffiEncode{}({arg_name})", to_upper_camel(name))
                                }
                                Type::String => {
                                    format!("Uint8List.fromList(utf8.encode({arg_name}))")
                                }
                                Type::Bytes => arg_name.clone(),
                                _ => {
                                    out.push_str(&format!(
                                        "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                        function.name
                                    ));
                                    continue;
                                }
                            };
                            out.push_str(&format!(
                                "      final Uint8List {arg_name}Bytes = {encode_expr};\n"
                            ));
                            out.push_str(&format!(
                                "      final ffi.Pointer<ffi.Uint8> {arg_name}Ptr = {arg_name}Bytes.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}Bytes.length);\n"
                            ));
                            out.push_str(&format!(
                                "      if ({arg_name}Bytes.isNotEmpty) {{ {arg_name}Ptr.asTypedList({arg_name}Bytes.length).setAll(0, {arg_name}Bytes); }}\n"
                            ));
                            out.push_str(&format!("      foreignArgPtrs.add({arg_name}Ptr);\n"));
                            let from_bytes_status_ptr = format!("{arg_name}FromBytesStatusPtr");
                            let from_bytes_code = format!("{arg_name}FromBytesCode");
                            let from_bytes_err_buf = format!("{arg_name}FromBytesErrBuf");
                            let from_bytes_err_buf_ptr = format!("{arg_name}FromBytesErrBufPtr");
                            out.push_str(&format!(
                                "      final ffi.Pointer<_UniFfiRustCallStatus> {from_bytes_status_ptr} = calloc<_UniFfiRustCallStatus>();\n"
                            ));
                            out.push_str(&format!(
                                "      {from_bytes_status_ptr}.ref.code = _uniFfiRustCallStatusSuccess;\n"
                            ));
                            out.push_str(&format!("      {from_bytes_status_ptr}.ref.errorBuf\n"));
                            out.push_str("        ..capacity = 0\n");
                            out.push_str("        ..len = 0\n");
                            out.push_str("        ..data = ffi.nullptr;\n");
                            out.push_str(&format!(
                                "      final ffi.Pointer<_UniFfiForeignBytes> {arg_name}ForeignPtr = calloc<_UniFfiForeignBytes>();\n"
                            ));
                            out.push_str(&format!(
                                "      {arg_name}ForeignPtr.ref\n        ..len = {arg_name}Bytes.length\n        ..data = {arg_name}Ptr;\n"
                            ));
                            out.push_str(&format!(
                                "      final _UniFfiRustBuffer {arg_name}RustBuffer = _uniFfiRustBufferFromBytes({arg_name}ForeignPtr.ref, {from_bytes_status_ptr});\n"
                            ));
                            out.push_str(&format!("      calloc.free({arg_name}ForeignPtr);\n"));
                            out.push_str(&format!(
                                "      final int {from_bytes_code} = {from_bytes_status_ptr}.ref.code;\n"
                            ));
                            out.push_str(&format!(
                                "      final _UniFfiRustBuffer {from_bytes_err_buf} = {from_bytes_status_ptr}.ref.errorBuf;\n"
                            ));
                            out.push_str(&format!("      calloc.free({from_bytes_status_ptr});\n"));
                            out.push_str(&format!(
                                "      if ({from_bytes_code} != _uniFfiRustCallStatusSuccess) {{\n"
                            ));
                            out.push_str(&format!(
                                "        final ffi.Pointer<_UniFfiRustBuffer> {from_bytes_err_buf_ptr} = calloc<_UniFfiRustBuffer>();\n"
                            ));
                            out.push_str(&format!(
                                "        {from_bytes_err_buf_ptr}.ref\n          ..capacity = {from_bytes_err_buf}.capacity\n          ..len = {from_bytes_err_buf}.len\n          ..data = {from_bytes_err_buf}.data;\n"
                            ));
                            out.push_str(&format!(
                                "        rustRetBufferPtrs.add({from_bytes_err_buf_ptr});\n"
                            ));
                            out.push_str(&format!(
                                "        throw StateError('UniFFI rustbuffer_from_bytes failed with status ${from_bytes_code}');\n"
                            ));
                            out.push_str("      }\n");
                            out.push_str(&format!(
                                "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.capacity;\n",
                                offset
                            ));
                            out.push_str(&format!(
                                "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.len;\n",
                                offset + 1
                            ));
                            out.push_str(&format!(
                                "      (argBuf + {}).ref.ptr = {arg_name}RustBuffer.data.cast<ffi.Void>();\n",
                                offset + 2
                            ));
                        }
                        _ => {
                            let Some(union_field) = ffibuffer_primitive_union_field(ffi_type)
                            else {
                                out.push_str(&format!(
                                    "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                    function.name
                                ));
                                continue;
                            };
                            if union_field == "ptr" {
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.ptr = {}.cast<ffi.Void>();\n",
                                    offset, arg_name
                                ));
                            } else {
                                let value_expr = if union_field == "i8"
                                    && matches!(runtime_unwrapped_type(&arg.type_), Type::Boolean)
                                {
                                    format!("{arg_name} ? 1 : 0")
                                } else {
                                    arg_name.clone()
                                };
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.{} = {};\n",
                                    offset, union_field, value_expr
                                ));
                            }
                        }
                    }
                }

                out.push_str(&format!("      {method_field}(argBuf, returnBuf);\n"));
                out.push_str(&format!(
                    "      final int statusCode = (returnBuf + {}).ref.i8;\n",
                    return_ffi_elements
                ));
                out.push_str("      if (statusCode != _uniFfiRustCallStatusSuccess) {\n");
                out.push_str(&format!(
                    "        final ffi.Pointer<_UniFfiRustBuffer> errBufPtr = calloc<_UniFfiRustBuffer>();\n        errBufPtr.ref\n          ..capacity = (returnBuf + {}).ref.u64\n          ..len = (returnBuf + {}).ref.u64\n          ..data = (returnBuf + {}).ref.ptr.cast<ffi.Uint8>();\n",
                    return_ffi_elements + 1,
                    return_ffi_elements + 2,
                    return_ffi_elements + 3
                ));
                out.push_str("        rustRetBufferPtrs.add(errBufPtr);\n");
                out.push_str(
                    "        throw StateError('UniFFI ffibuffer async start failed with status $statusCode');\n",
                );
                out.push_str("      }\n");
                if start_return_union_field == "ptr" {
                    out.push_str(
                        "      final int futureHandle = (returnBuf + 0).ref.ptr.address;\n",
                    );
                } else {
                    out.push_str(&format!(
                        "      final int futureHandle = (returnBuf + 0).ref.{start_return_union_field};\n"
                    ));
                }
                out.push_str(
                    "      final StreamController<int> pollEvents = StreamController<int>.broadcast();\n",
                );
                out.push_str(
                    "      final callback = ffi.NativeCallable<ffi.Void Function(ffi.Uint64, ffi.Int8)>.listener((int _, int pollResult) {\n",
                );
                out.push_str("        pollEvents.add(pollResult);\n");
                out.push_str("      });\n");
                out.push_str("      try {\n");
                out.push_str(&format!(
                    "        {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
                ));
                out.push_str("        while (true) {\n");
                out.push_str("          final int pollResult = await pollEvents.stream.first;\n");
                out.push_str("          if (pollResult == _rustFuturePollReady) {\n");
                out.push_str("            break;\n");
                out.push_str("          }\n");
                out.push_str("          if (pollResult == _rustFuturePollWake) {\n");
                out.push_str(&format!(
                    "            {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
                ));
                out.push_str("            continue;\n");
                out.push_str("          }\n");
                out.push_str(&format!(
                    "          throw StateError('Rust future poll returned invalid status for {}: $pollResult');\n",
                    function.name
                ));
                out.push_str("        }\n");
                out.push_str(
                    "        final ffi.Pointer<_UniFfiRustCallStatus> outStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                );
                out.push_str("        outStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n");
                out.push_str("        outStatusPtr.ref.errorBuf\n");
                out.push_str("          ..capacity = 0\n");
                out.push_str("          ..len = 0\n");
                out.push_str("          ..data = ffi.nullptr;\n");
                out.push_str("        try {\n");
                if function.return_type.is_none() {
                    out.push_str(&format!(
                        "          {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "          final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                        async_spec.complete_dart_type
                    ));
                }
                out.push_str("          final int completeStatusCode = outStatusPtr.ref.code;\n");
                out.push_str(
                    "          if (completeStatusCode == _uniFfiRustCallStatusSuccess) {\n",
                );
                if function.return_type.is_none() {
                    out.push_str("            return;\n");
                } else if async_spec.suffix == "rust_buffer" {
                    if let Some(ret_type) = function.return_type.as_ref() {
                        let decode_expr = match runtime_unwrapped_type(ret_type) {
                            Type::String => "utf8.decode(resultBytes)".to_string(),
                            Type::Bytes => "resultBytes".to_string(),
                            Type::Record { name, .. } | Type::Enum { name, .. } => {
                                format!("_uniffiDecode{}(resultBytes)", to_upper_camel(name))
                            }
                            _ => render_uniffi_binary_read_expression(
                                ret_type,
                                "resultReader",
                                enums,
                            ),
                        };
                        out.push_str(
                            "            final ffi.Pointer<_UniFfiRustBuffer> resultBufPtr = calloc<_UniFfiRustBuffer>();\n",
                        );
                        out.push_str(
                            "            resultBufPtr.ref\n              ..capacity = resultValue.capacity\n              ..len = resultValue.len\n              ..data = resultValue.data;\n",
                        );
                        out.push_str("            rustRetBufferPtrs.add(resultBufPtr);\n");
                        out.push_str(
                            "            final Uint8List resultBytes = resultBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(resultBufPtr.ref.data.asTypedList(resultBufPtr.ref.len));\n",
                        );
                        if matches!(
                            runtime_unwrapped_type(ret_type),
                            Type::String | Type::Bytes | Type::Record { .. } | Type::Enum { .. }
                        ) {
                            out.push_str(&format!("            return {decode_expr};\n"));
                        } else {
                            out.push_str(
                                "            final _UniFfiBinaryReader resultReader = _UniFfiBinaryReader(resultBytes);\n",
                            );
                            out.push_str(&format!(
                                "            final decodedValue = {decode_expr};\n"
                            ));
                            out.push_str("            if (!resultReader.isDone) {\n");
                            out.push_str(
                                "              throw StateError('extra bytes remaining while decoding UniFFI rust future payload');\n",
                            );
                            out.push_str("            }\n");
                            out.push_str("            return decodedValue;\n");
                        }
                    }
                } else if let Some(ret_type) = function.return_type.as_ref() {
                    if is_runtime_object_type(ret_type) {
                        let lift = render_object_lift_expr(
                            ret_type,
                            "resultValue",
                            local_module_path,
                            "this",
                        );
                        out.push_str(&format!("            return {lift};\n"));
                    } else if is_runtime_timestamp_type(ret_type) {
                        out.push_str(
                            "            return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
                        );
                    } else if is_runtime_duration_type(ret_type) {
                        out.push_str("            return Duration(microseconds: resultValue);\n");
                    } else {
                        let decode = render_plain_ffi_decode_expr(ret_type, "resultValue");
                        out.push_str(&format!("            return {decode};\n"));
                    }
                }
                out.push_str("          }\n");
                out.push_str(
                    "          if (completeStatusCode == _uniFfiRustCallStatusCancelled) {\n",
                );
                out.push_str(&format!(
                    "            throw StateError('Rust future was cancelled for {}');\n",
                    function.name
                ));
                out.push_str("          }\n");
                out.push_str(
                    "          final _UniFfiRustBuffer errorBuf = outStatusPtr.ref.errorBuf;\n",
                );
                out.push_str(
                    "          if (!(errorBuf.data == ffi.nullptr && errorBuf.len == 0 && errorBuf.capacity == 0)) {\n",
                );
                out.push_str(
                    "            final ffi.Pointer<_UniFfiRustBuffer> errorBufPtr = calloc<_UniFfiRustBuffer>();\n",
                );
                out.push_str(
                    "            errorBufPtr.ref\n              ..capacity = errorBuf.capacity\n              ..len = errorBuf.len\n              ..data = errorBuf.data;\n",
                );
                out.push_str("            rustRetBufferPtrs.add(errorBufPtr);\n");
                out.push_str(
                    "            final Uint8List errorBytes = errorBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(errorBufPtr.ref.data.asTypedList(errorBufPtr.ref.len));\n",
                );
                out.push_str("            if (errorBytes.isNotEmpty) {\n");
                out.push_str("              throw StateError(utf8.decode(errorBytes, allowMalformed: true));\n");
                out.push_str("            }\n");
                out.push_str("          }\n");
                out.push_str(&format!(
                    "          throw StateError('Rust future failed for {} with status code: $completeStatusCode');\n",
                    function.name
                ));
                out.push_str("        } finally {\n");
                out.push_str("          calloc.free(outStatusPtr);\n");
                out.push_str("        }\n");
                out.push_str("      } catch (_) {\n");
                out.push_str(&format!("        {cancel_field}(futureHandle);\n"));
                out.push_str("        rethrow;\n");
                out.push_str("      } finally {\n");
                out.push_str("        await pollEvents.close();\n");
                out.push_str("        callback.close();\n");
                out.push_str(&format!("        {free_field}(futureHandle);\n"));
                out.push_str("      }\n");
                out.push_str("    } finally {\n");
                out.push_str("      for (final ptr in foreignArgPtrs) {\n");
                out.push_str("        if (ptr != ffi.nullptr) {\n");
                out.push_str("          calloc.free(ptr);\n");
                out.push_str("        }\n");
                out.push_str("      }\n");
                out.push_str("      for (final bufPtr in rustRetBufferPtrs) {\n");
                out.push_str(
                    "        if (bufPtr.ref.data == ffi.nullptr && bufPtr.ref.len == 0 && bufPtr.ref.capacity == 0) {\n",
                );
                out.push_str("          continue;\n");
                out.push_str("        }\n");
                out.push_str(
                    "        final ffi.Pointer<_UniFfiRustCallStatus> freeStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                );
                out.push_str("        freeStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n");
                out.push_str("        freeStatusPtr.ref.errorBuf\n");
                out.push_str("          ..capacity = 0\n");
                out.push_str("          ..len = 0\n");
                out.push_str("          ..data = ffi.nullptr;\n");
                out.push_str("        _uniFfiRustBufferFree(bufPtr.ref, freeStatusPtr);\n");
                out.push_str("        calloc.free(freeStatusPtr);\n");
                out.push_str("        calloc.free(bufPtr);\n");
                out.push_str("      }\n");
                out.push_str("      calloc.free(argBuf);\n");
                out.push_str("      calloc.free(returnBuf);\n");
                out.push_str("    }\n");
                out.push_str("  }\n");
                continue;
            }
            if ffibuffer_eligible {
                let value_return_type = function
                    .return_type
                    .as_ref()
                    .map(map_uniffi_type_to_dart)
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = if function.is_async {
                    format!("Future<{value_return_type}>")
                } else {
                    value_return_type.clone()
                };
                let dart_sig = function
                    .args
                    .iter()
                    .map(|a| {
                        format!(
                            "{} {}",
                            map_uniffi_type_to_dart(&a.type_),
                            safe_dart_identifier(&to_lower_camel(&a.name))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let method_field = format!("_{method_name}FfiBuffer");
                let ffi_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
                let ffibuffer_symbol = ffibuffer_symbol_name(ffi_symbol);
                let ffi_return_type = function.ffi_return_type.clone().or_else(|| {
                    function
                        .return_type
                        .as_ref()
                        .and_then(ffibuffer_ffi_type_from_uniffi_type)
                });
                let Some(ffi_return_type) = ffi_return_type else {
                    continue;
                };
                let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_return_type) else {
                    continue;
                };
                let ffi_arg_types = if function.ffi_arg_types.len() == function.args.len() {
                    function.ffi_arg_types.clone()
                } else {
                    function
                        .args
                        .iter()
                        .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_))
                        .collect::<Vec<_>>()
                };
                let mut arg_ffi_offsets = Vec::new();
                let mut arg_cursor = 0usize;
                let mut signature_compatible = ffi_arg_types.len() == function.args.len();
                if signature_compatible {
                    for ffi_type in &ffi_arg_types {
                        let Some(size) = ffibuffer_element_count(ffi_type) else {
                            signature_compatible = false;
                            break;
                        };
                        arg_ffi_offsets.push(arg_cursor);
                        arg_cursor += size;
                    }
                }
                if !signature_compatible {
                    let escaped_reason = reason.replace('\'', "\\'");
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_name}({dart_sig}){} {{\n",
                        if function.is_async { " async" } else { "" }
                    ));
                    out.push_str(&format!(
                        "    throw UnsupportedError('{escaped_reason} ({})');\n",
                        function.name
                    ));
                    out.push_str("  }\n");
                    continue;
                }

                out.push('\n');
                out.push_str(&format!(
                    "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {method_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{ffibuffer_symbol}');\n"
                ));
                out.push('\n');
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}){} {{\n",
                    if function.is_async { " async" } else { "" }
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                ));
                out.push_str(&format!(
                    "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                    return_ffi_elements + 4
                ));
                out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                out.push_str("    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n");
                out.push_str("    try {\n");

                for ((arg, ffi_type), offset) in function
                    .args
                    .iter()
                    .zip(ffi_arg_types.iter())
                    .zip(arg_ffi_offsets.iter())
                {
                    let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                    match ffi_type {
                        FfiType::RustBuffer(_) => {
                            let encode_expr = match runtime_unwrapped_type(&arg.type_) {
                                Type::Record { name, .. } | Type::Enum { name, .. } => {
                                    format!("_uniffiEncode{}({arg_name})", to_upper_camel(name))
                                }
                                Type::String => {
                                    format!("Uint8List.fromList(utf8.encode({arg_name}))")
                                }
                                Type::Bytes => arg_name.clone(),
                                _ => {
                                    let escaped_reason = reason.replace('\'', "\\'");
                                    out.push_str(&format!(
                                        "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                        function.name
                                    ));
                                    continue;
                                }
                            };
                            out.push_str(&format!(
                                "      final Uint8List {arg_name}Bytes = {encode_expr};\n"
                            ));
                            out.push_str(&format!(
                                "      final ffi.Pointer<ffi.Uint8> {arg_name}Ptr = {arg_name}Bytes.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}Bytes.length);\n"
                            ));
                            out.push_str(&format!(
                                "      if ({arg_name}Bytes.isNotEmpty) {{ {arg_name}Ptr.asTypedList({arg_name}Bytes.length).setAll(0, {arg_name}Bytes); }}\n"
                            ));
                            out.push_str(&format!("      foreignArgPtrs.add({arg_name}Ptr);\n"));
                            let from_bytes_status_ptr = format!("{arg_name}FromBytesStatusPtr");
                            let from_bytes_code = format!("{arg_name}FromBytesCode");
                            let from_bytes_err_buf = format!("{arg_name}FromBytesErrBuf");
                            let from_bytes_err_buf_ptr = format!("{arg_name}FromBytesErrBufPtr");
                            out.push_str(&format!(
                                "      final ffi.Pointer<_UniFfiRustCallStatus> {from_bytes_status_ptr} = calloc<_UniFfiRustCallStatus>();\n"
                            ));
                            out.push_str(&format!(
                                "      {from_bytes_status_ptr}.ref.code = _uniFfiRustCallStatusSuccess;\n"
                            ));
                            out.push_str(&format!("      {from_bytes_status_ptr}.ref.errorBuf\n"));
                            out.push_str("        ..capacity = 0\n");
                            out.push_str("        ..len = 0\n");
                            out.push_str("        ..data = ffi.nullptr;\n");
                            out.push_str(&format!(
                                "      final ffi.Pointer<_UniFfiForeignBytes> {arg_name}ForeignPtr = calloc<_UniFfiForeignBytes>();\n"
                            ));
                            out.push_str(&format!(
                                "      {arg_name}ForeignPtr.ref\n        ..len = {arg_name}Bytes.length\n        ..data = {arg_name}Ptr;\n"
                            ));
                            out.push_str(&format!(
                                "      final _UniFfiRustBuffer {arg_name}RustBuffer = _uniFfiRustBufferFromBytes({arg_name}ForeignPtr.ref, {from_bytes_status_ptr});\n"
                            ));
                            out.push_str(&format!("      calloc.free({arg_name}ForeignPtr);\n"));
                            out.push_str(&format!(
                                "      final int {from_bytes_code} = {from_bytes_status_ptr}.ref.code;\n"
                            ));
                            out.push_str(&format!(
                                "      final _UniFfiRustBuffer {from_bytes_err_buf} = {from_bytes_status_ptr}.ref.errorBuf;\n"
                            ));
                            out.push_str(&format!("      calloc.free({from_bytes_status_ptr});\n"));
                            out.push_str(&format!(
                                "      if ({from_bytes_code} != _uniFfiRustCallStatusSuccess) {{\n"
                            ));
                            out.push_str(&format!(
                                "        final ffi.Pointer<_UniFfiRustBuffer> {from_bytes_err_buf_ptr} = calloc<_UniFfiRustBuffer>();\n"
                            ));
                            out.push_str(&format!(
                                "        {from_bytes_err_buf_ptr}.ref\n          ..capacity = {from_bytes_err_buf}.capacity\n          ..len = {from_bytes_err_buf}.len\n          ..data = {from_bytes_err_buf}.data;\n"
                            ));
                            out.push_str(&format!(
                                "        rustRetBufferPtrs.add({from_bytes_err_buf_ptr});\n"
                            ));
                            out.push_str(&format!(
                                "        throw StateError('UniFFI rustbuffer_from_bytes failed with status ${from_bytes_code}');\n"
                            ));
                            out.push_str("      }\n");
                            out.push_str(&format!(
                                "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.capacity;\n",
                                offset
                            ));
                            out.push_str(&format!(
                                "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.len;\n",
                                offset + 1
                            ));
                            out.push_str(&format!(
                                "      (argBuf + {}).ref.ptr = {arg_name}RustBuffer.data.cast<ffi.Void>();\n",
                                offset + 2
                            ));
                        }
                        _ => {
                            let Some(union_field) = ffibuffer_primitive_union_field(ffi_type)
                            else {
                                let escaped_reason = reason.replace('\'', "\\'");
                                out.push_str(&format!(
                                    "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                    function.name
                                ));
                                continue;
                            };
                            if union_field == "ptr" {
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.ptr = {}.cast<ffi.Void>();\n",
                                    offset, arg_name
                                ));
                            } else {
                                let value_expr = if union_field == "i8"
                                    && matches!(runtime_unwrapped_type(&arg.type_), Type::Boolean)
                                {
                                    format!("{arg_name} ? 1 : 0")
                                } else {
                                    arg_name.clone()
                                };
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.{} = {};\n",
                                    offset, union_field, value_expr
                                ));
                            }
                        }
                    }
                }

                out.push_str(&format!("      {method_field}(argBuf, returnBuf);\n"));
                out.push_str(&format!(
                    "      final int statusCode = (returnBuf + {}).ref.i8;\n",
                    return_ffi_elements
                ));
                out.push_str("      if (statusCode != _uniFfiRustCallStatusSuccess) {\n");
                out.push_str(&format!(
                    "        final ffi.Pointer<_UniFfiRustBuffer> errBufPtr = calloc<_UniFfiRustBuffer>();\n        errBufPtr.ref\n          ..capacity = (returnBuf + {}).ref.u64\n          ..len = (returnBuf + {}).ref.u64\n          ..data = (returnBuf + {}).ref.ptr.cast<ffi.Uint8>();\n",
                    return_ffi_elements + 1,
                    return_ffi_elements + 2,
                    return_ffi_elements + 3
                ));
                out.push_str("        rustRetBufferPtrs.add(errBufPtr);\n");
                if let Some(throws_name) = function
                    .throws_type
                    .as_ref()
                    .and_then(enum_name_from_type)
                    .map(to_upper_camel)
                {
                    let exception_name = format!("{throws_name}Exception");
                    out.push_str("        if (statusCode == _uniFfiRustCallStatusError) {\n");
                    out.push_str(
                        "          final Uint8List errBytes = errBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(errBufPtr.ref.data.asTypedList(errBufPtr.ref.len));\n",
                    );
                    out.push_str(&format!(
                        "          throw _uniffiLift{exception_name}(errBytes);\n"
                    ));
                    out.push_str("        }\n");
                }
                out.push_str(
                    "        throw StateError('UniFFI ffibuffer call failed with status $statusCode');\n",
                );
                out.push_str("      }\n");

                match function.return_type.as_ref() {
                    None => out.push_str("      return;\n"),
                    Some(ret_type) => match &ffi_return_type {
                        FfiType::RustBuffer(_) => {
                            let decode_expr = match runtime_unwrapped_type(ret_type) {
                                Type::String => "utf8.decode(retBytes)".to_string(),
                                Type::Bytes => "retBytes".to_string(),
                                Type::Record { name, .. } | Type::Enum { name, .. } => {
                                    format!("_uniffiDecode{}(retBytes)", to_upper_camel(name))
                                }
                                _ => render_uniffi_binary_read_expression(
                                    ret_type,
                                    "retReader",
                                    enums,
                                ),
                            };
                            out.push_str(
                                "      final ffi.Pointer<_UniFfiRustBuffer> retBufPtr = calloc<_UniFfiRustBuffer>();\n",
                            );
                            out.push_str(
                                "      retBufPtr.ref\n        ..capacity = (returnBuf + 0).ref.u64\n        ..len = (returnBuf + 1).ref.u64\n        ..data = (returnBuf + 2).ref.ptr.cast<ffi.Uint8>();\n",
                            );
                            out.push_str("      rustRetBufferPtrs.add(retBufPtr);\n");
                            out.push_str(
                                "      final Uint8List retBytes = retBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(retBufPtr.ref.data.asTypedList(retBufPtr.ref.len));\n",
                            );
                            if matches!(
                                runtime_unwrapped_type(ret_type),
                                Type::String
                                    | Type::Bytes
                                    | Type::Record { .. }
                                    | Type::Enum { .. }
                            ) {
                                out.push_str(&format!(
                                    "      final decodedValue = {decode_expr};\n"
                                ));
                            } else {
                                out.push_str(
                                    "      final _UniFfiBinaryReader retReader = _UniFfiBinaryReader(retBytes);\n",
                                );
                                out.push_str(&format!(
                                    "      final decodedValue = {decode_expr};\n"
                                ));
                                out.push_str("      if (!retReader.isDone) {\n");
                                out.push_str(
                                    "        throw StateError('extra bytes remaining while decoding UniFFI ffibuffer return payload');\n",
                                );
                                out.push_str("      }\n");
                            }
                            out.push_str("      return decodedValue;\n");
                        }
                        _ => {
                            let Some(union_field) =
                                ffibuffer_primitive_union_field(&ffi_return_type)
                            else {
                                let escaped_reason = reason.replace('\'', "\\'");
                                out.push_str(&format!(
                                    "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                    function.name
                                ));
                                out.push_str("      return;\n");
                                out.push_str("    } finally {\n");
                                out.push_str("      calloc.free(argBuf);\n");
                                out.push_str("      calloc.free(returnBuf);\n");
                                out.push_str("    }\n");
                                out.push_str("  }\n");
                                continue;
                            };
                            if union_field == "ptr" {
                                out.push_str("      return (returnBuf + 0).ref.ptr;\n");
                            } else {
                                out.push_str(&format!(
                                    "      return (returnBuf + 0).ref.{union_field};\n"
                                ));
                            }
                        }
                    },
                }
                out.push_str("    } finally {\n");
                out.push_str("      for (final ptr in foreignArgPtrs) {\n");
                out.push_str("        if (ptr != ffi.nullptr) {\n");
                out.push_str("          calloc.free(ptr);\n");
                out.push_str("        }\n");
                out.push_str("      }\n");
                out.push_str("      for (final bufPtr in rustRetBufferPtrs) {\n");
                out.push_str(
                    "        if (bufPtr.ref.data == ffi.nullptr && bufPtr.ref.len == 0 && bufPtr.ref.capacity == 0) {\n",
                );
                out.push_str("          continue;\n");
                out.push_str("        }\n");
                out.push_str(
                    "        final ffi.Pointer<_UniFfiRustCallStatus> freeStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                );
                out.push_str("        freeStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n");
                out.push_str("        freeStatusPtr.ref.errorBuf\n");
                out.push_str("          ..capacity = 0\n");
                out.push_str("          ..len = 0\n");
                out.push_str("          ..data = ffi.nullptr;\n");
                out.push_str("        _uniFfiRustBufferFree(bufPtr.ref, freeStatusPtr);\n");
                out.push_str("        calloc.free(freeStatusPtr);\n");
                out.push_str("        calloc.free(bufPtr);\n");
                out.push_str("      }\n");
                out.push_str("      calloc.free(argBuf);\n");
                out.push_str("      calloc.free(returnBuf);\n");
                out.push_str("    }\n");
                out.push_str("  }\n");
                continue;
            }

            let value_return_type = function
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return_type = if function.is_async {
                format!("Future<{value_return_type}>")
            } else {
                value_return_type
            };
            let dart_sig = function
                .args
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        map_uniffi_type_to_dart(&a.type_),
                        safe_dart_identifier(&to_lower_camel(&a.name))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let escaped_reason = reason.replace('\'', "\\'");
            out.push('\n');
            if function.is_async {
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}) async {{\n"
                ));
            } else {
                out.push_str(&format!(
                    "  {signature_return_type} {method_name}({dart_sig}) {{\n"
                ));
            }
            out.push_str(&format!(
                "    throw UnsupportedError('{escaped_reason} ({})');\n",
                function.name
            ));
            out.push_str("  }\n");
            continue;
        }

        let is_runtime_supported = is_runtime_ffi_compatible_function(function, records, enums);
        let is_sync_callback_supported =
            is_runtime_callback_compatible_function(function, callback_interfaces, records, enums);
        let has_callback_args =
            has_runtime_callback_args_in_args(&function.args, callback_interfaces, records, enums);
        if !is_runtime_supported && !is_sync_callback_supported && !has_callback_args {
            continue;
        }
        let field_name = format!("_{}", method_name);
        let function_symbol = function.ffi_symbol.as_deref().unwrap_or(&function.name);
        if is_sync_callback_supported {
            let return_type = function
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let native_return = function
                .return_type
                .as_ref()
                .and_then(|t| map_runtime_native_ffi_type(t, records, enums))
                .unwrap_or("ffi.Void");
            let dart_ffi_return = function
                .return_type
                .as_ref()
                .and_then(|t| map_runtime_dart_ffi_type(t, records, enums))
                .unwrap_or("void");

            let mut native_args = Vec::new();
            let mut dart_ffi_args = Vec::new();
            let mut dart_args = Vec::new();
            let mut call_args = Vec::new();
            let mut pre_call = Vec::new();
            let mut post_call = Vec::new();

            for arg in &function.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                dart_args.push(format!(
                    "{} {}",
                    map_uniffi_type_to_dart(&arg.type_),
                    arg_name
                ));
                if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
                    let init_done_field = callback_init_done_field_name(callback_name);
                    let bridge_name = callback_bridge_class_name(callback_name);
                    let handle_name = format!("{arg_name}Handle");
                    native_args.push(format!("ffi.Uint64 {arg_name}"));
                    dart_ffi_args.push(format!("int {arg_name}"));
                    pre_call.push(format!("    {init_done_field};\n"));
                    pre_call.push(format!(
                        "    final int {handle_name} = {bridge_name}.instance.register({arg_name});\n"
                    ));
                    post_call.push(format!(
                        "    {bridge_name}.instance.release({handle_name});\n"
                    ));
                    call_args.push(handle_name);
                    continue;
                }
                let native_type = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    .expect("validated callback-compatible arg type");
                let dart_ffi_type = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    .expect("validated callback-compatible arg type");
                native_args.push(format!("{native_type} {arg_name}"));
                dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
                append_runtime_arg_marshalling(
                    &arg_name,
                    &arg.type_,
                    enums,
                    &mut pre_call,
                    &mut post_call,
                    &mut call_args,
                );
            }

            let native_sig = format!("{native_return} Function({})", native_args.join(", "));
            let dart_ffi_sig = format!("{dart_ffi_return} Function({})", dart_ffi_args.join(", "));
            let dart_sig = dart_args.join(", ");

            out.push('\n');
            out.push_str(&format!(
                "  late final {dart_ffi_sig} {field_name} = _lib.lookupFunction<{native_sig}, {dart_ffi_sig}>('{}');\n",
                function_symbol
            ));
            out.push('\n');
            out.push_str(&format!("  {return_type} {method_name}({dart_sig}) {{\n"));
            for line in &pre_call {
                out.push_str(line);
            }
            if !post_call.is_empty() {
                out.push_str("    try {\n");
            }
            let call = format!("{field_name}({})", call_args.join(", "));
            match function.return_type.as_ref() {
                None => out.push_str(&format!("    {call};\n")),
                Some(ret_type) => {
                    let decode = render_plain_ffi_decode_expr(ret_type, &call);
                    out.push_str(&format!("    return {decode};\n"));
                }
            }
            if !post_call.is_empty() {
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
            continue;
        }

        let return_type = function
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let is_throwing = is_runtime_throwing_ffi_compatible_function(
            function,
            callback_interfaces,
            records,
            enums,
        );
        let native_return = function
            .return_type
            .as_ref()
            .map(|t| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    map_runtime_native_ffi_type(t, records, enums)
                }
            })
            .unwrap_or_else(|| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    Some("ffi.Void")
                }
            });
        let dart_ffi_return = function
            .return_type
            .as_ref()
            .map(|t| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    map_runtime_dart_ffi_type(t, records, enums)
                }
            })
            .unwrap_or_else(|| {
                if is_throwing {
                    Some("ffi.Pointer<Utf8>")
                } else {
                    Some("void")
                }
            });

        let Some(native_return) = native_return else {
            continue;
        };
        let Some(dart_ffi_return) = dart_ffi_return else {
            continue;
        };

        let mut native_args = Vec::new();
        let mut dart_ffi_args = Vec::new();
        let mut dart_args = Vec::new();
        let mut call_args = Vec::new();
        let mut pre_call = Vec::new();
        let mut post_call = Vec::new();
        let mut signature_compatible = true;

        for arg in &function.args {
            let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
            dart_args.push(format!(
                "{} {}",
                map_uniffi_type_to_dart(&arg.type_),
                arg_name
            ));
            if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
                let init_done_field = callback_init_done_field_name(callback_name);
                let bridge_name = callback_bridge_class_name(callback_name);
                let handle_name = format!("{arg_name}Handle");
                native_args.push(format!("ffi.Uint64 {arg_name}"));
                dart_ffi_args.push(format!("int {arg_name}"));
                pre_call.push(format!("    {init_done_field};\n"));
                pre_call.push(format!(
                    "    final int {handle_name} = {bridge_name}.instance.register({arg_name});\n"
                ));
                post_call.push(format!(
                    "    {bridge_name}.instance.release({handle_name});\n"
                ));
                call_args.push(handle_name);
                continue;
            }
            let Some(native_type) = map_runtime_native_ffi_type(&arg.type_, records, enums) else {
                signature_compatible = false;
                break;
            };
            let Some(dart_ffi_type) = map_runtime_dart_ffi_type(&arg.type_, records, enums) else {
                signature_compatible = false;
                break;
            };
            native_args.push(format!("{native_type} {arg_name}"));
            dart_ffi_args.push(format!("{dart_ffi_type} {arg_name}"));
            append_runtime_arg_marshalling(
                &arg_name,
                &arg.type_,
                enums,
                &mut pre_call,
                &mut post_call,
                &mut call_args,
            );
        }

        if !signature_compatible {
            continue;
        }

        if is_runtime_async_rust_future_compatible_function(
            function,
            callback_interfaces,
            records,
            enums,
        ) {
            let Some(async_spec) =
                async_rust_future_spec(function.return_type.as_ref(), records, enums)
            else {
                continue;
            };
            let start_native_sig = format!("ffi.Uint64 Function({})", native_args.join(", "));
            let start_dart_sig = format!("int Function({})", dart_ffi_args.join(", "));
            let poll_field = format!("{field_name}RustFuturePoll");
            let cancel_field = format!("{field_name}RustFutureCancel");
            let complete_field = format!("{field_name}RustFutureComplete");
            let free_field = format!("{field_name}RustFutureFree");
            let complete_symbol = format!("rust_future_complete_{}", async_spec.suffix);
            let poll_symbol = format!("rust_future_poll_{}", async_spec.suffix);
            let cancel_symbol = format!("rust_future_cancel_{}", async_spec.suffix);
            let free_symbol = format!("rust_future_free_{}", async_spec.suffix);
            let complete_native_sig = format!(
                "{} Function(ffi.Uint64 handle, ffi.Pointer<_RustCallStatus> outStatus)",
                async_spec.complete_native_type
            );
            let complete_dart_sig = format!(
                "{} Function(int handle, ffi.Pointer<_RustCallStatus> outStatus)",
                async_spec.complete_dart_type
            );

            out.push('\n');
            out.push_str(&format!(
                "  late final {start_dart_sig} {field_name} = _lib.lookupFunction<{start_native_sig}, {start_dart_sig}>('{}');\n",
                function_symbol
            ));
            out.push_str(&format!(
                "  late final void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData) {poll_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, ffi.Uint64 callbackData), void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData)>('{poll_symbol}');\n"
            ));
            out.push_str(&format!(
                "  late final void Function(int handle) {cancel_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{cancel_symbol}');\n"
            ));
            out.push_str(&format!(
                "  late final {complete_dart_sig} {complete_field} = _lib.lookupFunction<{complete_native_sig}, {complete_dart_sig}>('{complete_symbol}');\n"
            ));
            out.push_str(&format!(
                "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{free_symbol}');\n"
            ));
            out.push('\n');
            out.push_str(&format!(
                "  Future<{return_type}> {method_name}({}) async {{\n",
                dart_args.join(", ")
            ));
            for line in &pre_call {
                out.push_str(line);
            }
            out.push_str("    final int futureHandle;\n");
            if !post_call.is_empty() {
                out.push_str("    try {\n");
                out.push_str(&format!(
                    "      futureHandle = {field_name}({});\n",
                    call_args.join(", ")
                ));
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            } else {
                out.push_str(&format!(
                    "    futureHandle = {field_name}({});\n",
                    call_args.join(", ")
                ));
            }
            out.push_str(
                "    final StreamController<int> pollEvents = StreamController<int>.broadcast();\n",
            );
            out.push_str(
                "    final callback = ffi.NativeCallable<ffi.Void Function(ffi.Uint64, ffi.Int8)>.listener((int _, int pollResult) {\n",
            );
            out.push_str("      pollEvents.add(pollResult);\n");
            out.push_str("    });\n");
            out.push_str("    try {\n");
            out.push_str(&format!(
                "      {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
            ));
            out.push_str("      while (true) {\n");
            out.push_str("        final int pollResult = await pollEvents.stream.first;\n");
            out.push_str("        if (pollResult == _rustFuturePollReady) {\n");
            out.push_str("          break;\n");
            out.push_str("        }\n");
            out.push_str("        if (pollResult == _rustFuturePollWake) {\n");
            out.push_str(&format!(
                "          {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
            ));
            out.push_str("          continue;\n");
            out.push_str("        }\n");
            out.push_str(&format!(
                "        throw StateError('Rust future poll returned invalid status for {}: $pollResult');\n",
                function.name
            ));
            out.push_str("      }\n");
            out.push_str(
                "      final ffi.Pointer<_RustCallStatus> outStatusPtr = calloc<_RustCallStatus>();\n",
            );
            out.push_str("      try {\n");
            if function.return_type.is_none() {
                out.push_str(&format!(
                    "        {complete_field}(futureHandle, outStatusPtr);\n"
                ));
            } else if let Some(ret_type) = function.return_type.as_ref() {
                if is_runtime_utf8_pointer_marshaled_type(ret_type, records, enums) {
                    out.push_str(&format!(
                        "        final ffi.Pointer<Utf8> resultPtr = {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "        final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                        async_spec.complete_dart_type
                    ));
                }
            } else {
                out.push_str(&format!(
                    "        final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                    async_spec.complete_dart_type
                ));
            }
            out.push_str("        final int statusCode = outStatusPtr.ref.code;\n");
            out.push_str("        if (statusCode == _rustCallStatusSuccess) {\n");
            if let Some(ret_type) = function.return_type.as_ref() {
                if is_runtime_string_type(ret_type) {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_optional_string_type(ret_type) {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_record_type(ret_type) {
                    let record_name = record_name_from_type(ret_type).unwrap_or("Record");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                        to_upper_camel(record_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_enum_type(ret_type, enums) {
                    let enum_name = enum_name_from_type(ret_type).unwrap_or("Enum");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_object_type(ret_type) {
                    let lift =
                        render_object_lift_expr(ret_type, "resultValue", local_module_path, "this");
                    out.push_str(&format!("          return {lift};\n"));
                } else if is_runtime_map_with_string_key_type(ret_type) {
                    let decode = render_json_decode_expr("jsonDecode(payload)", ret_type);
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if is_runtime_bytes_type(ret_type) {
                    out.push_str("          final _RustBuffer resultBuf = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesFree(resultBuf);\n");
                    out.push_str("              return Uint8List(0);\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if is_runtime_optional_bytes_type(ret_type) {
                    out.push_str("          final _RustBufferOpt resultOpt = resultValue;\n");
                    out.push_str("          if (resultOpt.isSome == 0) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesFree(resultBuf);\n");
                    out.push_str("              return Uint8List(0);\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid optional buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if is_runtime_sequence_bytes_type(ret_type) {
                    out.push_str("          final _RustBufferVec resultVec = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("          final int resultLen = resultVec.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesVecFree(resultVec);\n");
                    out.push_str("              return <Uint8List>[];\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid byte vector for {}');\n",
                        function.name
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final out = <Uint8List>[];\n");
                    out.push_str("            for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("              final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str(
                        "              final ffi.Pointer<ffi.Uint8> itemData = item.data;\n",
                    );
                    out.push_str("              final int itemLen = item.len;\n");
                    out.push_str("              if (itemData == ffi.nullptr) {\n");
                    out.push_str("                if (itemLen == 0) {\n");
                    out.push_str("                  out.add(Uint8List(0));\n");
                    out.push_str("                  continue;\n");
                    out.push_str("                }\n");
                    out.push_str(&format!(
                        "                throw StateError('Rust returned invalid nested buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("              }\n");
                    out.push_str("              try {\n");
                    out.push_str(
                        "                out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("              } finally {\n");
                    out.push_str("                _rustBytesFree(item);\n");
                    out.push_str("              }\n");
                    out.push_str("            }\n");
                    out.push_str("            return out;\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesVecFree(resultVec);\n");
                    out.push_str("          }\n");
                } else if is_runtime_timestamp_type(ret_type) {
                    out.push_str(
                        "          return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
                    );
                } else if is_runtime_duration_type(ret_type) {
                    out.push_str("          return Duration(microseconds: resultValue);\n");
                } else {
                    let decode = render_plain_ffi_decode_expr(ret_type, "resultValue");
                    out.push_str(&format!("          return {decode};\n"));
                }
            } else {
                out.push_str("          return;\n");
            }
            out.push_str("        }\n");
            out.push_str("        if (statusCode == _rustCallStatusCancelled) {\n");
            out.push_str(&format!(
                "          throw StateError('Rust future was cancelled for {}');\n",
                function.name
            ));
            out.push_str("        }\n");
            out.push_str("        final ffi.Pointer<Utf8> errorPtr = outStatusPtr.ref.errorBuf;\n");
            out.push_str("        if (errorPtr != ffi.nullptr) {\n");
            out.push_str("          try {\n");
            out.push_str("            throw StateError(errorPtr.toDartString());\n");
            out.push_str("          } finally {\n");
            out.push_str("            _rustStringFree(errorPtr);\n");
            out.push_str("          }\n");
            out.push_str("        }\n");
            out.push_str(&format!(
                "        throw StateError('Rust future failed for {} with status code: $statusCode');\n",
                function.name
            ));
            out.push_str("      } finally {\n");
            out.push_str("        calloc.free(outStatusPtr);\n");
            out.push_str("      }\n");
            out.push_str("    } catch (_) {\n");
            out.push_str(&format!("      {cancel_field}(futureHandle);\n"));
            out.push_str("      rethrow;\n");
            out.push_str("    } finally {\n");
            out.push_str("      await pollEvents.close();\n");
            out.push_str("      callback.close();\n");
            out.push_str(&format!("      {free_field}(futureHandle);\n"));
            out.push_str("    }\n");
            out.push_str("  }\n");
            continue;
        }

        let native_sig = format!("{native_return} Function({})", native_args.join(", "));
        let dart_sig = format!("{dart_ffi_return} Function({})", dart_ffi_args.join(", "));

        out.push('\n');
        out.push_str(&format!(
            "  late final {dart_sig} {field_name} = _lib.lookupFunction<{native_sig}, {dart_sig}>('{}');\n",
            function_symbol
        ));
        out.push('\n');
        out.push_str(&format!(
            "  {return_type} {method_name}({}) {{\n",
            dart_args.join(", ")
        ));
        for line in &pre_call {
            out.push_str(line);
        }
        if !post_call.is_empty() {
            out.push_str("    try {\n");
        }
        let call_expr = format!("{field_name}({})", call_args.join(", "));
        if is_throwing {
            let Some(throws_name) = function
                .throws_type
                .as_ref()
                .and_then(enum_name_from_type)
                .map(to_upper_camel)
            else {
                continue;
            };
            let ok_decode = function
                .return_type
                .as_ref()
                .map(|t| render_json_decode_expr("okRaw", t));
            out.push_str(&format!(
                "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
            ));
            out.push_str("      if (resultPtr == ffi.nullptr) {\n");
            out.push_str(&format!(
                "        throw StateError('Rust returned null for {}');\n",
                function.name
            ));
            out.push_str("      }\n");
            out.push_str("      final String payload;\n");
            out.push_str("      try {\n");
            out.push_str("        payload = resultPtr.toDartString();\n");
            out.push_str("      } finally {\n");
            out.push_str("        _rustStringFree(resultPtr);\n");
            out.push_str("      }\n");
            out.push_str(
                "      final Map<String, dynamic> envelope = jsonDecode(payload) as Map<String, dynamic>;\n",
            );
            out.push_str("      final Object? errRaw = envelope['err'];\n");
            out.push_str("      if (errRaw != null) {\n");
            out.push_str(&format!(
                "        throw {}ExceptionFfiCodec.decode(errRaw);\n",
                throws_name
            ));
            out.push_str("      }\n");
            if let Some(ok_decode) = ok_decode {
                out.push_str("      if (!envelope.containsKey('ok')) {\n");
                out.push_str(&format!(
                    "        throw StateError('Rust returned malformed result for {}');\n",
                    function.name
                ));
                out.push_str("      }\n");
                out.push_str("      final Object? okRaw = envelope['ok'];\n");
                out.push_str(&format!("      return {ok_decode};\n"));
            } else {
                out.push_str("      return;\n");
            }
        } else {
            match function.return_type.as_ref() {
                Some(type_) if is_runtime_string_type(type_) => {
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        return resultPtr.toDartString();\n");
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_optional_string_type(type_) => {
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        return resultPtr.toDartString();\n");
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_timestamp_type(type_) => {
                    out.push_str(&format!("      final int micros = {call_expr};\n"));
                    out.push_str(
                        "      return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);\n",
                    );
                }
                Some(type_) if is_runtime_duration_type(type_) => {
                    out.push_str(&format!("      final int micros = {call_expr};\n"));
                    out.push_str("      return Duration(microseconds: micros);\n");
                }
                Some(type_) if is_runtime_bytes_type(type_) => {
                    out.push_str(&format!(
                        "      final _RustBuffer resultBuf = {call_expr};\n"
                    ));
                    out.push_str(
                        "      final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("      final int resultLen = resultBuf.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        if (resultLen == 0) {\n");
                    out.push_str("          _rustBytesFree(resultBuf);\n");
                    out.push_str("          return Uint8List(0);\n");
                    out.push_str("        }\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned invalid buffer for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str(
                        "        return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_optional_bytes_type(type_) => {
                    out.push_str(&format!(
                        "      final _RustBufferOpt resultOpt = {call_expr};\n"
                    ));
                    out.push_str("      if (resultOpt.isSome == 0) {\n");
                    out.push_str("        return null;\n");
                    out.push_str("      }\n");
                    out.push_str("      final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str(
                        "      final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("      final int resultLen = resultBuf.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        if (resultLen == 0) {\n");
                    out.push_str("          _rustBytesFree(resultBuf);\n");
                    out.push_str("          return Uint8List(0);\n");
                    out.push_str("        }\n");
                    out.push_str(&format!(
                    "        throw StateError('Rust returned invalid optional buffer for {}');\n",
                    function.name
                ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str(
                        "        return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_sequence_bytes_type(type_) => {
                    out.push_str(&format!(
                        "      final _RustBufferVec resultVec = {call_expr};\n"
                    ));
                    out.push_str(
                        "      final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("      final int resultLen = resultVec.len;\n");
                    out.push_str("      if (resultData == ffi.nullptr) {\n");
                    out.push_str("        if (resultLen == 0) {\n");
                    out.push_str("          _rustBytesVecFree(resultVec);\n");
                    out.push_str("          return <Uint8List>[];\n");
                    out.push_str("        }\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned invalid byte vector for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final out = <Uint8List>[];\n");
                    out.push_str("        for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("          final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str("          final ffi.Pointer<ffi.Uint8> itemData = item.data;\n");
                    out.push_str("          final int itemLen = item.len;\n");
                    out.push_str("          if (itemData == ffi.nullptr) {\n");
                    out.push_str("            if (itemLen == 0) {\n");
                    out.push_str("              out.add(Uint8List(0));\n");
                    out.push_str("              continue;\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                    "            throw StateError('Rust returned invalid nested buffer for {}');\n",
                    function.name
                ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(item);\n");
                    out.push_str("          }\n");
                    out.push_str("        }\n");
                    out.push_str("        return out;\n");
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustBytesVecFree(resultVec);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_record_type(type_) => {
                    let record_name = record_name_from_type(type_).unwrap_or("Record");
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                    "        return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                    to_upper_camel(record_name)
                ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_enum_type(type_, enums) => {
                    let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "        return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(type_) if is_runtime_object_type(type_) => {
                    let lift =
                        render_object_lift_expr(type_, &call_expr, local_module_path, "this");
                    out.push_str(&format!("      return {lift};\n"));
                }
                Some(type_) if is_runtime_map_with_string_key_type(type_) => {
                    let decode = render_json_decode_expr("jsonDecode(payload)", type_);
                    out.push_str(&format!(
                        "      final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("      if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "        throw StateError('Rust returned null for {}');\n",
                        function.name
                    ));
                    out.push_str("      }\n");
                    out.push_str("      try {\n");
                    out.push_str("        final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!("        return {decode};\n"));
                    out.push_str("      } finally {\n");
                    out.push_str("        _rustStringFree(resultPtr);\n");
                    out.push_str("      }\n");
                }
                Some(_) => {
                    out.push_str(&format!("      return {call_expr};\n"));
                }
                None => {
                    out.push_str(&format!("      {call_expr};\n"));
                }
            }
        }
        if !post_call.is_empty() {
            out.push_str("    } finally {\n");
            for line in &post_call {
                out.push_str(line);
            }
            out.push_str("    }\n");
        }
        out.push_str("  }\n");
    }

    for object in objects {
        let object_name = to_upper_camel(&object.name);
        let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
        let object_symbol = dart_identifier(&object.name);
        let free_field = format!("_{}Free", object_lower);
        out.push('\n');
        out.push_str(&format!(
            "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{object_symbol}_free');\n"
        ));

        for ctor in &object.constructors {
            if let Some(reason) = ctor.runtime_unsupported.as_ref() {
                let ctor_camel = to_upper_camel(&ctor.name);
                let ctor_method = format!("{}Create{}", object_lower, ctor_camel);
                let dart_args = ctor
                    .args
                    .iter()
                    .map(|arg| {
                        let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                        format!("{} {arg_name}", map_uniffi_type_to_dart(&arg.type_))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let escaped_reason = reason.replace('\'', "\\'");
                let ffibuffer_eligible = is_ffibuffer_eligible_object_constructor(ctor);
                if ffibuffer_eligible {
                    let ctor_field = format!("_{}Ctor{}FfiBuffer", object_lower, ctor_camel);
                    let ctor_symbol = ctor.ffi_symbol.as_deref().unwrap_or(&ctor.name).to_string();
                    let ffibuffer_symbol = ffibuffer_symbol_name(&ctor_symbol);
                    let ffi_return_type = ctor.ffi_return_type.clone().or(Some(FfiType::Handle));
                    let Some(ffi_return_type) = ffi_return_type else {
                        out.push('\n');
                        out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            ctor.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_return_type)
                    else {
                        out.push('\n');
                        out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            ctor.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    let ffi_arg_types = if ctor.ffi_arg_types.len() == ctor.args.len() {
                        ctor.ffi_arg_types.clone()
                    } else {
                        ctor.args
                            .iter()
                            .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_))
                            .collect::<Vec<_>>()
                    };
                    let mut arg_ffi_offsets = Vec::new();
                    let mut arg_cursor = 0usize;
                    let mut signature_compatible = ffi_arg_types.len() == ctor.args.len();
                    if signature_compatible {
                        for ffi_type in &ffi_arg_types {
                            let Some(size) = ffibuffer_element_count(ffi_type) else {
                                signature_compatible = false;
                                break;
                            };
                            arg_ffi_offsets.push(arg_cursor);
                            arg_cursor += size;
                        }
                    }
                    if !signature_compatible {
                        out.push('\n');
                        out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            ctor.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    }

                    out.push('\n');
                    out.push_str(&format!(
                        "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {ctor_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{ffibuffer_symbol}');\n"
                    ));
                    out.push('\n');
                    out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                        return_ffi_elements + 4
                    ));
                    out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                    out.push_str(
                        "    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n",
                    );
                    out.push_str("    try {\n");
                    for ((arg, ffi_type), offset) in ctor
                        .args
                        .iter()
                        .zip(ffi_arg_types.iter())
                        .zip(arg_ffi_offsets.iter())
                    {
                        let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                        match ffi_type {
                            FfiType::RustBuffer(_) => {
                                let encode_expr = match &arg.type_ {
                                    Type::Record { name, .. } | Type::Enum { name, .. } => {
                                        format!("_uniffiEncode{}({arg_name})", to_upper_camel(name))
                                    }
                                    _ => {
                                        out.push_str(&format!(
                                            "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                            ctor.name
                                        ));
                                        continue;
                                    }
                                };
                                out.push_str(&format!(
                                    "      final Uint8List {arg_name}Bytes = {encode_expr};\n"
                                ));
                                out.push_str(&format!(
                                    "      final ffi.Pointer<ffi.Uint8> {arg_name}Ptr = {arg_name}Bytes.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}Bytes.length);\n"
                                ));
                                out.push_str(&format!(
                                    "      if ({arg_name}Bytes.isNotEmpty) {{ {arg_name}Ptr.asTypedList({arg_name}Bytes.length).setAll(0, {arg_name}Bytes); }}\n"
                                ));
                                out.push_str(&format!(
                                    "      foreignArgPtrs.add({arg_name}Ptr);\n"
                                ));
                                out.push_str(
                                    "      final ffi.Pointer<_UniFfiRustCallStatus> fromBytesStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                                );
                                out.push_str(
                                    "      fromBytesStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n",
                                );
                                out.push_str("      fromBytesStatusPtr.ref.errorBuf\n");
                                out.push_str("        ..capacity = 0\n");
                                out.push_str("        ..len = 0\n");
                                out.push_str("        ..data = ffi.nullptr;\n");
                                out.push_str(&format!(
                                    "      final ffi.Pointer<_UniFfiForeignBytes> {arg_name}ForeignPtr = calloc<_UniFfiForeignBytes>();\n"
                                ));
                                out.push_str(&format!(
                                    "      {arg_name}ForeignPtr.ref\n        ..len = {arg_name}Bytes.length\n        ..data = {arg_name}Ptr;\n"
                                ));
                                out.push_str(&format!(
                                    "      final _UniFfiRustBuffer {arg_name}RustBuffer = _uniFfiRustBufferFromBytes({arg_name}ForeignPtr.ref, fromBytesStatusPtr);\n"
                                ));
                                out.push_str(&format!(
                                    "      calloc.free({arg_name}ForeignPtr);\n"
                                ));
                                out.push_str(
                                    "      final int fromBytesCode = fromBytesStatusPtr.ref.code;\n",
                                );
                                out.push_str(
                                    "      final _UniFfiRustBuffer fromBytesErrBuf = fromBytesStatusPtr.ref.errorBuf;\n",
                                );
                                out.push_str("      calloc.free(fromBytesStatusPtr);\n");
                                out.push_str(
                                    "      if (fromBytesCode != _uniFfiRustCallStatusSuccess) {\n",
                                );
                                out.push_str(
                                    "        final ffi.Pointer<_UniFfiRustBuffer> fromBytesErrBufPtr = calloc<_UniFfiRustBuffer>();\n",
                                );
                                out.push_str(
                                    "        fromBytesErrBufPtr.ref\n          ..capacity = fromBytesErrBuf.capacity\n          ..len = fromBytesErrBuf.len\n          ..data = fromBytesErrBuf.data;\n",
                                );
                                out.push_str(
                                    "        rustRetBufferPtrs.add(fromBytesErrBufPtr);\n",
                                );
                                out.push_str(
                                    "        throw StateError('UniFFI rustbuffer_from_bytes failed with status $fromBytesCode');\n",
                                );
                                out.push_str("      }\n");
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.capacity;\n",
                                    offset
                                ));
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.len;\n",
                                    offset + 1
                                ));
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.ptr = {arg_name}RustBuffer.data.cast<ffi.Void>();\n",
                                    offset + 2
                                ));
                            }
                            _ => {
                                let Some(union_field) = ffibuffer_primitive_union_field(ffi_type)
                                else {
                                    out.push_str(&format!(
                                        "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                        ctor.name
                                    ));
                                    continue;
                                };
                                if union_field == "ptr" {
                                    out.push_str(&format!(
                                        "      (argBuf + {}).ref.ptr = {}.cast<ffi.Void>();\n",
                                        offset, arg_name
                                    ));
                                } else {
                                    let value_expr = if union_field == "i8"
                                        && matches!(
                                            runtime_unwrapped_type(&arg.type_),
                                            Type::Boolean
                                        ) {
                                        format!("{arg_name} ? 1 : 0")
                                    } else {
                                        arg_name.clone()
                                    };
                                    out.push_str(&format!(
                                        "      (argBuf + {}).ref.{union_field} = {value_expr};\n",
                                        offset
                                    ));
                                }
                            }
                        }
                    }
                    out.push_str(&format!("      {ctor_field}(argBuf, returnBuf);\n"));
                    out.push_str(&format!(
                        "      final int statusCode = (returnBuf + {}).ref.i8;\n",
                        return_ffi_elements
                    ));
                    out.push_str("      if (statusCode != _uniFfiRustCallStatusSuccess) {\n");
                    out.push_str(&format!(
                        "        final ffi.Pointer<_UniFfiRustBuffer> errBufPtr = calloc<_UniFfiRustBuffer>();\n        errBufPtr.ref\n          ..capacity = (returnBuf + {}).ref.u64\n          ..len = (returnBuf + {}).ref.u64\n          ..data = (returnBuf + {}).ref.ptr.cast<ffi.Uint8>();\n",
                        return_ffi_elements + 1,
                        return_ffi_elements + 2,
                        return_ffi_elements + 3
                    ));
                    out.push_str("        rustRetBufferPtrs.add(errBufPtr);\n");
                    if let Some(throws_name) = ctor
                        .throws_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .map(to_upper_camel)
                    {
                        let exception_name = format!("{throws_name}Exception");
                        out.push_str("        if (statusCode == _uniFfiRustCallStatusError) {\n");
                        out.push_str(
                            "          final Uint8List errBytes = errBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(errBufPtr.ref.data.asTypedList(errBufPtr.ref.len));\n",
                        );
                        out.push_str(&format!(
                            "          throw _uniffiLift{exception_name}(errBytes);\n"
                        ));
                        out.push_str("        }\n");
                    }
                    out.push_str(
                        "        throw StateError('UniFFI ffibuffer call failed with status $statusCode');\n",
                    );
                    out.push_str("      }\n");
                    match ffi_return_type {
                        FfiType::Handle | FfiType::UInt64 | FfiType::Int64 => {
                            out.push_str("      final int handle = (returnBuf + 0).ref.u64;\n");
                            out.push_str(&format!("      return {object_name}._(this, handle);\n"));
                        }
                        _ => {
                            out.push_str(&format!(
                                "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                ctor.name
                            ));
                        }
                    }
                    out.push_str("    } finally {\n");
                    out.push_str("      for (final ptr in foreignArgPtrs) {\n");
                    out.push_str("        if (ptr != ffi.nullptr) {\n");
                    out.push_str("          calloc.free(ptr);\n");
                    out.push_str("        }\n");
                    out.push_str("      }\n");
                    out.push_str("      for (final bufPtr in rustRetBufferPtrs) {\n");
                    out.push_str("        if (bufPtr.ref.data == ffi.nullptr && bufPtr.ref.len == 0 && bufPtr.ref.capacity == 0) {\n");
                    out.push_str("          continue;\n");
                    out.push_str("        }\n");
                    out.push_str(
                        "        final ffi.Pointer<_UniFfiRustCallStatus> freeStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                    );
                    out.push_str(
                        "        freeStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n",
                    );
                    out.push_str("        freeStatusPtr.ref.errorBuf\n");
                    out.push_str("          ..capacity = 0\n");
                    out.push_str("          ..len = 0\n");
                    out.push_str("          ..data = ffi.nullptr;\n");
                    out.push_str("        _uniFfiRustBufferFree(bufPtr.ref, freeStatusPtr);\n");
                    out.push_str("        calloc.free(freeStatusPtr);\n");
                    out.push_str("        calloc.free(bufPtr);\n");
                    out.push_str("      }\n");
                    out.push_str("      calloc.free(argBuf);\n");
                    out.push_str("      calloc.free(returnBuf);\n");
                    out.push_str("    }\n");
                    out.push_str("  }\n");
                    continue;
                }
                out.push('\n');
                out.push_str(&format!("  {object_name} {ctor_method}({dart_args}) {{\n"));
                out.push_str(&format!(
                    "    throw UnsupportedError('{escaped_reason} ({})');\n",
                    ctor.name
                ));
                out.push_str("  }\n");
                continue;
            }
            if !ctor
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))
            {
                continue;
            }
            let ctor_camel = to_upper_camel(&ctor.name);
            let ctor_field = format!("_{}Ctor{}", object_lower, ctor_camel);
            let ctor_method = format!("{}Create{}", object_lower, ctor_camel);
            let ctor_symbol = ctor
                .ffi_symbol
                .clone()
                .unwrap_or_else(|| format!("{}_{}", object_symbol, dart_identifier(&ctor.name)));
            let is_throwing = ctor.throws_type.is_some();
            let native_return = if is_throwing {
                "ffi.Pointer<Utf8>"
            } else {
                "ffi.Uint64"
            };
            let dart_return = if is_throwing {
                "ffi.Pointer<Utf8>"
            } else {
                "int"
            };
            let mut native_args = Vec::new();
            let mut dart_args = Vec::new();
            let mut dart_ffi_args = Vec::new();
            let mut call_args = Vec::new();
            let mut pre_call = Vec::new();
            let mut post_call = Vec::new();
            let mut signature_compatible = true;
            for arg in &ctor.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                let Some(native_ty) = map_runtime_native_ffi_type(&arg.type_, records, enums)
                else {
                    signature_compatible = false;
                    break;
                };
                let Some(dart_ffi_ty) = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                else {
                    signature_compatible = false;
                    break;
                };
                native_args.push(format!("{native_ty} {arg_name}"));
                dart_ffi_args.push(format!("{dart_ffi_ty} {arg_name}"));
                dart_args.push(format!(
                    "{} {arg_name}",
                    map_uniffi_type_to_dart(&arg.type_)
                ));
                append_runtime_arg_marshalling(
                    &arg_name,
                    &arg.type_,
                    enums,
                    &mut pre_call,
                    &mut post_call,
                    &mut call_args,
                );
            }
            if !signature_compatible {
                continue;
            }
            out.push('\n');
            out.push_str(&format!(
                "  late final {dart_return} Function({}) {ctor_field} = _lib.lookupFunction<{native_return} Function({}), {dart_return} Function({})>('{ctor_symbol}');\n",
                dart_ffi_args.join(", "),
                native_args.join(", "),
                dart_ffi_args.join(", ")
            ));
            out.push('\n');
            out.push_str(&format!(
                "  {object_name} {ctor_method}({}) {{\n",
                dart_args.join(", ")
            ));
            for line in &pre_call {
                out.push_str(line);
            }
            if !post_call.is_empty() {
                out.push_str("    try {\n");
            }
            if is_throwing {
                let Some(throws_name) = ctor
                    .throws_type
                    .as_ref()
                    .and_then(enum_name_from_type)
                    .map(to_upper_camel)
                else {
                    continue;
                };
                out.push_str(&format!(
                    "    final ffi.Pointer<Utf8> resultPtr = {ctor_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                out.push_str(&format!(
                    "      throw StateError('Rust returned null for {}');\n",
                    ctor_symbol
                ));
                out.push_str("    }\n");
                out.push_str("    final String payload;\n");
                out.push_str("    try {\n");
                out.push_str("      payload = resultPtr.toDartString();\n");
                out.push_str("    } finally {\n");
                out.push_str("      _rustStringFree(resultPtr);\n");
                out.push_str("    }\n");
                out.push_str(
                    "    final Map<String, dynamic> envelope = jsonDecode(payload) as Map<String, dynamic>;\n",
                );
                out.push_str("    final Object? errRaw = envelope['err'];\n");
                out.push_str("    if (errRaw != null) {\n");
                out.push_str(&format!(
                    "      throw {}ExceptionFfiCodec.decode(errRaw);\n",
                    throws_name
                ));
                out.push_str("    }\n");
                out.push_str("    final Object? okRaw = envelope['ok'];\n");
                out.push_str("    final int handle = (okRaw as num).toInt();\n");
                out.push_str(&format!("    return {object_name}._(this, handle);\n"));
            } else {
                out.push_str(&format!(
                    "    final int handle = {ctor_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str(&format!("    return {object_name}._(this, handle);\n"));
            }
            if !post_call.is_empty() {
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
        }

        for method in &object.methods {
            if let Some(reason) = method.runtime_unsupported.as_ref() {
                let method_invoke =
                    format!("{}Invoke{}", object_lower, to_upper_camel(&method.name));
                let value_return_type = method
                    .return_type
                    .as_ref()
                    .map(map_uniffi_type_to_dart)
                    .unwrap_or_else(|| "void".to_string());
                let signature_return_type = if method.is_async {
                    format!("Future<{value_return_type}>")
                } else {
                    value_return_type
                };
                let mut dart_args = vec!["int handle".to_string()];
                dart_args.extend(method.args.iter().map(|arg| {
                    let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                    format!("{} {arg_name}", map_uniffi_type_to_dart(&arg.type_))
                }));
                let escaped_reason = reason.replace('\'', "\\'");
                let ffibuffer_eligible = is_ffibuffer_eligible_object_member(method);
                if ffibuffer_eligible {
                    let method_camel = to_upper_camel(&method.name);
                    let method_field = format!("_{}{}FfiBuffer", object_lower, method_camel);
                    let method_symbol = method
                        .ffi_symbol
                        .as_deref()
                        .unwrap_or(&method.name)
                        .to_string();
                    let ffibuffer_symbol = ffibuffer_symbol_name(&method_symbol);
                    let ffi_return_type = method
                        .ffi_return_type
                        .clone()
                        .or_else(|| {
                            method
                                .return_type
                                .as_ref()
                                .and_then(ffibuffer_ffi_type_from_uniffi_type)
                        })
                        .unwrap_or(FfiType::VoidPointer);
                    let Some(return_ffi_elements) = ffibuffer_element_count(&ffi_return_type)
                    else {
                        out.push('\n');
                        out.push_str(&format!(
                            "  {signature_return_type} {method_invoke}({}) {{\n",
                            dart_args.join(", ")
                        ));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            method.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    };
                    let ffi_arg_types = if method.ffi_arg_types.len() == method.args.len() + 1 {
                        method.ffi_arg_types.clone()
                    } else {
                        let mut inferred = vec![FfiType::Handle];
                        inferred.extend(
                            method
                                .args
                                .iter()
                                .filter_map(|a| ffibuffer_ffi_type_from_uniffi_type(&a.type_)),
                        );
                        inferred
                    };
                    let mut arg_ffi_offsets = Vec::new();
                    let mut arg_cursor = 0usize;
                    let mut signature_compatible = ffi_arg_types.len() == method.args.len() + 1;
                    if signature_compatible {
                        for ffi_type in &ffi_arg_types {
                            let Some(size) = ffibuffer_element_count(ffi_type) else {
                                signature_compatible = false;
                                break;
                            };
                            arg_ffi_offsets.push(arg_cursor);
                            arg_cursor += size;
                        }
                    }
                    if !signature_compatible {
                        out.push('\n');
                        out.push_str(&format!(
                            "  {signature_return_type} {method_invoke}({}) {{\n",
                            dart_args.join(", ")
                        ));
                        out.push_str(&format!(
                            "    throw UnsupportedError('{escaped_reason} ({})');\n",
                            method.name
                        ));
                        out.push_str("  }\n");
                        continue;
                    }

                    out.push('\n');
                    out.push_str(&format!(
                        "  late final void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr) {method_field} = _lib.lookupFunction<ffi.Void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr), void Function(ffi.Pointer<_UniFfiFfiBufferElement> argPtr, ffi.Pointer<_UniFfiFfiBufferElement> returnPtr)>('{ffibuffer_symbol}');\n"
                    ));
                    out.push('\n');
                    out.push_str(&format!(
                        "  {signature_return_type} {method_invoke}({}) {{\n",
                        dart_args.join(", ")
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> argBuf = calloc<_UniFfiFfiBufferElement>({arg_cursor});\n"
                    ));
                    out.push_str(&format!(
                        "    final ffi.Pointer<_UniFfiFfiBufferElement> returnBuf = calloc<_UniFfiFfiBufferElement>({});\n",
                        return_ffi_elements + 4
                    ));
                    out.push_str("    final foreignArgPtrs = <ffi.Pointer<ffi.Uint8>>[];\n");
                    out.push_str(
                        "    final rustRetBufferPtrs = <ffi.Pointer<_UniFfiRustBuffer>>[];\n",
                    );
                    out.push_str("    try {\n");

                    if let Some(handle_ffi_type) = ffi_arg_types.first() {
                        if let Some(handle_field) = ffibuffer_primitive_union_field(handle_ffi_type)
                        {
                            if handle_field == "ptr" {
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.ptr = handle.cast<ffi.Void>();\n",
                                    arg_ffi_offsets[0]
                                ));
                            } else {
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.{handle_field} = handle;\n",
                                    arg_ffi_offsets[0]
                                ));
                            }
                        } else {
                            out.push_str(&format!(
                                "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                method.name
                            ));
                        }
                    }

                    for (((arg, ffi_type), offset), _idx) in method
                        .args
                        .iter()
                        .zip(ffi_arg_types.iter().skip(1))
                        .zip(arg_ffi_offsets.iter().skip(1))
                        .zip(0..)
                    {
                        let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                        match ffi_type {
                            FfiType::RustBuffer(_) => {
                                let encode_expr = match &arg.type_ {
                                    Type::Record { name, .. } | Type::Enum { name, .. } => {
                                        format!("_uniffiEncode{}({arg_name})", to_upper_camel(name))
                                    }
                                    _ => {
                                        out.push_str(&format!(
                                            "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                            method.name
                                        ));
                                        continue;
                                    }
                                };
                                out.push_str(&format!(
                                    "      final Uint8List {arg_name}Bytes = {encode_expr};\n"
                                ));
                                out.push_str(&format!(
                                    "      final ffi.Pointer<ffi.Uint8> {arg_name}Ptr = {arg_name}Bytes.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>({arg_name}Bytes.length);\n"
                                ));
                                out.push_str(&format!(
                                    "      if ({arg_name}Bytes.isNotEmpty) {{ {arg_name}Ptr.asTypedList({arg_name}Bytes.length).setAll(0, {arg_name}Bytes); }}\n"
                                ));
                                out.push_str(&format!(
                                    "      foreignArgPtrs.add({arg_name}Ptr);\n"
                                ));
                                out.push_str(
                                    "      final ffi.Pointer<_UniFfiRustCallStatus> fromBytesStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                                );
                                out.push_str(
                                    "      fromBytesStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n",
                                );
                                out.push_str("      fromBytesStatusPtr.ref.errorBuf\n");
                                out.push_str("        ..capacity = 0\n");
                                out.push_str("        ..len = 0\n");
                                out.push_str("        ..data = ffi.nullptr;\n");
                                out.push_str(&format!(
                                    "      final ffi.Pointer<_UniFfiForeignBytes> {arg_name}ForeignPtr = calloc<_UniFfiForeignBytes>();\n"
                                ));
                                out.push_str(&format!(
                                    "      {arg_name}ForeignPtr.ref\n        ..len = {arg_name}Bytes.length\n        ..data = {arg_name}Ptr;\n"
                                ));
                                out.push_str(&format!(
                                    "      final _UniFfiRustBuffer {arg_name}RustBuffer = _uniFfiRustBufferFromBytes({arg_name}ForeignPtr.ref, fromBytesStatusPtr);\n"
                                ));
                                out.push_str(&format!(
                                    "      calloc.free({arg_name}ForeignPtr);\n"
                                ));
                                out.push_str(
                                    "      final int fromBytesCode = fromBytesStatusPtr.ref.code;\n",
                                );
                                out.push_str(
                                    "      final _UniFfiRustBuffer fromBytesErrBuf = fromBytesStatusPtr.ref.errorBuf;\n",
                                );
                                out.push_str("      calloc.free(fromBytesStatusPtr);\n");
                                out.push_str(
                                    "      if (fromBytesCode != _uniFfiRustCallStatusSuccess) {\n",
                                );
                                out.push_str(
                                    "        final ffi.Pointer<_UniFfiRustBuffer> fromBytesErrBufPtr = calloc<_UniFfiRustBuffer>();\n",
                                );
                                out.push_str(
                                    "        fromBytesErrBufPtr.ref\n          ..capacity = fromBytesErrBuf.capacity\n          ..len = fromBytesErrBuf.len\n          ..data = fromBytesErrBuf.data;\n",
                                );
                                out.push_str(
                                    "        rustRetBufferPtrs.add(fromBytesErrBufPtr);\n",
                                );
                                out.push_str(
                                    "        throw StateError('UniFFI rustbuffer_from_bytes failed with status $fromBytesCode');\n",
                                );
                                out.push_str("      }\n");
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.capacity;\n",
                                    offset
                                ));
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.u64 = {arg_name}RustBuffer.len;\n",
                                    offset + 1
                                ));
                                out.push_str(&format!(
                                    "      (argBuf + {}).ref.ptr = {arg_name}RustBuffer.data.cast<ffi.Void>();\n",
                                    offset + 2
                                ));
                            }
                            _ => {
                                let Some(union_field) = ffibuffer_primitive_union_field(ffi_type)
                                else {
                                    out.push_str(&format!(
                                        "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                        method.name
                                    ));
                                    continue;
                                };
                                if union_field == "ptr" {
                                    out.push_str(&format!(
                                        "      (argBuf + {}).ref.ptr = {}.cast<ffi.Void>();\n",
                                        offset, arg_name
                                    ));
                                } else {
                                    let value_expr = if union_field == "i8"
                                        && matches!(
                                            runtime_unwrapped_type(&arg.type_),
                                            Type::Boolean
                                        ) {
                                        format!("{arg_name} ? 1 : 0")
                                    } else {
                                        arg_name.clone()
                                    };
                                    out.push_str(&format!(
                                        "      (argBuf + {}).ref.{union_field} = {value_expr};\n",
                                        offset
                                    ));
                                }
                            }
                        }
                    }

                    out.push_str(&format!("      {method_field}(argBuf, returnBuf);\n"));
                    out.push_str(&format!(
                        "      final int statusCode = (returnBuf + {}).ref.i8;\n",
                        return_ffi_elements
                    ));
                    out.push_str("      if (statusCode != _uniFfiRustCallStatusSuccess) {\n");
                    out.push_str(&format!(
                        "        final ffi.Pointer<_UniFfiRustBuffer> errBufPtr = calloc<_UniFfiRustBuffer>();\n        errBufPtr.ref\n          ..capacity = (returnBuf + {}).ref.u64\n          ..len = (returnBuf + {}).ref.u64\n          ..data = (returnBuf + {}).ref.ptr.cast<ffi.Uint8>();\n",
                        return_ffi_elements + 1,
                        return_ffi_elements + 2,
                        return_ffi_elements + 3
                    ));
                    out.push_str("        rustRetBufferPtrs.add(errBufPtr);\n");
                    if let Some(throws_name) = method
                        .throws_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .map(to_upper_camel)
                    {
                        let exception_name = format!("{throws_name}Exception");
                        out.push_str("        if (statusCode == _uniFfiRustCallStatusError) {\n");
                        out.push_str(
                            "          final Uint8List errBytes = errBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(errBufPtr.ref.data.asTypedList(errBufPtr.ref.len));\n",
                        );
                        out.push_str(&format!(
                            "          throw _uniffiLift{exception_name}(errBytes);\n"
                        ));
                        out.push_str("        }\n");
                    }
                    out.push_str(
                        "        throw StateError('UniFFI ffibuffer call failed with status $statusCode');\n",
                    );
                    out.push_str("      }\n");

                    match method.return_type.as_ref() {
                        None => out.push_str("      return;\n"),
                        Some(Type::Boolean) => {
                            out.push_str("      return (returnBuf + 0).ref.i8 == 1;\n");
                        }
                        Some(ret_type) if is_runtime_object_type(ret_type) => {
                            let lift = render_object_lift_expr(
                                ret_type,
                                "(returnBuf + 0).ref.u64",
                                local_module_path,
                                "this",
                            );
                            out.push_str(&format!("      return {lift};\n"));
                        }
                        Some(ret_type) => match &ffi_return_type {
                            FfiType::RustBuffer(_) => {
                                let decode_expr = match ret_type {
                                    Type::Record { name, .. } | Type::Enum { name, .. } => {
                                        format!("_uniffiDecode{}(retBytes)", to_upper_camel(name))
                                    }
                                    _ => {
                                        out.push_str(&format!(
                                            "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                            method.name
                                        ));
                                        String::new()
                                    }
                                };
                                if !decode_expr.is_empty() {
                                    out.push_str(
                                        "      final ffi.Pointer<_UniFfiRustBuffer> retBufPtr = calloc<_UniFfiRustBuffer>();\n",
                                    );
                                    out.push_str(
                                        "      retBufPtr.ref\n        ..capacity = (returnBuf + 0).ref.u64\n        ..len = (returnBuf + 1).ref.u64\n        ..data = (returnBuf + 2).ref.ptr.cast<ffi.Uint8>();\n",
                                    );
                                    out.push_str("      rustRetBufferPtrs.add(retBufPtr);\n");
                                    out.push_str(
                                        "      final Uint8List retBytes = retBufPtr.ref.len == 0 ? Uint8List(0) : Uint8List.fromList(retBufPtr.ref.data.asTypedList(retBufPtr.ref.len));\n",
                                    );
                                    out.push_str(&format!("      return {decode_expr};\n"));
                                }
                            }
                            _ => {
                                let Some(union_field) =
                                    ffibuffer_primitive_union_field(&ffi_return_type)
                                else {
                                    out.push_str(&format!(
                                        "      throw UnsupportedError('{escaped_reason} ({})');\n",
                                        method.name
                                    ));
                                    out.push_str("      return;\n");
                                    out.push_str("    } finally {\n");
                                    out.push_str("      calloc.free(argBuf);\n");
                                    out.push_str("      calloc.free(returnBuf);\n");
                                    out.push_str("    }\n");
                                    out.push_str("  }\n");
                                    continue;
                                };
                                if union_field == "ptr" {
                                    out.push_str("      return (returnBuf + 0).ref.ptr;\n");
                                } else {
                                    out.push_str(&format!(
                                        "      return (returnBuf + 0).ref.{union_field};\n"
                                    ));
                                }
                            }
                        },
                    }

                    out.push_str("    } finally {\n");
                    out.push_str("      for (final ptr in foreignArgPtrs) {\n");
                    out.push_str("        if (ptr != ffi.nullptr) {\n");
                    out.push_str("          calloc.free(ptr);\n");
                    out.push_str("        }\n");
                    out.push_str("      }\n");
                    out.push_str("      for (final bufPtr in rustRetBufferPtrs) {\n");
                    out.push_str("        if (bufPtr.ref.data == ffi.nullptr && bufPtr.ref.len == 0 && bufPtr.ref.capacity == 0) {\n");
                    out.push_str("          continue;\n");
                    out.push_str("        }\n");
                    out.push_str(
                        "        final ffi.Pointer<_UniFfiRustCallStatus> freeStatusPtr = calloc<_UniFfiRustCallStatus>();\n",
                    );
                    out.push_str(
                        "        freeStatusPtr.ref.code = _uniFfiRustCallStatusSuccess;\n",
                    );
                    out.push_str("        freeStatusPtr.ref.errorBuf\n");
                    out.push_str("          ..capacity = 0\n");
                    out.push_str("          ..len = 0\n");
                    out.push_str("          ..data = ffi.nullptr;\n");
                    out.push_str("        _uniFfiRustBufferFree(bufPtr.ref, freeStatusPtr);\n");
                    out.push_str("        calloc.free(freeStatusPtr);\n");
                    out.push_str("        calloc.free(bufPtr);\n");
                    out.push_str("      }\n");
                    out.push_str("      calloc.free(argBuf);\n");
                    out.push_str("      calloc.free(returnBuf);\n");
                    out.push_str("    }\n");
                    out.push_str("  }\n");
                    continue;
                }
                out.push('\n');
                if method.is_async {
                    out.push_str(&format!(
                        "  {signature_return_type} {method_invoke}({}) async {{\n",
                        dart_args.join(", ")
                    ));
                } else {
                    out.push_str(&format!(
                        "  {signature_return_type} {method_invoke}({}) {{\n",
                        dart_args.join(", ")
                    ));
                }
                out.push_str(&format!(
                    "    throw UnsupportedError('{escaped_reason} ({})');\n",
                    method.name
                ));
                out.push_str("  }\n");
                continue;
            }
            let has_callback_args = has_runtime_callback_args_in_args(
                &method.args,
                callback_interfaces,
                records,
                enums,
            );
            let supported_return = method
                .return_type
                .as_ref()
                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                .unwrap_or(true);
            let supports_runtime_args = method
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums));
            if !has_callback_args && (!supported_return || !supports_runtime_args) {
                continue;
            }
            let method_camel = to_upper_camel(&method.name);
            let method_field = format!("_{}{}", object_lower, method_camel);
            let method_invoke = format!("{}Invoke{}", object_lower, method_camel);
            let method_symbol = method
                .ffi_symbol
                .clone()
                .unwrap_or_else(|| format!("{}_{}", object_symbol, dart_identifier(&method.name)));
            let is_throwing = method.throws_type.is_some();

            let mut native_args = vec!["ffi.Uint64 handle".to_string()];
            let mut dart_ffi_args = vec!["int handle".to_string()];
            let mut dart_args = vec!["int handle".to_string()];
            let mut call_args = vec!["handle".to_string()];
            let mut pre_call = Vec::new();
            let mut post_call = Vec::new();
            let mut signature_compatible = true;
            for arg in &method.args {
                let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
                dart_args.push(format!(
                    "{} {arg_name}",
                    map_uniffi_type_to_dart(&arg.type_)
                ));
                if has_callback_args {
                    if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
                        let init_done_field = callback_init_done_field_name(callback_name);
                        let bridge_name = callback_bridge_class_name(callback_name);
                        let handle_name = format!("{arg_name}Handle");
                        native_args.push(format!("ffi.Uint64 {arg_name}"));
                        dart_ffi_args.push(format!("int {arg_name}"));
                        pre_call.push(format!("    {init_done_field};\n"));
                        pre_call.push(format!(
                            "    final int {handle_name} = {bridge_name}.instance.register({arg_name});\n"
                        ));
                        post_call.push(format!(
                            "    {bridge_name}.instance.release({handle_name});\n"
                        ));
                        call_args.push(handle_name);
                        continue;
                    }
                    let Some(native_ty) = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    let Some(dart_ffi_ty) = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    native_args.push(format!("{native_ty} {arg_name}"));
                    dart_ffi_args.push(format!("{dart_ffi_ty} {arg_name}"));
                    append_runtime_arg_marshalling(
                        &arg_name,
                        &arg.type_,
                        enums,
                        &mut pre_call,
                        &mut post_call,
                        &mut call_args,
                    );
                } else {
                    let Some(native_ty) = map_runtime_native_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    let Some(dart_ffi_ty) = map_runtime_dart_ffi_type(&arg.type_, records, enums)
                    else {
                        signature_compatible = false;
                        break;
                    };
                    native_args.push(format!("{native_ty} {arg_name}"));
                    dart_ffi_args.push(format!("{dart_ffi_ty} {arg_name}"));
                    append_runtime_arg_marshalling(
                        &arg_name,
                        &arg.type_,
                        enums,
                        &mut pre_call,
                        &mut post_call,
                        &mut call_args,
                    );
                }
            }
            if !signature_compatible {
                continue;
            }
            let return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let native_return = if is_throwing {
                "ffi.Pointer<Utf8>".to_string()
            } else {
                method
                    .return_type
                    .as_ref()
                    .and_then(|t| map_runtime_native_ffi_type(t, records, enums))
                    .unwrap_or("ffi.Void")
                    .to_string()
            };
            let dart_return = if is_throwing {
                "ffi.Pointer<Utf8>".to_string()
            } else {
                method
                    .return_type
                    .as_ref()
                    .and_then(|t| map_runtime_dart_ffi_type(t, records, enums))
                    .unwrap_or("void")
                    .to_string()
            };

            if is_runtime_async_rust_future_compatible_method(
                method,
                callback_interfaces,
                records,
                enums,
            ) {
                let Some(async_spec) =
                    async_rust_future_spec(method.return_type.as_ref(), records, enums)
                else {
                    continue;
                };
                let start_native_sig = format!("ffi.Uint64 Function({})", native_args.join(", "));
                let start_dart_sig = format!("int Function({})", dart_ffi_args.join(", "));
                let poll_field = format!("{method_field}RustFuturePoll");
                let cancel_field = format!("{method_field}RustFutureCancel");
                let complete_field = format!("{method_field}RustFutureComplete");
                let free_field = format!("{method_field}RustFutureFree");
                let complete_symbol = format!("rust_future_complete_{}", async_spec.suffix);
                let poll_symbol = format!("rust_future_poll_{}", async_spec.suffix);
                let cancel_symbol = format!("rust_future_cancel_{}", async_spec.suffix);
                let free_symbol = format!("rust_future_free_{}", async_spec.suffix);
                let complete_native_sig = format!(
                    "{} Function(ffi.Uint64 handle, ffi.Pointer<_RustCallStatus> outStatus)",
                    async_spec.complete_native_type
                );
                let complete_dart_sig = format!(
                    "{} Function(int handle, ffi.Pointer<_RustCallStatus> outStatus)",
                    async_spec.complete_dart_type
                );

                out.push('\n');
                out.push_str(&format!(
                    "  late final {start_dart_sig} {method_field} = _lib.lookupFunction<{start_native_sig}, {start_dart_sig}>('{method_symbol}');\n",
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData) {poll_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, ffi.Uint64 callbackData), void Function(int handle, ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 callbackData, ffi.Int8 pollResult)>> callback, int callbackData)>('{poll_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle) {cancel_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{cancel_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final {complete_dart_sig} {complete_field} = _lib.lookupFunction<{complete_native_sig}, {complete_dart_sig}>('{complete_symbol}');\n"
                ));
                out.push_str(&format!(
                    "  late final void Function(int handle) {free_field} = _lib.lookupFunction<ffi.Void Function(ffi.Uint64 handle), void Function(int handle)>('{free_symbol}');\n"
                ));
                out.push('\n');
                out.push_str(&format!(
                    "  Future<{return_type}> {method_invoke}({}) async {{\n",
                    dart_args.join(", ")
                ));
                for line in &pre_call {
                    out.push_str(line);
                }
                out.push_str("    final int futureHandle;\n");
                if !post_call.is_empty() {
                    out.push_str("    try {\n");
                    out.push_str(&format!(
                        "      futureHandle = {method_field}({});\n",
                        call_args.join(", ")
                    ));
                    out.push_str("    } finally {\n");
                    for line in &post_call {
                        out.push_str(line);
                    }
                    out.push_str("    }\n");
                } else {
                    out.push_str(&format!(
                        "    futureHandle = {method_field}({});\n",
                        call_args.join(", ")
                    ));
                }
                out.push_str(
                    "    final StreamController<int> pollEvents = StreamController<int>.broadcast();\n",
                );
                out.push_str(
                    "    final callback = ffi.NativeCallable<ffi.Void Function(ffi.Uint64, ffi.Int8)>.listener((int _, int pollResult) {\n",
                );
                out.push_str("      pollEvents.add(pollResult);\n");
                out.push_str("    });\n");
                out.push_str("    try {\n");
                out.push_str(&format!(
                    "      {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
                ));
                out.push_str("      while (true) {\n");
                out.push_str("        final int pollResult = await pollEvents.stream.first;\n");
                out.push_str("        if (pollResult == _rustFuturePollReady) {\n");
                out.push_str("          break;\n");
                out.push_str("        }\n");
                out.push_str("        if (pollResult == _rustFuturePollWake) {\n");
                out.push_str(&format!(
                    "          {poll_field}(futureHandle, callback.nativeFunction, 0);\n"
                ));
                out.push_str("          continue;\n");
                out.push_str("        }\n");
                out.push_str(&format!(
                    "        throw StateError('Rust future poll returned invalid status for {}: $pollResult');\n",
                    method_symbol
                ));
                out.push_str("      }\n");
                out.push_str(
                    "      final ffi.Pointer<_RustCallStatus> outStatusPtr = calloc<_RustCallStatus>();\n",
                );
                out.push_str("      try {\n");
                if method.return_type.is_none() {
                    out.push_str(&format!(
                        "        {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(|t| is_runtime_utf8_pointer_marshaled_type(t, records, enums))
                {
                    out.push_str(&format!(
                        "        final ffi.Pointer<Utf8> resultPtr = {complete_field}(futureHandle, outStatusPtr);\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "        final {} resultValue = {complete_field}(futureHandle, outStatusPtr);\n",
                        async_spec.complete_dart_type
                    ));
                }
                out.push_str("        final int statusCode = outStatusPtr.ref.code;\n");
                out.push_str("        if (statusCode == _rustCallStatusSuccess) {\n");
                if method.return_type.is_none() {
                    out.push_str("          return;\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_string_type)
                {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_string_type)
                {
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            return resultPtr.toDartString();\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_record_type)
                {
                    let record_name = method
                        .return_type
                        .as_ref()
                        .and_then(record_name_from_type)
                        .unwrap_or("Record");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                        to_upper_camel(record_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(|t| is_runtime_enum_type(t, enums))
                {
                    let enum_name = method
                        .return_type
                        .as_ref()
                        .and_then(enum_name_from_type)
                        .unwrap_or("Enum");
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "            return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_map_with_string_key_type)
                {
                    let decode = method
                        .return_type
                        .as_ref()
                        .map(|t| render_json_decode_expr("jsonDecode(payload)", t))
                        .unwrap_or_else(|| "null".to_string());
                    out.push_str("          if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!("            return {decode};\n"));
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustStringFree(resultPtr);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_bytes_type)
                {
                    out.push_str("          final _RustBuffer resultBuf = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesFree(resultBuf);\n");
                    out.push_str("              return Uint8List(0);\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_optional_bytes_type)
                {
                    out.push_str("          final _RustBufferOpt resultOpt = resultValue;\n");
                    out.push_str("          if (resultOpt.isSome == 0) {\n");
                    out.push_str("            return null;\n");
                    out.push_str("          }\n");
                    out.push_str("          final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str(
                        "          final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n",
                    );
                    out.push_str("          final int resultLen = resultBuf.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesFree(resultBuf);\n");
                    out.push_str("              return Uint8List(0);\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid optional buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str(
                        "            return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesFree(resultBuf);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_sequence_bytes_type)
                {
                    out.push_str("          final _RustBufferVec resultVec = resultValue;\n");
                    out.push_str(
                        "          final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("          final int resultLen = resultVec.len;\n");
                    out.push_str("          if (resultData == ffi.nullptr) {\n");
                    out.push_str("            if (resultLen == 0) {\n");
                    out.push_str("              _rustBytesVecFree(resultVec);\n");
                    out.push_str("              return <Uint8List>[];\n");
                    out.push_str("            }\n");
                    out.push_str(&format!(
                        "            throw StateError('Rust returned invalid byte vector for {}');\n",
                        method_symbol
                    ));
                    out.push_str("          }\n");
                    out.push_str("          try {\n");
                    out.push_str("            final out = <Uint8List>[];\n");
                    out.push_str("            for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("              final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str(
                        "              final ffi.Pointer<ffi.Uint8> itemData = item.data;\n",
                    );
                    out.push_str("              final int itemLen = item.len;\n");
                    out.push_str("              if (itemData == ffi.nullptr) {\n");
                    out.push_str("                if (itemLen == 0) {\n");
                    out.push_str("                  out.add(Uint8List(0));\n");
                    out.push_str("                  continue;\n");
                    out.push_str("                }\n");
                    out.push_str(&format!(
                        "                throw StateError('Rust returned invalid nested buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("              }\n");
                    out.push_str("              try {\n");
                    out.push_str(
                        "                out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("              } finally {\n");
                    out.push_str("                _rustBytesFree(item);\n");
                    out.push_str("              }\n");
                    out.push_str("            }\n");
                    out.push_str("            return out;\n");
                    out.push_str("          } finally {\n");
                    out.push_str("            _rustBytesVecFree(resultVec);\n");
                    out.push_str("          }\n");
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_timestamp_type)
                {
                    out.push_str(
                        "          return DateTime.fromMicrosecondsSinceEpoch(resultValue, isUtc: true);\n",
                    );
                } else if method
                    .return_type
                    .as_ref()
                    .is_some_and(is_runtime_duration_type)
                {
                    out.push_str("          return Duration(microseconds: resultValue);\n");
                } else if let Some(ret_type) = method.return_type.as_ref() {
                    let decode = render_plain_ffi_decode_expr(ret_type, "resultValue");
                    out.push_str(&format!("          return {decode};\n"));
                }
                out.push_str("        }\n");
                out.push_str("        if (statusCode == _rustCallStatusCancelled) {\n");
                out.push_str(&format!(
                    "          throw StateError('Rust future was cancelled for {}');\n",
                    method_symbol
                ));
                out.push_str("        }\n");
                out.push_str(
                    "        final ffi.Pointer<Utf8> errorPtr = outStatusPtr.ref.errorBuf;\n",
                );
                out.push_str("        if (errorPtr != ffi.nullptr) {\n");
                out.push_str("          try {\n");
                out.push_str("            throw StateError(errorPtr.toDartString());\n");
                out.push_str("          } finally {\n");
                out.push_str("            _rustStringFree(errorPtr);\n");
                out.push_str("          }\n");
                out.push_str("        }\n");
                out.push_str(&format!(
                    "        throw StateError('Rust future failed for {} with status code: $statusCode');\n",
                    method_symbol
                ));
                out.push_str("      } finally {\n");
                out.push_str("        calloc.free(outStatusPtr);\n");
                out.push_str("      }\n");
                out.push_str("    } catch (_) {\n");
                out.push_str(&format!("      {cancel_field}(futureHandle);\n"));
                out.push_str("      rethrow;\n");
                out.push_str("    } finally {\n");
                out.push_str("      await pollEvents.close();\n");
                out.push_str("      callback.close();\n");
                out.push_str(&format!("      {free_field}(futureHandle);\n"));
                out.push_str("    }\n");
                out.push_str("  }\n");
                continue;
            }

            out.push('\n');
            out.push_str(&format!(
                "  late final {dart_return} Function({}) {method_field} = _lib.lookupFunction<{native_return} Function({}), {dart_return} Function({})>('{method_symbol}');\n",
                dart_ffi_args.join(", "),
                native_args.join(", "),
                dart_ffi_args.join(", ")
            ));
            out.push('\n');
            out.push_str(&format!(
                "  {return_type} {method_invoke}({}) {{\n",
                dart_args.join(", ")
            ));
            for line in &pre_call {
                out.push_str(line);
            }
            if !post_call.is_empty() {
                out.push_str("    try {\n");
            }
            if is_throwing {
                let Some(throws_name) = method
                    .throws_type
                    .as_ref()
                    .and_then(enum_name_from_type)
                    .map(to_upper_camel)
                else {
                    continue;
                };
                out.push_str(&format!(
                    "    final ffi.Pointer<Utf8> resultPtr = {method_field}({});\n",
                    call_args.join(", ")
                ));
                out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                out.push_str(&format!(
                    "      throw StateError('Rust returned null for {}');\n",
                    method_symbol
                ));
                out.push_str("    }\n");
                out.push_str("    final String payload;\n");
                out.push_str("    try {\n");
                out.push_str("      payload = resultPtr.toDartString();\n");
                out.push_str("    } finally {\n");
                out.push_str("      _rustStringFree(resultPtr);\n");
                out.push_str("    }\n");
                out.push_str(
                    "    final Map<String, dynamic> envelope = jsonDecode(payload) as Map<String, dynamic>;\n",
                );
                out.push_str("    final Object? errRaw = envelope['err'];\n");
                out.push_str("    if (errRaw != null) {\n");
                out.push_str(&format!(
                    "      throw {}ExceptionFfiCodec.decode(errRaw);\n",
                    throws_name
                ));
                out.push_str("    }\n");
                if let Some(ret_type) = method.return_type.as_ref() {
                    out.push_str("    final Object? okRaw = envelope['ok'];\n");
                    let decode = render_json_decode_expr("okRaw", ret_type);
                    out.push_str(&format!("    return {decode};\n"));
                } else {
                    out.push_str("    return;\n");
                }
            } else if let Some(ret) = &method.return_type {
                let call_expr = format!("{method_field}({})", call_args.join(", "));
                if is_runtime_string_type(ret) {
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      return resultPtr.toDartString();\n");
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_optional_string_type(ret) {
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      return resultPtr.toDartString();\n");
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_record_type(ret) {
                    let record_name = record_name_from_type(ret).unwrap_or("Record");
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "      return {}.fromJson(jsonDecode(payload) as Map<String, dynamic>);\n",
                        to_upper_camel(record_name)
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_enum_type(ret, enums) {
                    let enum_name = enum_name_from_type(ret).unwrap_or("Enum");
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!(
                        "      return {}FfiCodec.decode(payload);\n",
                        to_upper_camel(enum_name)
                    ));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_object_type(ret) {
                    let object_name = object_name_from_type(ret).unwrap_or("Object");
                    out.push_str(&format!(
                        "    return {}FfiCodec.lift({call_expr});\n",
                        to_upper_camel(object_name)
                    ));
                } else if is_runtime_map_with_string_key_type(ret) {
                    let decode = render_json_decode_expr("jsonDecode(payload)", ret);
                    out.push_str(&format!(
                        "    final ffi.Pointer<Utf8> resultPtr = {call_expr};\n"
                    ));
                    out.push_str("    if (resultPtr == ffi.nullptr) {\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned null for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final String payload = resultPtr.toDartString();\n");
                    out.push_str(&format!("      return {decode};\n"));
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustStringFree(resultPtr);\n");
                    out.push_str("    }\n");
                } else if is_runtime_bytes_type(ret) {
                    out.push_str(&format!("    final _RustBuffer resultBuf = {call_expr};\n"));
                    out.push_str("    final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n");
                    out.push_str("    final int resultLen = resultBuf.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      if (resultLen == 0) {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("        return Uint8List(0);\n");
                    out.push_str("      }\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned invalid buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str(
                        "      return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustBytesFree(resultBuf);\n");
                    out.push_str("    }\n");
                } else if is_runtime_optional_bytes_type(ret) {
                    out.push_str(&format!(
                        "    final _RustBufferOpt resultOpt = {call_expr};\n"
                    ));
                    out.push_str("    if (resultOpt.isSome == 0) {\n");
                    out.push_str("      return null;\n");
                    out.push_str("    }\n");
                    out.push_str("    final _RustBuffer resultBuf = resultOpt.value;\n");
                    out.push_str("    final ffi.Pointer<ffi.Uint8> resultData = resultBuf.data;\n");
                    out.push_str("    final int resultLen = resultBuf.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      if (resultLen == 0) {\n");
                    out.push_str("        _rustBytesFree(resultBuf);\n");
                    out.push_str("        return Uint8List(0);\n");
                    out.push_str("      }\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned invalid optional buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str(
                        "      return Uint8List.fromList(resultData.asTypedList(resultLen));\n",
                    );
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustBytesFree(resultBuf);\n");
                    out.push_str("    }\n");
                } else if is_runtime_sequence_bytes_type(ret) {
                    out.push_str(&format!(
                        "    final _RustBufferVec resultVec = {call_expr};\n"
                    ));
                    out.push_str(
                        "    final ffi.Pointer<_RustBuffer> resultData = resultVec.data;\n",
                    );
                    out.push_str("    final int resultLen = resultVec.len;\n");
                    out.push_str("    if (resultData == ffi.nullptr) {\n");
                    out.push_str("      if (resultLen == 0) {\n");
                    out.push_str("        _rustBytesVecFree(resultVec);\n");
                    out.push_str("        return <Uint8List>[];\n");
                    out.push_str("      }\n");
                    out.push_str(&format!(
                        "      throw StateError('Rust returned invalid byte vector for {}');\n",
                        method_symbol
                    ));
                    out.push_str("    }\n");
                    out.push_str("    try {\n");
                    out.push_str("      final out = <Uint8List>[];\n");
                    out.push_str("      for (var i = 0; i < resultLen; i++) {\n");
                    out.push_str("        final _RustBuffer item = (resultData + i).ref;\n");
                    out.push_str("        final ffi.Pointer<ffi.Uint8> itemData = item.data;\n");
                    out.push_str("        final int itemLen = item.len;\n");
                    out.push_str("        if (itemData == ffi.nullptr) {\n");
                    out.push_str("          if (itemLen == 0) {\n");
                    out.push_str("            out.add(Uint8List(0));\n");
                    out.push_str("            continue;\n");
                    out.push_str("          }\n");
                    out.push_str(&format!(
                        "          throw StateError('Rust returned invalid nested buffer for {}');\n",
                        method_symbol
                    ));
                    out.push_str("        }\n");
                    out.push_str("        try {\n");
                    out.push_str(
                        "          out.add(Uint8List.fromList(itemData.asTypedList(itemLen)));\n",
                    );
                    out.push_str("        } finally {\n");
                    out.push_str("          _rustBytesFree(item);\n");
                    out.push_str("        }\n");
                    out.push_str("      }\n");
                    out.push_str("      return out;\n");
                    out.push_str("    } finally {\n");
                    out.push_str("      _rustBytesVecFree(resultVec);\n");
                    out.push_str("    }\n");
                } else {
                    let decode = render_plain_ffi_decode_expr(ret, &call_expr);
                    out.push_str(&format!("    return {decode};\n"));
                }
            } else {
                out.push_str(&format!("    {method_field}({});\n", call_args.join(", ")));
            }
            if !post_call.is_empty() {
                out.push_str("    } finally {\n");
                for line in &post_call {
                    out.push_str(line);
                }
                out.push_str("    }\n");
            }
            out.push_str("  }\n");
        }
    }

    out
}

#[derive(Debug, Clone, Default)]
struct ApiOverrides {
    rename: HashMap<String, String>,
    exclude: HashSet<String>,
}

impl ApiOverrides {
    fn new(rename: &HashMap<String, String>, exclude: &[String]) -> Self {
        Self {
            rename: rename.clone(),
            exclude: exclude.iter().cloned().collect(),
        }
    }

    fn fn_key(name: &str) -> String {
        name.to_string()
    }

    fn object_key(object: &str) -> String {
        object.to_string()
    }

    fn object_member_key(object: &str, member: &str) -> String {
        format!("{object}.{member}")
    }

    fn renamed_or_default(&self, key: &str, default: impl FnOnce() -> String) -> String {
        self.rename.get(key).cloned().unwrap_or_else(default)
    }

    fn excluded(&self, key: &str) -> bool {
        self.exclude.contains(key)
    }
}

fn crate_name_from_module_path(module_path: &str) -> &str {
    module_path.split("::").next().unwrap_or(module_path)
}

fn collect_external_import_uris(
    local_module_path: &str,
    external_packages: &HashMap<String, String>,
    functions: &[UdlFunction],
    objects: &[UdlObject],
) -> Vec<String> {
    if local_module_path.is_empty() || external_packages.is_empty() {
        return Vec::new();
    }

    let local_crate = crate_name_from_module_path(local_module_path);
    let mut crates = HashSet::new();

    for f in functions {
        if let Some(t) = f.return_type.as_ref() {
            collect_external_crates_from_type(t, local_crate, &mut crates);
        }
        if let Some(t) = f.throws_type.as_ref() {
            collect_external_crates_from_type(t, local_crate, &mut crates);
        }
        for a in &f.args {
            collect_external_crates_from_type(&a.type_, local_crate, &mut crates);
        }
    }

    for o in objects {
        for ctor in &o.constructors {
            if let Some(t) = ctor.throws_type.as_ref() {
                collect_external_crates_from_type(t, local_crate, &mut crates);
            }
            for a in &ctor.args {
                collect_external_crates_from_type(&a.type_, local_crate, &mut crates);
            }
        }
        for m in &o.methods {
            if let Some(t) = m.return_type.as_ref() {
                collect_external_crates_from_type(t, local_crate, &mut crates);
            }
            if let Some(t) = m.throws_type.as_ref() {
                collect_external_crates_from_type(t, local_crate, &mut crates);
            }
            for a in &m.args {
                collect_external_crates_from_type(&a.type_, local_crate, &mut crates);
            }
        }
    }

    let mut uris = crates
        .into_iter()
        .filter_map(|crate_name| external_packages.get(crate_name).cloned())
        .collect::<Vec<_>>();
    uris.sort();
    uris
}

fn collect_external_crates_from_type<'a>(
    type_: &'a Type,
    local_crate: &str,
    out: &mut HashSet<&'a str>,
) {
    match type_ {
        Type::Object { module_path, .. }
        | Type::Record { module_path, .. }
        | Type::Enum { module_path, .. }
        | Type::CallbackInterface { module_path, .. }
        | Type::Custom { module_path, .. } => {
            let crate_name = crate_name_from_module_path(module_path);
            if crate_name != local_crate {
                out.insert(crate_name);
            }
            if let Type::Custom { builtin, .. } = type_ {
                collect_external_crates_from_type(builtin, local_crate, out);
            }
        }
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            collect_external_crates_from_type(inner_type, local_crate, out);
        }
        Type::Map {
            key_type,
            value_type,
        } => {
            collect_external_crates_from_type(key_type, local_crate, out);
            collect_external_crates_from_type(value_type, local_crate, out);
        }
        _ => {}
    }
}

fn render_object_classes(
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    api_overrides: &ApiOverrides,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let mut out = String::new();
    for object in objects {
        if api_overrides.excluded(&ApiOverrides::object_key(&object.name)) {
            continue;
        }
        let object_name = api_overrides
            .renamed_or_default(&ApiOverrides::object_key(&object.name), || {
                to_upper_camel(&object.name)
            });
        let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
        let free_field = format!("_{}Free", object_lower);
        let token_name = format!("_{}FinalizerToken", object_name);
        out.push('\n');
        out.push_str(&format!(
            "final class {token_name} {{\n  const {token_name}(this.free, this.handle);\n  final void Function(int) free;\n  final int handle;\n}}\n\n"
        ));
        out.push_str(&render_doc_comment(object.docstring.as_deref(), ""));
        if object.trait_methods.ord_cmp.is_some() {
            out.push_str(&format!(
                "final class {object_name} implements Comparable<{object_name}> {{\n"
            ));
        } else {
            out.push_str(&format!("final class {object_name} {{\n"));
        }
        out.push_str(&format!("  {object_name}._(this._ffi, this._handle) {{\n"));
        out.push_str(&format!(
            "    _finalizer.attach(this, {token_name}(_ffi.{free_field}, _handle), detach: this);\n"
        ));
        out.push_str("  }\n\n");
        out.push_str(&format!("  final {ffi_class_name} _ffi;\n"));
        out.push_str("  int _handle;\n");
        out.push_str("  bool _closed = false;\n\n");
        out.push_str(&format!(
            "  static final Finalizer<{token_name}> _finalizer = Finalizer((token) {{\n"
        ));
        out.push_str("    token.free(token.handle);\n");
        out.push_str("  });\n\n");
        out.push_str("  bool get isClosed => _closed;\n\n");
        out.push_str("  void close() {\n");
        out.push_str("    if (_closed) {\n");
        out.push_str("      return;\n");
        out.push_str("    }\n");
        out.push_str("    _closed = true;\n");
        out.push_str("    _finalizer.detach(this);\n");
        out.push_str(&format!("    _ffi.{free_field}(_handle);\n"));
        out.push_str("  }\n\n");
        out.push_str("  void _ensureOpen() {\n");
        out.push_str("    if (_closed) {\n");
        out.push_str(&format!(
            "      throw StateError('{object_name} is closed');\n"
        ));
        out.push_str("    }\n");
        out.push_str("  }\n\n");

        for ctor in &object.constructors {
            if api_overrides.excluded(&ApiOverrides::object_member_key(&object.name, &ctor.name)) {
                continue;
            }
            if !ctor
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))
            {
                continue;
            }
            let ctor_camel = to_upper_camel(&ctor.name);
            let ctor_invoker = format!("{}Create{}", object_lower, ctor_camel);
            let static_name = safe_dart_identifier(&api_overrides.renamed_or_default(
                &ApiOverrides::object_member_key(&object.name, &ctor.name),
                || {
                    if ctor.name == "new" {
                        "create".to_string()
                    } else {
                        to_lower_camel(&ctor.name)
                    }
                },
            ));
            let args = render_callable_args_signature(&ctor.args, enums);
            let arg_names = render_callable_arg_names(&ctor.args);
            let invoke_expr = format!("_bindings().{ctor_invoker}({arg_names})");
            let signature_return = if ctor.is_async {
                format!("Future<{object_name}>")
            } else {
                object_name.clone()
            };
            out.push_str(&render_doc_comment(ctor.docstring.as_deref(), "  "));
            out.push_str(&format!(
                "  static {signature_return} {static_name}({args}) {{\n"
            ));
            if ctor.is_async {
                out.push_str(&format!("    return Future(() => {invoke_expr});\n"));
            } else {
                out.push_str(&format!("    return {invoke_expr};\n"));
            }
            out.push_str("  }\n\n");
        }

        for method in &object.methods {
            if is_uniffi_trait_method_name(&method.name) {
                continue;
            }
            if api_overrides.excluded(&ApiOverrides::object_member_key(&object.name, &method.name))
            {
                continue;
            }
            let has_callback_args = has_runtime_callback_args_in_args(
                &method.args,
                callback_interfaces,
                records,
                enums,
            );
            let supported_return = method
                .return_type
                .as_ref()
                .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
                .unwrap_or(true);
            let supports_runtime_args = method
                .args
                .iter()
                .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums));
            if !has_callback_args && (!supported_return || !supports_runtime_args) {
                continue;
            }
            let method_name = safe_dart_identifier(&api_overrides.renamed_or_default(
                &ApiOverrides::object_member_key(&object.name, &method.name),
                || to_lower_camel(&method.name),
            ));
            let method_camel = to_upper_camel(&method.name);
            let invoke_name = format!("{}Invoke{}", object_lower, method_camel);
            let return_type = method
                .return_type
                .as_ref()
                .map(map_uniffi_type_to_dart)
                .unwrap_or_else(|| "void".to_string());
            let signature_return = if method.is_async {
                format!("Future<{return_type}>")
            } else {
                return_type.clone()
            };
            let args = render_callable_args_signature(&method.args, enums);
            let arg_names = render_callable_arg_names(&method.args);
            out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
            out.push_str(&format!("  {signature_return} {method_name}({args}) {{\n"));
            out.push_str("    _ensureOpen();\n");
            let invoke_args = if arg_names.is_empty() {
                "_handle".to_string()
            } else {
                format!("_handle, {arg_names}")
            };
            if method.is_async {
                if is_runtime_async_rust_future_compatible_method(
                    method,
                    callback_interfaces,
                    records,
                    enums,
                ) {
                    out.push_str(&format!("    return _ffi.{invoke_name}({invoke_args});\n"));
                } else {
                    out.push_str(&format!(
                        "    return Future(() => _ffi.{invoke_name}({invoke_args}));\n"
                    ));
                }
            } else if method.return_type.is_some() {
                out.push_str(&format!("    return _ffi.{invoke_name}({invoke_args});\n"));
            } else {
                out.push_str(&format!("    _ffi.{invoke_name}({invoke_args});\n"));
            }
            out.push_str("  }\n\n");
        }

        if let Some(display_method) = object
            .trait_methods
            .display
            .as_deref()
            .or(object.trait_methods.debug.as_deref())
        {
            let invoke_name = format!("{}Invoke{}", object_lower, to_upper_camel(display_method));
            out.push_str("  @override\n");
            out.push_str("  String toString() {\n");
            out.push_str("    if (_closed) {\n");
            out.push_str(&format!("      return '{object_name}(closed)';\n"));
            out.push_str("    }\n");
            out.push_str(&format!("    return _ffi.{invoke_name}(_handle);\n"));
            out.push_str("  }\n\n");
        }

        if let Some(hash_method) = object.trait_methods.hash.as_deref() {
            let invoke_name = format!("{}Invoke{}", object_lower, to_upper_camel(hash_method));
            out.push_str("  @override\n");
            out.push_str("  int get hashCode {\n");
            out.push_str("    _ensureOpen();\n");
            out.push_str(&format!("    return _ffi.{invoke_name}(_handle);\n"));
            out.push_str("  }\n\n");
        }

        if let Some(eq_method) = object.trait_methods.eq.as_deref() {
            let invoke_name = format!("{}Invoke{}", object_lower, to_upper_camel(eq_method));
            out.push_str("  @override\n");
            out.push_str("  bool operator ==(Object other) {\n");
            out.push_str("    if (identical(this, other)) {\n");
            out.push_str("      return true;\n");
            out.push_str("    }\n");
            out.push_str(&format!("    if (other is! {object_name}) {{\n"));
            out.push_str("      return false;\n");
            out.push_str("    }\n");
            out.push_str("    if (_closed || other._closed) {\n");
            out.push_str("      return false;\n");
            out.push_str("    }\n");
            out.push_str(&format!(
                "    return _ffi.{invoke_name}(_handle, other._handle);\n"
            ));
            out.push_str("  }\n\n");
        }

        if let Some(ord_cmp_method) = object.trait_methods.ord_cmp.as_deref() {
            let invoke_name = format!("{}Invoke{}", object_lower, to_upper_camel(ord_cmp_method));
            out.push_str("  @override\n");
            out.push_str(&format!("  int compareTo({object_name} other) {{\n"));
            out.push_str("    _ensureOpen();\n");
            out.push_str("    other._ensureOpen();\n");
            out.push_str(&format!(
                "    return _ffi.{invoke_name}(_handle, other._handle);\n"
            ));
            out.push_str("  }\n\n");
        }
        out.push_str("}\n\n");
        let object_type_name = to_upper_camel(&object.name);
        let codec_name = format!("{object_type_name}FfiCodec");
        out.push_str(&format!("final class {codec_name} {{\n"));
        out.push_str(&format!("  const {codec_name}._();\n\n"));
        out.push_str(&format!(
            "  static int lower({object_name} value) => value._handle;\n\n"
        ));
        out.push_str(&format!(
            "  static {object_name} lift(int handle) => {object_name}._(_bindings(), handle);\n"
        ));
        out.push_str("}\n");
    }

    out
}

fn render_function_stubs(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    api_overrides: &ApiOverrides,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    if functions.is_empty()
        && objects.is_empty()
        && records.iter().all(|r| r.methods.is_empty())
        && enums.iter().all(|e| e.methods.is_empty())
    {
        return String::new();
    }

    let mut out = String::new();
    let has_runtime_functions = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums)
            || is_runtime_callback_compatible_function(f, callback_interfaces, records, enums)
            || has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums)
    }) || !objects.is_empty()
        || records.iter().any(|r| !r.methods.is_empty())
        || enums.iter().any(|e| !e.methods.is_empty());
    out.push('\n');
    if has_runtime_functions {
        out.push_str(&format!("{ffi_class_name}? _defaultBindings;\n\n"));
        out.push_str(&format!(
            "{ffi_class_name} _bindings() => _defaultBindings ??= {ffi_class_name}();\n\n"
        ));
        out.push_str(
            "void configureDefaultBindings({ffi.DynamicLibrary? dynamicLibrary, String? libraryPath}) {\n",
        );
        out.push_str(&format!(
            "  _defaultBindings = {ffi_class_name}(dynamicLibrary: dynamicLibrary, libraryPath: libraryPath);\n"
        ));
        out.push_str("}\n\n");
        out.push_str("void resetDefaultBindings() {\n");
        out.push_str("  _defaultBindings = null;\n");
        out.push_str("}\n\n");
    }
    for f in functions {
        if api_overrides.excluded(&ApiOverrides::fn_key(&f.name)) {
            continue;
        }
        let public_fn_name = safe_dart_identifier(
            &api_overrides
                .renamed_or_default(&ApiOverrides::fn_key(&f.name), || to_lower_camel(&f.name)),
        );
        let internal_fn_name = safe_dart_identifier(&to_lower_camel(&f.name));
        let value_return_type = f
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let signature_return_type = if f.is_async {
            format!("Future<{value_return_type}>")
        } else {
            value_return_type.clone()
        };
        let args = render_callable_args_signature(&f.args, enums);
        let arg_names = render_callable_arg_names(&f.args);

        out.push_str(&render_doc_comment(f.docstring.as_deref(), ""));
        out.push_str(&format!(
            "{signature_return_type} {public_fn_name}({args}) {{\n"
        ));
        if is_runtime_ffi_compatible_function(f, records, enums)
            || is_runtime_callback_compatible_function(f, callback_interfaces, records, enums)
            || has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums)
        {
            if f.is_async {
                if is_runtime_async_rust_future_compatible_function(
                    f,
                    callback_interfaces,
                    records,
                    enums,
                ) {
                    out.push_str(&format!(
                        "  return _bindings().{internal_fn_name}({arg_names});\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "  return Future(() => _bindings().{internal_fn_name}({arg_names}));\n"
                    ));
                }
            } else if f.return_type.is_some() {
                out.push_str(&format!(
                    "  return _bindings().{internal_fn_name}({arg_names});\n"
                ));
            } else {
                out.push_str(&format!("  _bindings().{internal_fn_name}({arg_names});\n"));
            }
        } else if f.is_async {
            out.push_str("  return Future.error(UnimplementedError('TODO: bind to Rust FFI'));\n");
        } else {
            out.push_str("  throw UnimplementedError('TODO: bind to Rust FFI');\n");
        }
        out.push_str("}\n\n");
    }
    out
}

fn has_runtime_callback_support(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions
        .iter()
        .any(|f| has_runtime_callback_args_in_args(&f.args, callback_interfaces, records, enums))
        || objects.iter().any(|o| {
            o.methods.iter().any(|m| {
                has_runtime_callback_args_in_args(&m.args, callback_interfaces, records, enums)
            })
        })
        || records.iter().any(|r| {
            r.methods.iter().any(|m| {
                has_runtime_callback_args_in_args(&m.args, callback_interfaces, records, enums)
            })
        })
        || enums.iter().any(|e| {
            e.methods.iter().any(|m| {
                has_runtime_callback_args_in_args(&m.args, callback_interfaces, records, enums)
            })
        })
}

fn is_runtime_callback_compatible_function(
    function: &UdlFunction,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    if function.is_async || function.throws_type.is_some() {
        return false;
    }
    let return_supported = function
        .return_type
        .as_ref()
        .map(is_runtime_callback_function_return_compatible_type)
        .unwrap_or(true);
    if !return_supported {
        return false;
    }
    has_runtime_callback_args_in_args(&function.args, callback_interfaces, records, enums)
}

fn callback_interfaces_used_for_runtime<'a>(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &'a [UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Vec<&'a UdlCallbackInterface> {
    callback_interfaces
        .iter()
        .filter(|callback_interface| {
            is_runtime_callback_interface_compatible(callback_interface, records, enums)
                && (functions.iter().any(|function| {
                    has_runtime_callback_args_in_args(
                        &function.args,
                        callback_interfaces,
                        records,
                        enums,
                    ) && function.args.iter().any(|arg| {
                        callback_interface_name_from_type(&arg.type_)
                            .is_some_and(|name| name == callback_interface.name)
                    })
                }) || objects.iter().any(|object| {
                    object.methods.iter().any(|method| {
                        has_runtime_callback_args_in_args(
                            &method.args,
                            callback_interfaces,
                            records,
                            enums,
                        ) && method.args.iter().any(|arg| {
                            callback_interface_name_from_type(&arg.type_)
                                .is_some_and(|name| name == callback_interface.name)
                        })
                    })
                }))
        })
        .collect()
}

fn runtime_args_compatible_with_optional_callbacks(
    args: &[UdlArg],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<bool> {
    let mut saw_callback = false;
    for arg in args {
        if let Some(callback_name) = callback_interface_name_from_type(&arg.type_) {
            saw_callback = true;
            let callback_interface = callback_interfaces
                .iter()
                .find(|cb| cb.name == callback_name)?;
            if !is_runtime_callback_interface_compatible(callback_interface, records, enums) {
                return None;
            }
            continue;
        }
        if !is_runtime_ffi_compatible_type(&arg.type_, records, enums) {
            return None;
        }
    }
    Some(saw_callback)
}

fn has_runtime_callback_args_in_args(
    args: &[UdlArg],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    runtime_args_compatible_with_optional_callbacks(args, callback_interfaces, records, enums)
        .unwrap_or(false)
}

fn callback_interface_name_from_type(type_: &Type) -> Option<&str> {
    match type_ {
        Type::CallbackInterface { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn is_runtime_callback_interface_compatible(
    callback_interface: &UdlCallbackInterface,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    callback_interface
        .methods
        .iter()
        .all(|method| is_runtime_callback_method_compatible(method, records, enums))
}

fn is_runtime_callback_method_compatible(
    method: &UdlCallbackMethod,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    method
        .throws_type
        .as_ref()
        .map(|t| {
            is_runtime_ffi_compatible_type(t, records, enums)
                && is_runtime_throws_enum_type(t, enums)
        })
        .unwrap_or(true)
        && method
            .return_type
            .as_ref()
            .map(|t| {
                if method.is_async {
                    is_runtime_callback_async_return_type_compatible(t, records, enums)
                } else {
                    is_runtime_callback_method_type_compatible(t, records, enums)
                }
            })
            .unwrap_or(true)
        && method
            .args
            .iter()
            .all(|arg| is_runtime_callback_method_type_compatible(&arg.type_, records, enums))
}

fn is_runtime_callback_function_return_compatible_type(type_: &Type) -> bool {
    let type_ = runtime_unwrapped_type(type_);
    matches!(
        type_,
        Type::UInt8
            | Type::Int8
            | Type::UInt16
            | Type::Int16
            | Type::UInt32
            | Type::Int32
            | Type::UInt64
            | Type::Int64
            | Type::Float32
            | Type::Float64
            | Type::Boolean
            | Type::Timestamp
            | Type::Duration
    )
}

fn is_runtime_callback_method_type_compatible(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    is_runtime_callback_function_return_compatible_type(type_)
        || is_runtime_string_type(type_)
        || is_runtime_optional_string_type(type_)
        || records
            .iter()
            .any(|r| record_name_from_type(type_) == Some(r.name.as_str()))
        || is_runtime_enum_type(type_, enums)
}

fn is_runtime_callback_async_return_type_compatible(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    is_runtime_callback_method_type_compatible(type_, records, enums)
}

fn callback_bridge_class_name(callback_name: &str) -> String {
    format!("_{}CallbackBridge", to_upper_camel(callback_name))
}

fn callback_vtable_struct_name(callback_name: &str) -> String {
    format!("_{}VTable", to_upper_camel(callback_name))
}

fn callback_init_symbol(callback_name: &str) -> String {
    format!("{}_callback_init", callback_name.to_ascii_lowercase())
}

fn callback_init_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!("_{}CallbackInit", to_lower_camel(callback_name)))
}

fn callback_init_done_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!(
        "_{}CallbackInitDone",
        to_lower_camel(callback_name)
    ))
}

fn callback_vtable_field_name(callback_name: &str) -> String {
    safe_dart_identifier(&format!("_{}CallbackVTable", to_lower_camel(callback_name)))
}

fn callback_async_result_struct_name(callback_name: &str, method_name: &str) -> String {
    format!(
        "_{}{}AsyncResult",
        to_upper_camel(callback_name),
        to_upper_camel(method_name)
    )
}

fn render_callback_async_result_return_field(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<String> {
    let type_ = runtime_unwrapped_type(type_);
    match type_ {
        Type::UInt8 => Some("  @ffi.Uint8()\n  external int returnValue;\n\n".to_string()),
        Type::Int8 => Some("  @ffi.Int8()\n  external int returnValue;\n\n".to_string()),
        Type::UInt16 => Some("  @ffi.Uint16()\n  external int returnValue;\n\n".to_string()),
        Type::Int16 => Some("  @ffi.Int16()\n  external int returnValue;\n\n".to_string()),
        Type::UInt32 => Some("  @ffi.Uint32()\n  external int returnValue;\n\n".to_string()),
        Type::Int32 => Some("  @ffi.Int32()\n  external int returnValue;\n\n".to_string()),
        Type::UInt64 => Some("  @ffi.Uint64()\n  external int returnValue;\n\n".to_string()),
        Type::Int64 => Some("  @ffi.Int64()\n  external int returnValue;\n\n".to_string()),
        Type::Float32 => Some("  @ffi.Float()\n  external double returnValue;\n\n".to_string()),
        Type::Float64 => Some("  @ffi.Double()\n  external double returnValue;\n\n".to_string()),
        Type::Boolean => Some("  @ffi.Uint8()\n  external int returnValue;\n\n".to_string()),
        Type::Timestamp | Type::Duration => {
            Some("  @ffi.Int64()\n  external int returnValue;\n\n".to_string())
        }
        Type::String => Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string()),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        Type::Record { .. }
            if records
                .iter()
                .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => {
            Some("  external ffi.Pointer<Utf8> returnValue;\n\n".to_string())
        }
        _ => None,
    }
}

fn callback_async_default_return_expr(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> &'static str {
    let type_ = runtime_unwrapped_type(type_);
    match type_ {
        Type::Float32 | Type::Float64 => "0.0",
        Type::String => "ffi.nullptr",
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => "ffi.nullptr",
        Type::Record { .. }
            if records
                .iter()
                .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            "ffi.nullptr"
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => "ffi.nullptr",
        _ => "0",
    }
}

fn render_callback_arg_decode_expr(
    type_: &Type,
    arg_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let type_ = runtime_unwrapped_type(type_);
    match type_ {
        Type::String => format!(
            "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null string callback arg')) : {arg_name}.toDartString()"
        ),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            format!("{arg_name} == ffi.nullptr ? null : {arg_name}.toDartString()")
        }
        Type::Record { .. } if records
            .iter()
            .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            let record_name = record_name_from_type(type_).unwrap_or("Record");
            format!(
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null record callback arg')) : {}.fromJson(jsonDecode({arg_name}.toDartString()) as Map<String, dynamic>)",
                to_upper_camel(record_name)
            )
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => {
            let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
            format!(
                "{arg_name} == ffi.nullptr ? (throw StateError('Rust passed null enum callback arg')) : {}FfiCodec.decode({arg_name}.toDartString())",
                to_upper_camel(enum_name)
            )
        }
        Type::Timestamp => {
            format!("DateTime.fromMicrosecondsSinceEpoch({arg_name}, isUtc: true)")
        }
        Type::Duration => format!("Duration(microseconds: {arg_name})"),
        _ => arg_name.to_string(),
    }
}

fn render_callback_return_encode_expr(
    type_: &Type,
    value_expr: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let type_ = runtime_unwrapped_type(type_);
    match type_ {
        Type::String => format!("{value_expr}.toNativeUtf8()"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            format!("{value_expr} == null ? ffi.nullptr : {value_expr}.toNativeUtf8()")
        }
        Type::Record { .. }
            if records
                .iter()
                .any(|r| record_name_from_type(type_) == Some(r.name.as_str())) =>
        {
            format!("jsonEncode({value_expr}.toJson()).toNativeUtf8()")
        }
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => {
            let enum_name = enum_name_from_type(type_).unwrap_or("Enum");
            format!(
                "{}FfiCodec.encode({value_expr}).toNativeUtf8()",
                to_upper_camel(enum_name)
            )
        }
        Type::Timestamp => format!("{value_expr}.toUtc().microsecondsSinceEpoch"),
        Type::Duration => format!("{value_expr}.inMicroseconds"),
        Type::Boolean => format!("{value_expr} ? 1 : 0"),
        _ => value_expr.to_string(),
    }
}

fn has_runtime_unsupported_async_ffibuffer_support(
    functions: &[UdlFunction],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions
        .iter()
        .any(is_runtime_unsupported_async_ffibuffer_eligible_function)
        || records.iter().any(|r| {
            r.methods
                .iter()
                .any(is_runtime_unsupported_async_ffibuffer_eligible_method)
        })
        || enums.iter().any(|e| {
            e.methods
                .iter()
                .any(is_runtime_unsupported_async_ffibuffer_eligible_method)
        })
}

fn has_runtime_async_rust_future_support(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    functions.iter().any(|f| {
        f.runtime_unsupported.is_none()
            && is_runtime_async_rust_future_compatible_function(
                f,
                callback_interfaces,
                records,
                enums,
            )
    }) || objects.iter().any(|o| {
        o.methods.iter().any(|m| {
            m.runtime_unsupported.is_none()
                && is_runtime_async_rust_future_compatible_method(
                    m,
                    callback_interfaces,
                    records,
                    enums,
                )
        })
    }) || records.iter().any(|r| {
        r.methods.iter().any(|m| {
            m.runtime_unsupported.is_none()
                && is_runtime_async_rust_future_compatible_method(
                    m,
                    callback_interfaces,
                    records,
                    enums,
                )
        })
    }) || enums.iter().any(|e| {
        e.methods.iter().any(|m| {
            m.runtime_unsupported.is_none()
                && is_runtime_async_rust_future_compatible_method(
                    m,
                    callback_interfaces,
                    records,
                    enums,
                )
        })
    })
}

struct AsyncRustFutureSpec {
    suffix: &'static str,
    complete_native_type: &'static str,
    complete_dart_type: &'static str,
}

fn async_rust_future_spec_from_uniffi_return_type(
    return_type: Option<&Type>,
) -> Option<AsyncRustFutureSpec> {
    let return_ffi_type = return_type.and_then(ffibuffer_ffi_type_from_uniffi_type);
    match return_ffi_type {
        None => Some(AsyncRustFutureSpec {
            suffix: "void",
            complete_native_type: "ffi.Void",
            complete_dart_type: "void",
        }),
        Some(FfiType::UInt8) => Some(AsyncRustFutureSpec {
            suffix: "u8",
            complete_native_type: "ffi.Uint8",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int8) => Some(AsyncRustFutureSpec {
            suffix: "i8",
            complete_native_type: "ffi.Int8",
            complete_dart_type: "int",
        }),
        Some(FfiType::UInt16) => Some(AsyncRustFutureSpec {
            suffix: "u16",
            complete_native_type: "ffi.Uint16",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int16) => Some(AsyncRustFutureSpec {
            suffix: "i16",
            complete_native_type: "ffi.Int16",
            complete_dart_type: "int",
        }),
        Some(FfiType::UInt32) => Some(AsyncRustFutureSpec {
            suffix: "u32",
            complete_native_type: "ffi.Uint32",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int32) => Some(AsyncRustFutureSpec {
            suffix: "i32",
            complete_native_type: "ffi.Int32",
            complete_dart_type: "int",
        }),
        Some(FfiType::UInt64) | Some(FfiType::Handle) => Some(AsyncRustFutureSpec {
            suffix: "u64",
            complete_native_type: "ffi.Uint64",
            complete_dart_type: "int",
        }),
        Some(FfiType::Int64) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        Some(FfiType::Float32) => Some(AsyncRustFutureSpec {
            suffix: "f32",
            complete_native_type: "ffi.Float",
            complete_dart_type: "double",
        }),
        Some(FfiType::Float64) => Some(AsyncRustFutureSpec {
            suffix: "f64",
            complete_native_type: "ffi.Double",
            complete_dart_type: "double",
        }),
        Some(FfiType::RustBuffer(_)) => Some(AsyncRustFutureSpec {
            suffix: "rust_buffer",
            complete_native_type: "_UniFfiRustBuffer",
            complete_dart_type: "_UniFfiRustBuffer",
        }),
        _ => None,
    }
}

fn async_rust_future_spec(
    return_type: Option<&Type>,
    _records: &[UdlRecord],
    enums: &[UdlEnum],
) -> Option<AsyncRustFutureSpec> {
    match return_type.map(runtime_unwrapped_type) {
        None => Some(AsyncRustFutureSpec {
            suffix: "void",
            complete_native_type: "ffi.Void",
            complete_dart_type: "void",
        }),
        Some(type_) if is_runtime_string_type(type_) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(type_) if is_runtime_optional_string_type(type_) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(type_) if is_runtime_record_type(type_) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(type_) if is_runtime_enum_type(type_, enums) => Some(AsyncRustFutureSpec {
            suffix: "string",
            complete_native_type: "ffi.Pointer<Utf8>",
            complete_dart_type: "ffi.Pointer<Utf8>",
        }),
        Some(Type::Map { key_type, .. }) if is_runtime_string_type(key_type) => {
            Some(AsyncRustFutureSpec {
                suffix: "string",
                complete_native_type: "ffi.Pointer<Utf8>",
                complete_dart_type: "ffi.Pointer<Utf8>",
            })
        }
        Some(Type::Bytes) => Some(AsyncRustFutureSpec {
            suffix: "bytes",
            complete_native_type: "_RustBuffer",
            complete_dart_type: "_RustBuffer",
        }),
        Some(Type::Optional { inner_type }) if is_runtime_bytes_type(inner_type) => {
            Some(AsyncRustFutureSpec {
                suffix: "bytes_opt",
                complete_native_type: "_RustBufferOpt",
                complete_dart_type: "_RustBufferOpt",
            })
        }
        Some(Type::Sequence { inner_type }) if is_runtime_bytes_type(inner_type) => {
            Some(AsyncRustFutureSpec {
                suffix: "bytes_vec",
                complete_native_type: "_RustBufferVec",
                complete_dart_type: "_RustBufferVec",
            })
        }
        Some(Type::Object { .. }) => Some(AsyncRustFutureSpec {
            suffix: "u64",
            complete_native_type: "ffi.Uint64",
            complete_dart_type: "int",
        }),
        Some(Type::UInt8) => Some(AsyncRustFutureSpec {
            suffix: "u8",
            complete_native_type: "ffi.Uint8",
            complete_dart_type: "int",
        }),
        Some(Type::Int8) => Some(AsyncRustFutureSpec {
            suffix: "i8",
            complete_native_type: "ffi.Int8",
            complete_dart_type: "int",
        }),
        Some(Type::UInt16) => Some(AsyncRustFutureSpec {
            suffix: "u16",
            complete_native_type: "ffi.Uint16",
            complete_dart_type: "int",
        }),
        Some(Type::Int16) => Some(AsyncRustFutureSpec {
            suffix: "i16",
            complete_native_type: "ffi.Int16",
            complete_dart_type: "int",
        }),
        Some(Type::UInt32) => Some(AsyncRustFutureSpec {
            suffix: "u32",
            complete_native_type: "ffi.Uint32",
            complete_dart_type: "int",
        }),
        Some(Type::Int32) => Some(AsyncRustFutureSpec {
            suffix: "i32",
            complete_native_type: "ffi.Int32",
            complete_dart_type: "int",
        }),
        Some(Type::UInt64) => Some(AsyncRustFutureSpec {
            suffix: "u64",
            complete_native_type: "ffi.Uint64",
            complete_dart_type: "int",
        }),
        Some(Type::Int64) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        Some(Type::Float32) => Some(AsyncRustFutureSpec {
            suffix: "f32",
            complete_native_type: "ffi.Float",
            complete_dart_type: "double",
        }),
        Some(Type::Float64) => Some(AsyncRustFutureSpec {
            suffix: "f64",
            complete_native_type: "ffi.Double",
            complete_dart_type: "double",
        }),
        Some(Type::Timestamp) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        Some(Type::Duration) => Some(AsyncRustFutureSpec {
            suffix: "i64",
            complete_native_type: "ffi.Int64",
            complete_dart_type: "int",
        }),
        _ => None,
    }
}

fn is_runtime_async_rust_future_compatible_function(
    function: &UdlFunction,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    function.is_async
        && function.throws_type.is_none()
        && function
            .return_type
            .as_ref()
            .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
            .unwrap_or(true)
        && runtime_args_compatible_with_optional_callbacks(
            &function.args,
            callback_interfaces,
            records,
            enums,
        )
        .is_some()
        && async_rust_future_spec(function.return_type.as_ref(), records, enums).is_some()
}

fn is_runtime_async_rust_future_compatible_method(
    method: &UdlObjectMethod,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    method.is_async
        && method.throws_type.is_none()
        && async_rust_future_spec(method.return_type.as_ref(), records, enums).is_some()
        && runtime_args_compatible_with_optional_callbacks(
            &method.args,
            callback_interfaces,
            records,
            enums,
        )
        .is_some()
}

fn is_runtime_ffi_compatible_function(
    function: &UdlFunction,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    function
        .return_type
        .as_ref()
        .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
        .unwrap_or(true)
        && function
            .args
            .iter()
            .all(|arg| is_runtime_ffi_compatible_type(&arg.type_, records, enums))
        && function
            .throws_type
            .as_ref()
            .map(|t| {
                is_runtime_ffi_compatible_type(t, records, enums)
                    && is_runtime_throws_enum_type(t, enums)
            })
            .unwrap_or(true)
}

fn is_runtime_throwing_ffi_compatible_function(
    function: &UdlFunction,
    callback_interfaces: &[UdlCallbackInterface],
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    function
        .throws_type
        .as_ref()
        .map(|t| {
            is_runtime_ffi_compatible_type(t, records, enums)
                && is_runtime_throws_enum_type(t, enums)
        })
        .unwrap_or(false)
        && function
            .return_type
            .as_ref()
            .map(|t| is_runtime_ffi_compatible_type(t, records, enums))
            .unwrap_or(true)
        && runtime_args_compatible_with_optional_callbacks(
            &function.args,
            callback_interfaces,
            records,
            enums,
        )
        .is_some()
}

fn is_runtime_ffi_compatible_type(type_: &Type, records: &[UdlRecord], enums: &[UdlEnum]) -> bool {
    map_runtime_native_ffi_type(type_, records, enums).is_some()
}

fn map_runtime_native_ffi_type(
    type_: &Type,
    _records: &[UdlRecord],
    _enums: &[UdlEnum],
) -> Option<&'static str> {
    if let Type::Custom { builtin, .. } = type_ {
        return map_runtime_native_ffi_type(builtin, _records, _enums);
    }

    match type_ {
        Type::UInt8 => Some("ffi.Uint8"),
        Type::Int8 => Some("ffi.Int8"),
        Type::UInt16 => Some("ffi.Uint16"),
        Type::Int16 => Some("ffi.Int16"),
        Type::UInt32 => Some("ffi.Uint32"),
        Type::Int32 => Some("ffi.Int32"),
        Type::UInt64 => Some("ffi.Uint64"),
        Type::Int64 => Some("ffi.Int64"),
        Type::Float32 => Some("ffi.Float"),
        Type::Float64 => Some("ffi.Double"),
        Type::Boolean => Some("ffi.Bool"),
        Type::String => Some("ffi.Pointer<Utf8>"),
        Type::Timestamp => Some("ffi.Int64"),
        Type::Duration => Some("ffi.Int64"),
        Type::Bytes => Some("_RustBuffer"),
        Type::Optional { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferOpt")
        }
        Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferVec")
        }
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => Some("ffi.Pointer<Utf8>"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Record { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Enum { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Object { .. } => Some("ffi.Uint64"),
        _ => None,
    }
}

fn map_runtime_dart_ffi_type(
    type_: &Type,
    _records: &[UdlRecord],
    _enums: &[UdlEnum],
) -> Option<&'static str> {
    if let Type::Custom { builtin, .. } = type_ {
        return map_runtime_dart_ffi_type(builtin, _records, _enums);
    }

    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64 => Some("int"),
        Type::Float32 | Type::Float64 => Some("double"),
        Type::Boolean => Some("bool"),
        Type::String => Some("ffi.Pointer<Utf8>"),
        Type::Timestamp | Type::Duration => Some("int"),
        Type::Bytes => Some("_RustBuffer"),
        Type::Optional { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferOpt")
        }
        Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type) => {
            Some("_RustBufferVec")
        }
        Type::Map { key_type, .. } if is_runtime_string_type(key_type) => Some("ffi.Pointer<Utf8>"),
        Type::Optional { inner_type } if is_runtime_string_type(inner_type) => {
            Some("ffi.Pointer<Utf8>")
        }
        Type::Record { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Enum { .. } => Some("ffi.Pointer<Utf8>"),
        Type::Object { .. } => Some("int"),
        _ => None,
    }
}

fn is_runtime_string_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::String)
}

fn is_runtime_timestamp_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Timestamp)
}

fn is_runtime_duration_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Duration)
}

fn is_runtime_bytes_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Bytes)
}

fn is_runtime_record_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Record { .. })
}

fn is_runtime_enum_type(type_: &Type, _enums: &[UdlEnum]) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Enum { .. })
}

fn is_runtime_object_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Object { .. })
}

fn is_runtime_error_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    let Some(name) = enum_name_from_type(type_) else {
        return false;
    };
    enums.iter().any(|e| e.name == name && e.is_error)
}

fn is_runtime_throws_enum_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    if is_runtime_error_enum_type(type_, enums) {
        return true;
    }
    matches!(runtime_unwrapped_type(type_), Type::Enum { .. })
}

fn is_runtime_record_or_enum_string_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    is_runtime_record_type(type_) || is_runtime_enum_type(type_, enums)
}

fn enum_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Enum { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn record_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Record { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn object_name_from_type(type_: &Type) -> Option<&str> {
    match runtime_unwrapped_type(type_) {
        Type::Object { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn is_external_object_type(type_: &Type, local_module_path: &str) -> bool {
    let local_crate = local_module_path.split("::").next().unwrap_or_default();
    match runtime_unwrapped_type(type_) {
        Type::Object { module_path, .. } => {
            let crate_name = module_path.split("::").next().unwrap_or_default();
            !crate_name.is_empty() && !local_crate.is_empty() && crate_name != local_crate
        }
        _ => false,
    }
}

fn render_object_lift_expr(
    type_: &Type,
    handle_expr: &str,
    local_module_path: &str,
    binding_expr: &str,
) -> String {
    let object_name = to_upper_camel(object_name_from_type(type_).unwrap_or("Object"));
    if is_external_object_type(type_, local_module_path) {
        format!("{object_name}FfiCodec.lift({handle_expr})")
    } else {
        format!("{object_name}._({binding_expr}, {handle_expr})")
    }
}

fn is_runtime_optional_bytes_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if is_runtime_bytes_type(inner_type))
}

fn is_runtime_sequence_bytes_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Sequence { inner_type } if is_runtime_bytes_type(inner_type))
}

fn is_runtime_bytes_like_type(type_: &Type) -> bool {
    is_runtime_bytes_type(type_)
        || is_runtime_optional_bytes_type(type_)
        || is_runtime_sequence_bytes_type(type_)
}

fn is_runtime_optional_string_type(type_: &Type) -> bool {
    matches!(runtime_unwrapped_type(type_), Type::Optional { inner_type } if is_runtime_string_type(inner_type))
}

fn is_runtime_string_like_type(type_: &Type) -> bool {
    is_runtime_string_type(type_) || is_runtime_optional_string_type(type_)
}

fn render_plain_ffi_decode_expr(type_: &Type, call_expr: &str) -> String {
    match runtime_unwrapped_type(type_) {
        Type::Timestamp => format!("DateTime.fromMicrosecondsSinceEpoch({call_expr}, isUtc: true)"),
        Type::Duration => format!("Duration(microseconds: {call_expr})"),
        _ => call_expr.to_string(),
    }
}

fn map_uniffi_type_to_dart(type_: &Type) -> String {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64 => "int".to_string(),
        Type::Float32 | Type::Float64 => "double".to_string(),
        Type::Boolean => "bool".to_string(),
        Type::String => "String".to_string(),
        Type::Bytes => "Uint8List".to_string(),
        Type::Timestamp => "DateTime".to_string(),
        Type::Duration => "Duration".to_string(),
        Type::Optional { inner_type } => format!("{}?", map_uniffi_type_to_dart(inner_type)),
        Type::Sequence { inner_type } => format!("List<{}>", map_uniffi_type_to_dart(inner_type)),
        Type::Map {
            key_type,
            value_type,
        } => format!(
            "Map<{}, {}>",
            map_uniffi_type_to_dart(key_type),
            map_uniffi_type_to_dart(value_type)
        ),
        Type::Enum { name, .. }
        | Type::Object { name, .. }
        | Type::Record { name, .. }
        | Type::CallbackInterface { name, .. } => to_upper_camel(name),
        Type::Custom { builtin, .. } => map_uniffi_type_to_dart(builtin),
    }
}

fn uniffi_type_uses_json(type_: &Type) -> bool {
    match type_ {
        Type::Record { .. } | Type::Enum { .. } | Type::Map { .. } => true,
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            uniffi_type_uses_json(inner_type)
        }
        Type::Custom { builtin, .. } => uniffi_type_uses_json(builtin),
        _ => false,
    }
}

fn uniffi_type_uses_bytes(type_: &Type) -> bool {
    match type_ {
        Type::Bytes => true,
        Type::Optional { inner_type } | Type::Sequence { inner_type } => {
            uniffi_type_uses_bytes(inner_type)
        }
        Type::Map {
            key_type,
            value_type,
        } => uniffi_type_uses_bytes(key_type) || uniffi_type_uses_bytes(value_type),
        Type::Custom { builtin, .. } => uniffi_type_uses_bytes(builtin),
        _ => false,
    }
}

fn runtime_unwrapped_type(type_: &Type) -> &Type {
    match type_ {
        Type::Custom { builtin, .. } => runtime_unwrapped_type(builtin),
        _ => type_,
    }
}

fn is_runtime_map_with_string_key_type(type_: &Type) -> bool {
    match runtime_unwrapped_type(type_) {
        Type::Map { key_type, .. } => is_runtime_string_type(key_type),
        _ => false,
    }
}

fn is_runtime_utf8_pointer_marshaled_type(
    type_: &Type,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> bool {
    map_runtime_native_ffi_type(type_, records, enums) == Some("ffi.Pointer<Utf8>")
}

fn to_upper_camel(input: &str) -> String {
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

fn dart_identifier(input: &str) -> String {
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

fn to_lower_camel(input: &str) -> String {
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

fn safe_dart_identifier(input: &str) -> String {
    if is_dart_keyword(input) {
        format!("{input}_")
    } else {
        input.to_string()
    }
}

fn uniffi_trait_method_kind(name: &str) -> Option<&'static str> {
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

fn is_uniffi_trait_method_name(name: &str) -> bool {
    uniffi_trait_method_kind(name).is_some()
}

fn is_dart_keyword(input: &str) -> bool {
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
    use std::collections::HashMap;
    use std::fs;

    use super::*;

    #[test]
    fn generates_dart_file_with_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("simple-fns.udl");
        let out_dir = temp.path().join("out");
        fs::write(&source, "namespace simple_fns {};").expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");

        let generated = out_dir.join("simple_fns.dart");
        assert!(generated.exists());
        let content = fs::read_to_string(generated).expect("read generated file");
        assert!(content.contains("library simple_fns;"));
        assert!(content.contains("class SimpleFnsFfi {"));
        assert!(content.contains("libraryName = 'uniffi_simple_fns';"));
    }

    #[test]
    fn library_mode_uses_library_metadata_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let out_dir = temp.path().join("out");
        let source = temp.path().join("libmissing.dylib");
        fs::write(&source, b"not-a-real-library").expect("write source");

        let args = GenerateArgs {
            source,
            out_dir,
            library: true,
            config: None,
            crate_name: None,
            no_format: false,
        };

        let err = generate_bindings(&args).expect_err("library mode should attempt metadata parse");
        let msg = format!("{err:#}");
        assert!(msg.contains("failed to parse library metadata"));
    }

    #[test]
    fn uses_udl_namespace_when_available() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("filename_does_not_match.udl");
        let out_dir = temp.path().join("out");
        fs::write(&source, "namespace from_udl { u32 add(u32 a, u32 b); };").expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("from_udl.dart")).expect("read generated");
        assert!(content.contains("library from_udl;"));
    }

    #[test]
    fn applies_config_overrides() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("demo.udl");
        let out_dir = temp.path().join("out");
        let config = temp.path().join("uniffi.toml");
        fs::write(&source, "namespace demo {};").expect("write source");
        fs::write(
            &config,
            r#"
[bindings.dart]
module_name = "demo_bindings"
ffi_class_name = "DemoInterop"
library_name = "demoffi"
"#,
        )
        .expect("write config");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: Some(config),
            crate_name: Some("crate_override".to_string()),
            no_format: false,
        };

        generate_bindings(&args).expect("generate");

        let content = fs::read_to_string(out_dir.join("demo.dart")).expect("read generated file");
        assert!(content.contains("library demo_bindings;"));
        assert!(content.contains("class DemoInterop {"));
        assert!(content.contains("libraryName = 'demoffi';"));
    }

    #[test]
    fn applies_rename_and_exclude_overrides() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("rename_demo.udl");
        let out_dir = temp.path().join("out");
        let config = temp.path().join("uniffi.toml");
        fs::write(
            &source,
            r#"
namespace rename_demo {
  u32 add_numbers(u32 left, u32 right);
  u32 skip_top_level(u32 value);
};

interface Counter {
  constructor(u32 initial);
  [Name=with_seed]
  constructor(u32 seed);
  u32 current_value();
  u32 hidden_value();
};
"#,
        )
        .expect("write source");
        fs::write(
            &config,
            r#"
[bindings.dart]
rename = { add_numbers = "sumValues", Counter = "Meter", "Counter.current_value" = "valueNow", "Counter.with_seed" = "seeded" }
exclude = ["skip_top_level", "Counter.hidden_value"]
"#,
        )
        .expect("write config");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: Some(config),
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content =
            fs::read_to_string(out_dir.join("rename_demo.dart")).expect("read generated file");
        assert!(content.contains("int sumValues(int left, int right) {"));
        assert!(!content.contains("\nint skipTopLevel("));
        assert!(content.contains("final class Meter {"));
        assert!(content.contains("static Meter seeded(int seed) {"));
        assert!(content.contains("int valueNow() {"));
        assert!(!content.contains("\n  int hiddenValue() {"));
    }

    #[test]
    fn imports_external_package_and_binds_external_record_and_enum_types() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("ext_demo.udl");
        let out_dir = temp.path().join("out");
        let config = temp.path().join("uniffi.toml");
        fs::write(
            &source,
            r#"
[External="other_crate"]
typedef dictionary RemoteThing;
[External="other_crate"]
typedef enum RemoteState;
[External="other_crate"]
typedef enum RemoteFailure;
[External="other_crate"]
typedef interface RemoteCounter;

namespace ext_demo {
  RemoteThing echo_remote(RemoteThing input);
  RemoteState echo_remote_state(RemoteState input);
  [Throws=RemoteFailure]
  u32 risky_remote_count(i32 input);
  RemoteCounter echo_remote_counter(RemoteCounter input);
  [Async]
  RemoteCounter echo_remote_counter_async(RemoteCounter input);
};
"#,
        )
        .expect("write source");
        fs::write(
            &config,
            r#"
[bindings.dart]
external_packages = { other_crate = "package:other_bindings/other_bindings.dart" }
"#,
        )
        .expect("write config");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: Some(config),
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("ext_demo.dart")).expect("read generated");
        assert!(content.contains("import 'package:other_bindings/other_bindings.dart';"));
        assert!(content.contains("RemoteThing echoRemote(RemoteThing input) {"));
        assert!(content.contains("return _bindings().echoRemote(input);"));
        assert!(content.contains("RemoteState echoRemoteState(RemoteState input) {"));
        assert!(content.contains("RemoteStateFfiCodec.encode(input)"));
        assert!(content.contains("RemoteStateFfiCodec.decode(payload)"));
        assert!(content.contains("int riskyRemoteCount(int input) {"));
        assert!(content.contains("throw RemoteFailureExceptionFfiCodec.decode(errRaw);"));
        assert!(content.contains("RemoteCounter echoRemoteCounter(RemoteCounter input) {"));
        assert!(content.contains("RemoteCounterFfiCodec.lower(input)"));
        assert!(content.contains("RemoteCounterFfiCodec.lift("));
        assert!(
            content.contains("Future<RemoteCounter> echoRemoteCounterAsync(RemoteCounter input) {")
        );
        assert!(content.contains("rust_future_complete_u64"));
        assert!(!content.contains("TODO: bind to Rust FFI"));
    }

    #[test]
    fn renders_docstrings_for_public_api_surfaces() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("docstrings_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
/// Namespace docs.
namespace docstrings_demo {
  /// Adds two values.
  u32 add_values(u32 left, u32 right);
};

/// 2D point value.
dictionary Point {
  /// Horizontal position.
  i32 x;
  /// Vertical position.
  i32 y;
};

/// Mood state.
enum Mood {
  /// Positive.
  "happy",
  /// Negative.
  "sad",
};

/// Callback reporter.
callback interface Reporter {
  /// Reports a message.
  void report(string message);
};

/// Counter docs.
interface Counter {
  /// Creates a counter.
  constructor();
  /// Returns current value.
  u32 current_value();
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content =
            fs::read_to_string(out_dir.join("docstrings_demo.dart")).expect("read generated");
        assert!(content.contains(
            "// ignore_for_file: unused_element\n/// Namespace docs.\nlibrary docstrings_demo;"
        ));
        assert!(content.contains("/// Adds two values.\nint addValues(int left, int right) {"));
        assert!(content.contains("/// 2D point value.\nclass Point {"));
        assert!(
            content.contains("/// Horizontal position.\n    required this.x,")
                || content.contains("/// Horizontal position.\n  final int x;")
        );
        assert!(content.contains("/// Mood state.\nenum Mood {"));
        assert!(content.contains("/// Positive.\n  happy,"));
        assert!(content.contains("/// Callback reporter.\nabstract interface class Reporter {"));
        assert!(content.contains("/// Reports a message.\n  void report(String message);"));
        assert!(content.contains("/// Counter docs.\nfinal class Counter {"));
        assert!(content.contains("/// Creates a counter.\n  static Counter create() {"));
        assert!(content.contains("/// Returns current value.\n  int currentValue() {"));
    }

    #[test]
    fn renders_top_level_function_stubs_from_udl() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace demo {
  u32 add_numbers(u32 left_value, u32 right_value);
  boolean is_even(i32 input_value);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("demo.dart")).expect("read generated file");
        assert!(content.contains("int addNumbers(int leftValue, int rightValue) {"));
        assert!(content.contains("bool isEven(int inputValue) {"));
    }

    #[test]
    fn renders_async_functions_and_methods_as_futures() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("async_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
[Custom]
typedef string Label;
[Custom]
typedef u32 Count;
[Custom]
typedef bytes Blob;

namespace async_demo {
  [Async]
  string greet_async(string name);
  [Async]
  u32 add_async(u32 left, u32 right);
  [Async]
  void tick_async();
  [Async]
  bytes echo_bytes_async(bytes input);
  [Async]
  bytes? maybe_echo_bytes_async(bytes? input);
  [Async]
  sequence<bytes> chunks_echo_bytes_async(sequence<bytes> input);
  [Async]
  record<string, u32> summarize_async(record<string, u32> values);
  [Async]
  Label label_echo_async(Label input);
  [Async]
  Count count_echo_async(Count input);
  [Async]
  Blob blob_echo_async(Blob input);
  [Async]
  Blob? blob_echo_maybe_async(Blob? input);
  [Async]
  Counter counter_new_async(u32 initial);
};

interface Counter {
  constructor(u32 initial);
  [Async]
  string async_describe();
  [Async]
  u32 async_value();
  [Async]
  bytes async_snapshot_bytes();
  [Async]
  record<string, u32> async_counts(record<string, u32> items);
  [Async]
  Label async_label_echo(Label input);
  [Async]
  Count async_count();
  [Async]
  Blob async_blob();
  [Async]
  Blob? async_blob_maybe(Blob? input);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("async_demo.dart")).expect("read generated");
        assert!(content.contains("Future<String> greetAsync(String name) {"));
        assert!(content.contains("return _bindings().greetAsync(name);"));
        assert!(content.contains("Future<int> addAsync(int left, int right) {"));
        assert!(content.contains("Future<void> tickAsync() {"));
        assert!(content.contains("Future<Uint8List> echoBytesAsync(Uint8List input) {"));
        assert!(content.contains("Future<Uint8List?> maybeEchoBytesAsync(Uint8List? input) {"));
        assert!(content
            .contains("Future<List<Uint8List>> chunksEchoBytesAsync(List<Uint8List> input) {"));
        assert!(
            content.contains("Future<Map<String, int>> summarizeAsync(Map<String, int> values) {")
        );
        assert!(content.contains("Future<String> labelEchoAsync(String input) {"));
        assert!(content.contains("Future<int> countEchoAsync(int input) {"));
        assert!(content.contains("Future<Uint8List> blobEchoAsync(Uint8List input) {"));
        assert!(content.contains("Future<Uint8List?> blobEchoMaybeAsync(Uint8List? input) {"));
        assert!(content.contains("Future<Counter> counterNewAsync(int initial) {"));
        assert!(content.contains("rust_future_poll_string"));
        assert!(content.contains("rust_future_complete_string"));
        assert!(content.contains("rust_future_poll_u32"));
        assert!(content.contains("rust_future_complete_u32"));
        assert!(content.contains("rust_future_poll_u64"));
        assert!(content.contains("rust_future_complete_u64"));
        assert!(content.contains("rust_future_poll_void"));
        assert!(content.contains("rust_future_complete_void"));
        assert!(content.contains("rust_future_poll_bytes"));
        assert!(content.contains("rust_future_complete_bytes"));
        assert!(content.contains("rust_future_poll_bytes_opt"));
        assert!(content.contains("rust_future_complete_bytes_opt"));
        assert!(content.contains("rust_future_poll_bytes_vec"));
        assert!(content.contains("rust_future_complete_bytes_vec"));
        assert!(content.contains("rust_future_free_string"));
        assert!(content.contains("final class _RustCallStatus extends ffi.Struct {"));
        assert!(content.contains("Future<String> asyncDescribe() {"));
        assert!(content.contains("return _ffi.counterInvokeAsyncDescribe(_handle);"));
        assert!(content.contains("Future<int> asyncValue() {"));
        assert!(content.contains("Future<Uint8List> asyncSnapshotBytes() {"));
        assert!(content.contains("Future<Map<String, int>> asyncCounts(Map<String, int> items) {"));
        assert!(content.contains("Future<String> asyncLabelEcho(String input) {"));
        assert!(content.contains("Future<int> asyncCount() {"));
        assert!(content.contains("Future<Uint8List> asyncBlob() {"));
        assert!(content.contains("Future<Uint8List?> asyncBlobMaybe(Uint8List? input) {"));
        assert!(content.contains("return _ffi.counterInvokeAsyncSnapshotBytes(_handle);"));
    }

    #[test]
    fn renders_runtime_callback_interface_bindings() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("callbacks.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace callbacks {
  u32 apply_adder(Adder adder, u32 left, u32 right);
  [Async]
  u32 apply_adder_async(Adder adder, u32 left, u32 right);
  [Throws=MathError]
  u32 checked_apply_adder(Adder adder, u32 left, u32 right);
  u32 apply_formatter(Formatter formatter, string? prefix, Person person, Outcome outcome);
  [Async]
  string apply_formatter_async(Formatter formatter, string? prefix, Person person, Outcome outcome);
  [Async]
  u32 apply_formatter_optional_len_async(Formatter formatter, string? prefix, Person person, Outcome outcome);
  [Async]
  u32 apply_formatter_person_len_async(Formatter formatter, string? prefix, Person person, Outcome outcome);
  [Async]
  u32 apply_formatter_outcome_len_async(Formatter formatter, string? prefix, Person person, Outcome outcome);
};

callback interface Adder {
  u32 add(u32 left, u32 right);
  [Async]
  u32 add_async(u32 left, u32 right);
  [Throws=MathError]
  u32 checked_add(u32 left, u32 right);
};

callback interface Formatter {
  string format(string? prefix, Person person, Outcome outcome);
  [Async]
  string format_async(string? prefix, Person person, Outcome outcome);
  [Async]
  string? format_async_optional(string? prefix, Person person, Outcome outcome);
  [Async]
  Person format_async_person(string? prefix, Person person, Outcome outcome);
  [Async]
  Outcome format_async_outcome(string? prefix, Person person, Outcome outcome);
};

dictionary Person {
  string name;
  u32 age;
};

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
};

[Traits=(Display, Hash, Eq, Ord)]
interface Counter {
  constructor();
  u32 apply_adder_with(Adder adder, u32 left, u32 right);
  [Async]
  u32 apply_adder_async_with(Adder adder, u32 left, u32 right);
  [Throws=MathError]
  u32 checked_apply_adder_with(Adder adder, u32 left, u32 right);
};

[Error]
interface MathError {
  DivisionByZero();
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("callbacks.dart")).expect("read generated");
        assert!(content.contains("abstract interface class Adder {"));
        assert!(content.contains("int add(int left, int right);"));
        assert!(content.contains("Future<int> addAsync(int left, int right);"));
        assert!(content.contains("int checkedAdd(int left, int right);"));
        assert!(content.contains("final class _AdderVTable extends ffi.Struct {"));
        assert!(content.contains("final class _AdderCallbackBridge {"));
        assert!(content
            .contains("final class _ForeignFutureDroppedCallbackStruct extends ffi.Struct {"));
        assert!(content.contains("final class _AdderAddAsyncAsyncResult extends ffi.Struct {"));
        assert!(content.contains("lookupFunction<ffi.Void Function(ffi.Pointer<_AdderVTable>)"));
        assert!(content.contains("'adder_callback_init'"));
        assert!(content.contains("int applyAdder(Adder adder, int left, int right) {"));
        assert!(content.contains("Future<int> applyAdderAsync(Adder adder, int left, int right) {"));
        assert!(content.contains("int checkedApplyAdder(Adder adder, int left, int right) {"));
        assert!(content.contains("return _bindings().applyAdder(adder, left, right);"));
        assert!(content.contains("return _bindings().applyAdderAsync(adder, left, right);"));
        assert!(content.contains("return _bindings().checkedApplyAdder(adder, left, right);"));
        assert!(content.contains("abstract interface class Formatter {"));
        assert!(content.contains("String format(String? prefix, Person person, Outcome outcome);"));
        assert!(content.contains(
            "Future<String> formatAsync(String? prefix, Person person, Outcome outcome);"
        ));
        assert!(content.contains(
            "Future<String?> formatAsyncOptional(String? prefix, Person person, Outcome outcome);"
        ));
        assert!(content.contains(
            "Future<Person> formatAsyncPerson(String? prefix, Person person, Outcome outcome);"
        ));
        assert!(content.contains(
            "Future<Outcome> formatAsyncOutcome(String? prefix, Person person, Outcome outcome);"
        ));
        assert!(
            content.contains("final class _FormatterFormatAsyncAsyncResult extends ffi.Struct {")
        );
        assert!(content
            .contains("final class _FormatterFormatAsyncOptionalAsyncResult extends ffi.Struct {"));
        assert!(content
            .contains("final class _FormatterFormatAsyncPersonAsyncResult extends ffi.Struct {"));
        assert!(content
            .contains("final class _FormatterFormatAsyncOutcomeAsyncResult extends ffi.Struct {"));
        assert!(content.contains("'formatter_callback_init'"));
        assert!(content.contains("final class _FormatterVTable extends ffi.Struct {"));
        assert!(content.contains("final class _FormatterCallbackBridge {"));
        assert!(content
            .contains("int applyFormatter(Formatter formatter, String? prefix, Person person, Outcome outcome) {"));
        assert!(content.contains(
            "Future<String> applyFormatterAsync(Formatter formatter, String? prefix, Person person, Outcome outcome) {"
        ));
        assert!(content.contains(
            "Future<int> applyFormatterOptionalLenAsync(Formatter formatter, String? prefix, Person person, Outcome outcome) {"
        ));
        assert!(content.contains(
            "Future<int> applyFormatterPersonLenAsync(Formatter formatter, String? prefix, Person person, Outcome outcome) {"
        ));
        assert!(content.contains(
            "Future<int> applyFormatterOutcomeLenAsync(Formatter formatter, String? prefix, Person person, Outcome outcome) {"
        ));
        assert!(content.contains(
            "int counterInvokeApplyAdderWith(int handle, Adder adder, int left, int right) {"
        ));
        assert!(content.contains("int applyAdderWith(Adder adder, int left, int right) {"));
        assert!(content.contains(
            "Future<int> counterInvokeApplyAdderAsyncWith(int handle, Adder adder, int left, int right) async {"
        ));
        assert!(content.contains(
            "int counterInvokeCheckedApplyAdderWith(int handle, Adder adder, int left, int right) {"
        ));
        assert!(
            content.contains("Future<int> applyAdderAsyncWith(Adder adder, int left, int right) {")
        );
        assert!(content.contains("int checkedApplyAdderWith(Adder adder, int left, int right) {"));
    }

    #[test]
    fn adds_typed_data_import_for_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("bytes_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace bytes_demo {
  bytes echo_bytes(bytes input);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("bytes_demo.dart")).expect("read generated");
        assert!(content.contains("import 'dart:typed_data';"));
        assert!(content.contains("import 'package:ffi/ffi.dart';"));
        assert!(content.contains("final class _RustBuffer extends ffi.Struct {"));
        assert!(content.contains("Uint8List echoBytes(Uint8List input) {"));
        assert!(content.contains("late final void Function(_RustBuffer) _rustBytesFree ="));
        assert!(content.contains("return Uint8List.fromList(resultData.asTypedList(resultLen));"));
    }

    #[test]
    fn renders_compound_udl_types_to_idiomatic_dart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("compound_demo.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace compound_demo {
  sequence<u32> listify(sequence<u32> values);
  record<string, u64> counts(record<string, u64> items);
  string? maybe_name(string? value);
  sequence<bytes> chunk(sequence<bytes> input);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content =
            fs::read_to_string(out_dir.join("compound_demo.dart")).expect("read generated file");
        assert!(content.contains("List<int> listify(List<int> values) {"));
        assert!(content.contains("Map<String, int> counts(Map<String, int> items) {"));
        assert!(content.contains("String? maybeName(String? value) {"));
        assert!(content.contains("List<Uint8List> chunk(List<Uint8List> input) {"));
        assert!(content.contains("import 'dart:typed_data';"));
    }

    #[test]
    fn rewrites_reserved_identifiers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("keywords.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace keywords {
  u32 class(u32 switch);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("keywords.dart")).expect("read generated");
        assert!(content.contains("int class_(int switch_) {"));
    }

    #[test]
    fn delegates_supported_top_level_functions_to_default_bindings() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("simple.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace simple {
  u32 add(u32 left, u32 right);
  string greeting(string name);
  sequence<u32> not_supported(sequence<u32> value);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("simple.dart")).expect("read generated");
        assert!(content.contains("SimpleFfi? _defaultBindings;"));
        assert!(content.contains("void configureDefaultBindings("));
        assert!(content.contains("return _bindings().add(left, right);"));
        assert!(content.contains("throw UnimplementedError('TODO: bind to Rust FFI');"));
    }

    #[test]
    fn renders_runtime_string_marshalling_and_free() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("strings.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace strings {
  string greet(string name);
  string? maybe_greet(string? name);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("strings.dart")).expect("read generated");
        assert!(content.contains("import 'package:ffi/ffi.dart';"));
        assert!(content.contains("ffi.Pointer<Utf8> nameNative = name.toNativeUtf8();"));
        assert!(content.contains("calloc.free(nameNative);"));
        assert!(content.contains("_rustStringFree(resultPtr);"));
        assert!(
            content.contains(
                "final ffi.Pointer<Utf8> nameNative = name == null ? ffi.nullptr : name.toNativeUtf8();"
            )
        );
        assert!(content.contains("if (nameNative != ffi.nullptr) calloc.free(nameNative);"));
        assert!(content.contains("if (resultPtr == ffi.nullptr) {"));
        assert!(content.contains("return null;"));
    }

    #[test]
    fn renders_runtime_bytes_marshalling_and_free() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("bytes.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace bytes_demo {
  bytes echo_bytes(bytes input);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("bytes_demo.dart")).expect("read generated");
        assert!(content.contains("final class _RustBuffer extends ffi.Struct {"));
        assert!(content.contains("late final void Function(_RustBuffer) _rustBytesFree ="));
        assert!(content.contains("final ffi.Pointer<ffi.Uint8> inputData = input.isEmpty ? ffi.nullptr : calloc<ffi.Uint8>(input.length);"));
        assert!(content.contains("inputData.asTypedList(input.length).setAll(0, input);"));
        assert!(content.contains("final _RustBuffer resultBuf = _echoBytes(inputNative);"));
        assert!(content.contains("return Uint8List.fromList(resultData.asTypedList(resultLen));"));
    }

    #[test]
    fn renders_timestamp_and_duration_runtime_conversions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("temporal.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace temporal {
  timestamp add_seconds(timestamp when, i64 seconds);
  duration multiply_duration(duration value, u32 factor);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("temporal.dart")).expect("read generated");
        assert!(content.contains("DateTime addSeconds(DateTime when_, int seconds) {"));
        assert!(
            content.contains("return DateTime.fromMicrosecondsSinceEpoch(micros, isUtc: true);")
        );
        assert!(content.contains("when_.toUtc().microsecondsSinceEpoch"));
        assert!(content.contains("Duration multiplyDuration(Duration value, int factor) {"));
        assert!(content.contains("return Duration(microseconds: micros);"));
        assert!(content.contains("value.inMicroseconds"));
    }

    #[test]
    fn renders_throwing_functions_with_typed_exceptions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("errors.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace errors {
  [Throws=MathError]
  i32 checked_divide(i32 left, i32 right);
};

[Error]
interface MathError {
  DivisionByZero();
  NegativeInput(i32 value);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("errors.dart")).expect("read generated");
        assert!(content.contains("int checkedDivide(int left, int right) {"));
        assert!(content.contains(
            "late final ffi.Pointer<Utf8> Function(int left, int right) _checkedDivide ="
        ));
        assert!(content.contains("sealed class MathErrorException implements Exception {"));
        assert!(content
            .contains("final class MathErrorExceptionDivisionByZero extends MathErrorException {"));
        assert!(content
            .contains("final class MathErrorExceptionNegativeInput extends MathErrorException {"));
        assert!(content.contains("MathErrorException _decodeMathErrorException(Object? raw) {"));
        assert!(content.contains("throw MathErrorExceptionFfiCodec.decode(errRaw);"));
        assert!(content.contains("_rustStringFree(resultPtr);"));
    }

    #[test]
    fn renders_object_classes_with_lifecycle_and_throws() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("objects.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace objects {};

[Error]
interface MathError {
  DivisionByZero();
};

dictionary Person {
  string name;
  u32 age;
};

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
};

[Traits=(Display, Hash, Eq, Ord)]
interface Counter {
  constructor(u32 initial);
  [Name=with_person]
  constructor(Person seed);
  void add_value(u32 amount);
  u32 current_value();
  void set_label(string label);
  string maybe_label(string? suffix);
  void ingest_person(Person input);
  Outcome flip_outcome(Outcome input);
  u32 bytes_len(bytes input);
  u32 optional_bytes_len(bytes? input);
  u32 chunks_total_len(sequence<bytes> input);
  string describe();
  Person snapshot_person();
  Outcome snapshot_outcome();
  bytes snapshot_bytes();
  [Throws=MathError]
  i32 divide_by(i32 divisor);
  [Throws=MathError]
  Outcome risky_outcome(i32 divisor);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("objects.dart")).expect("read generated");
        assert!(content.contains("final class Counter implements Comparable<Counter> {"));
        assert!(content.contains("Counter._(this._ffi, this._handle) {"));
        assert!(content.contains("void close() {"));
        assert!(content.contains("static Counter withPerson(Person seed) {"));
        assert!(content.contains("void addValue(int amount) {"));
        assert!(content.contains("int currentValue() {"));
        assert!(content.contains("void setLabel(String label) {"));
        assert!(content.contains("String maybeLabel(String? suffix) {"));
        assert!(content.contains("void ingestPerson(Person input) {"));
        assert!(content.contains("Outcome flipOutcome(Outcome input) {"));
        assert!(content.contains("int bytesLen(Uint8List input) {"));
        assert!(content.contains("int optionalBytesLen(Uint8List? input) {"));
        assert!(content.contains("int chunksTotalLen(List<Uint8List> input) {"));
        assert!(content.contains("String describe() {"));
        assert!(content.contains("Person snapshotPerson() {"));
        assert!(content.contains("Outcome snapshotOutcome() {"));
        assert!(content.contains("Uint8List snapshotBytes() {"));
        assert!(content.contains("int divideBy(int divisor) {"));
        assert!(content.contains("Outcome riskyOutcome(int divisor) {"));
        assert!(content.contains("String toString() {"));
        assert!(content.contains("int get hashCode {"));
        assert!(content.contains("bool operator ==(Object other) {"));
        assert!(content.contains("int compareTo(Counter other) {"));
        assert!(content.contains("return _ffi.counterInvokeUniffiTraitEq(_handle, other._handle);"));
        assert!(
            content.contains("return _ffi.counterInvokeUniffiTraitOrdCmp(_handle, other._handle);")
        );
        assert!(content.contains("return _ffi.counterInvokeUniffiTraitDisplay(_handle);"));
        assert!(content.contains("return _ffi.counterInvokeUniffiTraitHash(_handle);"));
        assert!(!content.contains("uniffiTraitDisplay() {"));
        assert!(!content.contains("uniffiTraitHash() {"));
        assert!(content.contains("late final void Function(int handle) _counterFree ="));
        assert!(content.contains("Counter counterCreateNew(int initial) {"));
        assert!(content.contains("int counterInvokeCurrentValue(int handle) {"));
        assert!(content.contains("throw MathErrorExceptionFfiCodec.decode(errRaw);"));
    }

    #[test]
    fn renders_record_and_enum_models() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("models.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace models {};

dictionary Person {
  string name;
  u32 age;
  string? nickname;
};

enum Color { "red", "blue" };

[Enum]
interface Outcome {
  Success(string message);
  Failure(i32 code, string reason);
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("models.dart")).expect("read generated");
        assert!(content.contains("class Person {"));
        assert!(content.contains("const Person({"));
        assert!(content.contains("Person copyWith({"));
        assert!(content.contains("enum Color {"));
        assert!(content.contains("sealed class Outcome {"));
        assert!(content.contains("final class OutcomeSuccess extends Outcome {"));
        assert!(content.contains("final class OutcomeFailure extends Outcome {"));
        assert!(content.contains("String _encodeOutcome(Outcome value) {"));
        assert!(content.contains("Outcome _decodeOutcome(String raw) {"));
    }

    #[test]
    fn binary_helpers_decode_error_enum_unit_variant_as_class_instance() {
        let enums = vec![UdlEnum {
            name: "MethodError".to_string(),
            docstring: None,
            is_error: true,
            variants: vec![
                UdlEnumVariant {
                    name: "division_by_zero".to_string(),
                    docstring: None,
                    fields: vec![],
                },
                UdlEnumVariant {
                    name: "negative_input".to_string(),
                    docstring: None,
                    fields: vec![UdlArg {
                        name: "value".to_string(),
                        type_: Type::Int32,
                        docstring: None,
                        default: None,
                    }],
                },
            ],
            methods: vec![],
        }];

        let content = render_uniffi_binary_helpers(&[], &enums);
        assert!(content.contains("if (value is MethodErrorDivisionByZero) {"));
        assert!(content.contains("value = const MethodErrorDivisionByZero();"));
        assert!(!content.contains("value = MethodError.divisionByZero;"));
    }

    #[test]
    fn renders_record_and_enum_methods_from_metadata() {
        let records = vec![UdlRecord {
            name: "Point".to_string(),
            docstring: None,
            fields: vec![UdlArg {
                name: "x".to_string(),
                type_: Type::Int32,
                docstring: None,
                default: None,
            }],
            methods: vec![UdlObjectMethod {
                name: "checksum".to_string(),
                ffi_symbol: None,
                ffi_arg_types: Vec::new(),
                ffi_return_type: None,
                ffi_has_rust_call_status: false,
                runtime_unsupported: None,
                docstring: None,
                is_async: false,
                return_type: Some(Type::UInt32),
                throws_type: None,
                args: vec![],
            }],
        }];
        let enums = vec![UdlEnum {
            name: "State".to_string(),
            docstring: None,
            is_error: false,
            variants: vec![
                UdlEnumVariant {
                    name: "on".to_string(),
                    docstring: None,
                    fields: vec![],
                },
                UdlEnumVariant {
                    name: "off".to_string(),
                    docstring: None,
                    fields: vec![],
                },
            ],
            methods: vec![UdlObjectMethod {
                name: "rank".to_string(),
                ffi_symbol: None,
                ffi_arg_types: Vec::new(),
                ffi_return_type: None,
                ffi_has_rust_call_status: false,
                runtime_unsupported: None,
                docstring: None,
                is_async: false,
                return_type: Some(Type::UInt32),
                throws_type: None,
                args: vec![],
            }],
        }];

        let content = render_dart_scaffold(
            "models",
            "ModelsFfi",
            "uniffi_models",
            None,
            "crate_name",
            None,
            None,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &[],
            &[],
            &[],
            &records,
            &enums,
        );

        assert!(content.contains("int checksum() {"));
        assert!(content.contains("return _bindings().pointChecksum(this);"));
        assert!(content.contains("extension StateMethods on State {"));
        assert!(content.contains("int rank() {"));
        assert!(content.contains("return _bindings().stateRank(this);"));
        assert!(content.contains("_pointChecksum = _lib.lookupFunction<"));
        assert!(content.contains("_stateRank = _lib.lookupFunction<"));
        assert!(content.contains(">('point_checksum');"));
        assert!(content.contains(">('state_rank');"));
    }

    #[test]
    fn ffibuffer_eligibility_allows_sync_non_throwing_rustbuffer_signatures() {
        let function = UdlFunction {
            name: "methodpoint_checksum".to_string(),
            ffi_symbol: Some(
                "uniffi_uniffi_record_enum_methods_fn_method_methodpoint_checksum".to_string(),
            ),
            ffi_arg_types: vec![FfiType::RustBuffer(None)],
            ffi_return_type: Some(FfiType::UInt32),
            ffi_has_rust_call_status: true,
            runtime_unsupported: Some("placeholder".to_string()),
            docstring: None,
            is_async: false,
            return_type: Some(Type::UInt32),
            throws_type: None,
            args: vec![UdlArg {
                name: "self".to_string(),
                type_: Type::Record {
                    module_path: "crate_name".to_string(),
                    name: "MethodPoint".to_string(),
                },
                docstring: None,
                default: None,
            }],
        };
        assert!(is_ffibuffer_eligible_function(&function));
    }

    #[test]
    fn ffibuffer_eligibility_rejects_async_functions_and_allows_throwing_functions() {
        let async_function = UdlFunction {
            name: "methodpoint_async_label".to_string(),
            ffi_symbol: Some(
                "uniffi_uniffi_record_enum_methods_fn_method_methodpoint_async_label".to_string(),
            ),
            ffi_arg_types: vec![FfiType::RustBuffer(None), FfiType::RustBuffer(None)],
            ffi_return_type: Some(FfiType::Handle),
            ffi_has_rust_call_status: false,
            runtime_unsupported: Some("placeholder".to_string()),
            docstring: None,
            is_async: true,
            return_type: Some(Type::String),
            throws_type: None,
            args: vec![],
        };
        assert!(!is_ffibuffer_eligible_function(&async_function));

        let throwing_function = UdlFunction {
            name: "methodpoint_checked_divide".to_string(),
            ffi_symbol: Some(
                "uniffi_uniffi_record_enum_methods_fn_method_methodpoint_checked_divide"
                    .to_string(),
            ),
            ffi_arg_types: vec![FfiType::RustBuffer(None), FfiType::UInt32],
            ffi_return_type: Some(FfiType::UInt32),
            ffi_has_rust_call_status: true,
            runtime_unsupported: Some("placeholder".to_string()),
            docstring: None,
            is_async: false,
            return_type: Some(Type::UInt32),
            throws_type: Some(Type::Enum {
                module_path: "crate_name".to_string(),
                name: "MethodError".to_string(),
            }),
            args: vec![],
        };
        assert!(is_ffibuffer_eligible_function(&throwing_function));
    }

    #[test]
    fn renders_ffibuffer_fallback_for_runtime_unsupported_object_members() {
        let objects = vec![UdlObject {
            name: "widget".to_string(),
            docstring: None,
            constructors: vec![UdlObjectConstructor {
                name: "new".to_string(),
                ffi_symbol: Some("uniffi_demo_ctor_widget_new".to_string()),
                ffi_arg_types: vec![FfiType::UInt32],
                ffi_return_type: Some(FfiType::Handle),
                ffi_has_rust_call_status: true,
                runtime_unsupported: Some("placeholder".to_string()),
                docstring: None,
                is_async: false,
                args: vec![UdlArg {
                    name: "count".to_string(),
                    type_: Type::UInt32,
                    docstring: None,
                    default: None,
                }],
                throws_type: None,
            }],
            methods: vec![
                UdlObjectMethod {
                    name: "value".to_string(),
                    ffi_symbol: Some("uniffi_demo_method_widget_value".to_string()),
                    ffi_arg_types: vec![FfiType::Handle],
                    ffi_return_type: Some(FfiType::UInt32),
                    ffi_has_rust_call_status: true,
                    runtime_unsupported: Some("placeholder".to_string()),
                    docstring: None,
                    is_async: false,
                    return_type: Some(Type::UInt32),
                    throws_type: None,
                    args: vec![],
                },
                UdlObjectMethod {
                    name: "async_label".to_string(),
                    ffi_symbol: Some("uniffi_demo_method_widget_async_label".to_string()),
                    ffi_arg_types: vec![FfiType::Handle, FfiType::RustBuffer(None)],
                    ffi_return_type: Some(FfiType::Handle),
                    ffi_has_rust_call_status: false,
                    runtime_unsupported: Some("placeholder".to_string()),
                    docstring: None,
                    is_async: true,
                    return_type: Some(Type::String),
                    throws_type: None,
                    args: vec![UdlArg {
                        name: "prefix".to_string(),
                        type_: Type::String,
                        docstring: None,
                        default: None,
                    }],
                },
            ],
            trait_methods: UdlObjectTraitMethods::default(),
        }];

        let content = render_dart_scaffold(
            "demo",
            "DemoFfi",
            "uniffi_demo",
            None,
            "crate_name",
            None,
            None,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &[],
            &objects,
            &[],
            &[],
            &[],
        );

        assert!(content.contains("_widgetCtorNewFfiBuffer"));
        assert!(content.contains("uniffi_ffibuffer_demo_ctor_widget_new"));
        assert!(content.contains("_widgetValueFfiBuffer"));
        assert!(content.contains("uniffi_ffibuffer_demo_method_widget_value"));
        assert!(!content.contains("throw UnsupportedError('placeholder (new)');"));
        assert!(!content.contains("throw UnsupportedError('placeholder (value)');"));
        assert!(content.contains("throw UnsupportedError('placeholder (async_label)');"));
    }

    #[test]
    fn renders_async_ffibuffer_fallback_for_runtime_unsupported_functions() {
        let functions = vec![UdlFunction {
            name: "greet_async".to_string(),
            ffi_symbol: Some("uniffi_uniffi_demo_fn_func_greet_async".to_string()),
            ffi_arg_types: vec![FfiType::RustBuffer(None)],
            ffi_return_type: Some(FfiType::Handle),
            ffi_has_rust_call_status: true,
            runtime_unsupported: Some("placeholder".to_string()),
            docstring: None,
            is_async: true,
            return_type: Some(Type::String),
            throws_type: None,
            args: vec![UdlArg {
                name: "name".to_string(),
                type_: Type::String,
                docstring: None,
                default: None,
            }],
        }];

        let content = render_dart_scaffold(
            "demo",
            "DemoFfi",
            "uniffi_demo",
            None,
            "crate_name",
            None,
            None,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &functions,
            &[],
            &[],
            &[],
            &[],
        );

        assert!(content.contains("uniffi_ffibuffer_uniffi_demo_fn_func_greet_async"));
        assert!(content.contains("ffi_uniffi_demo_rust_future_complete_rust_buffer"));
        assert!(content.contains("Future<String> greetAsync(String name) {"));
        assert!(!content.contains("throw UnsupportedError('placeholder (greet_async)');"));
    }

    #[test]
    fn renders_default_values_for_public_dart_apis() {
        let callable_args = vec![
            UdlArg {
                name: "left".to_string(),
                type_: Type::UInt32,
                docstring: None,
                default: Some(DefaultValue::Literal(Literal::new_uint(7))),
            },
            UdlArg {
                name: "right".to_string(),
                type_: Type::UInt32,
                docstring: None,
                default: Some(DefaultValue::Literal(Literal::new_uint(9))),
            },
            UdlArg {
                name: "label".to_string(),
                type_: Type::String,
                docstring: None,
                default: Some(DefaultValue::Literal(Literal::String("world".to_string()))),
            },
        ];
        let rendered = render_callable_args_signature(&callable_args, &[]);
        assert_eq!(
            rendered,
            "{int left = 7, int right = 9, String label = 'world'}"
        );

        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("defaults.udl");
        let out_dir = temp.path().join("out");
        fs::write(
            &source,
            r#"
namespace defaults {};

dictionary Config {
  boolean enabled = true;
  string label = "alpha";
  u32 count;
};
"#,
        )
        .expect("write source");

        let args = GenerateArgs {
            source,
            out_dir: out_dir.clone(),
            library: false,
            config: None,
            crate_name: None,
            no_format: false,
        };

        generate_bindings(&args).expect("generate");
        let content = fs::read_to_string(out_dir.join("defaults.dart")).expect("read generated");
        assert!(content.contains("class Config {"));
        assert!(content.contains("this.enabled = true,"));
        assert!(content.contains("this.label = 'alpha',"));
        assert!(content.contains("required this.count,"));
        assert!(content
            .contains("enabled: json.containsKey('enabled') ? json['enabled'] as bool : true,"));
        assert!(content
            .contains("label: json.containsKey('label') ? json['label'] as String : 'alpha',"));
    }
}
