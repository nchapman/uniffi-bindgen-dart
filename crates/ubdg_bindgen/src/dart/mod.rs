use std::collections::HashMap;
use std::fs;

use anyhow::{Context, Result};
#[cfg(test)]
use uniffi_bindgen::interface::{ffi::FfiType, DefaultValue, Literal, Type};

use crate::GenerateArgs;

mod async_support;
mod callback;
mod codec;
pub mod config;
mod ffi_buffer;
mod naming;
mod parsing;
mod render_bound_methods;
mod render_data_models;
mod render_helpers;
mod render_objects;
mod render_stubs;
mod type_map;
mod types;
use async_support::*;
use callback::*;
use codec::*;
use ffi_buffer::*;
use naming::*;
use parsing::*;
use render_bound_methods::*;
use render_data_models::*;
use render_helpers::*;
use render_objects::*;
use render_stubs::*;
use type_map::*;
use types::*;

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
    let ctx = RenderContext {
        module_name: &module_name,
        ffi_class_name: &ffi_class_name,
        library_name: &library_name,
        namespace_docstring: metadata.namespace_docstring.as_deref(),
        local_module_path: &metadata.local_module_path,
        uniffi_contract_version: metadata.uniffi_contract_version,
        ffi_uniffi_contract_version_symbol: metadata.ffi_uniffi_contract_version_symbol.as_deref(),
        api_checksums: &metadata.api_checksums,
        external_packages: &cfg.external_packages,
        api_overrides: ApiOverrides::new(&cfg.rename, &cfg.exclude),
        functions: &metadata.functions,
        objects: &metadata.objects,
        callback_interfaces: &metadata.callback_interfaces,
        records: &metadata.records,
        enums: &metadata.enums,
    };
    let content = render_dart_scaffold(&ctx);
    fs::write(&output_file, content).with_context(|| {
        format!(
            "failed to write generated dart bindings: {}",
            output_file.display()
        )
    })?;

    Ok(())
}

struct RenderContext<'a> {
    module_name: &'a str,
    ffi_class_name: &'a str,
    library_name: &'a str,
    namespace_docstring: Option<&'a str>,
    local_module_path: &'a str,
    uniffi_contract_version: Option<u32>,
    ffi_uniffi_contract_version_symbol: Option<&'a str>,
    api_checksums: &'a [UdlApiChecksum],
    external_packages: &'a HashMap<String, String>,
    api_overrides: ApiOverrides,
    functions: &'a [UdlFunction],
    objects: &'a [UdlObject],
    callback_interfaces: &'a [UdlCallbackInterface],
    records: &'a [UdlRecord],
    enums: &'a [UdlEnum],
}

fn render_dart_scaffold(ctx: &RenderContext<'_>) -> String {
    let RenderContext {
        module_name,
        ffi_class_name,
        library_name,
        namespace_docstring,
        local_module_path,
        uniffi_contract_version,
        ffi_uniffi_contract_version_symbol,
        api_checksums,
        external_packages,
        ref api_overrides,
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
    } = *ctx;
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
        || functions.iter().any(function_uses_bytes)
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
                && (function_uses_runtime_string(f)
                    || function_uses_runtime_bytes(f)
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
    let needs_runtime_bytes = functions.iter().any(|f| {
        is_runtime_ffi_compatible_function(f, records, enums) && function_uses_runtime_bytes(f)
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
        api_overrides,
        records,
        enums,
    ));
    out.push_str(&render_function_stubs(
        functions,
        objects,
        callback_interfaces,
        ffi_class_name,
        api_overrides,
        records,
        enums,
    ));
    out
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

        let content = render_dart_scaffold(&RenderContext {
            module_name: "models",
            ffi_class_name: "ModelsFfi",
            library_name: "uniffi_models",
            namespace_docstring: None,
            local_module_path: "crate_name",
            uniffi_contract_version: None,
            ffi_uniffi_contract_version_symbol: None,
            api_checksums: &[],
            external_packages: &HashMap::new(),
            api_overrides: ApiOverrides::new(&HashMap::new(), &[]),
            functions: &[],
            objects: &[],
            callback_interfaces: &[],
            records: &records,
            enums: &enums,
        });

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

        let content = render_dart_scaffold(&RenderContext {
            module_name: "demo",
            ffi_class_name: "DemoFfi",
            library_name: "uniffi_demo",
            namespace_docstring: None,
            local_module_path: "crate_name",
            uniffi_contract_version: None,
            ffi_uniffi_contract_version_symbol: None,
            api_checksums: &[],
            external_packages: &HashMap::new(),
            api_overrides: ApiOverrides::new(&HashMap::new(), &[]),
            functions: &[],
            objects: &objects,
            callback_interfaces: &[],
            records: &[],
            enums: &[],
        });

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

        let content = render_dart_scaffold(&RenderContext {
            module_name: "demo",
            ffi_class_name: "DemoFfi",
            library_name: "uniffi_demo",
            namespace_docstring: None,
            local_module_path: "crate_name",
            uniffi_contract_version: None,
            ffi_uniffi_contract_version_symbol: None,
            api_checksums: &[],
            external_packages: &HashMap::new(),
            api_overrides: ApiOverrides::new(&HashMap::new(), &[]),
            functions: &functions,
            objects: &[],
            callback_interfaces: &[],
            records: &[],
            enums: &[],
        });

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
