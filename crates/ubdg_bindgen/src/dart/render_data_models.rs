use std::collections::HashMap;

use uniffi_bindgen::interface::Literal;

use super::config::CustomTypeConfig;
use super::*;

pub(super) fn render_data_models(
    records: &[UdlRecord],
    enums: &[UdlEnum],
    callback_interfaces: &[UdlCallbackInterface],
    emit_uniffi_error_lift_helpers: bool,
    custom_types: &HashMap<String, CustomTypeConfig>,
) -> String {
    let mut out = String::new();

    for record in records {
        let class_name = to_upper_camel(&record.name);
        out.push_str(&render_doc_comment(record.docstring.as_deref(), ""));
        out.push_str(&format!("class {class_name} {{\n"));
        if record.fields.is_empty() {
            out.push_str(&format!("  const {class_name}();\n\n"));
        } else {
            out.push_str(&format!("  const {class_name}({{\n"));
            for field in &record.fields {
                out.push_str(&render_doc_comment(field.docstring.as_deref(), "    "));
                let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
                if let Some(default_expr) = field
                    .default
                    .as_ref()
                    .and_then(|d| render_default_value_expr(d, &field.type_, enums, custom_types))
                {
                    out.push_str(&format!("    this.{field_name} = {default_expr},\n"));
                } else {
                    out.push_str(&format!("    required this.{field_name},\n"));
                }
            }
            out.push_str("  });\n\n");
        }
        for field in &record.fields {
            out.push_str(&render_doc_comment(field.docstring.as_deref(), "  "));
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            out.push_str(&format!(
                "  final {} {field_name};\n",
                map_uniffi_type_to_dart(&field.type_, custom_types)
            ));
        }
        out.push('\n');
        out.push_str("  Map<String, dynamic> toJson() {\n");
        out.push_str("    return {\n");
        for field in &record.fields {
            let field_name = safe_dart_identifier(&to_lower_camel(&field.name));
            let expr =
                render_json_encode_expr(&format!("this.{field_name}"), &field.type_, custom_types);
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
            let decode = render_json_decode_expr(
                &format!("json['{field_name}']"),
                &field.type_,
                custom_types,
            );
            if let Some(default_expr) = field
                .default
                .as_ref()
                .and_then(|d| render_default_value_expr(d, &field.type_, enums, custom_types))
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
                let field_type = map_uniffi_type_to_dart(&field.type_, custom_types);
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
                .map(|t| map_uniffi_type_to_dart(t, custom_types))
                .unwrap_or_else(|| "void".to_string());
            let signature_return = if method.is_async {
                format!("Future<{return_type}>")
            } else {
                return_type.clone()
            };
            let args = render_callable_args_signature(&method.args, enums, custom_types);
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
        out.push_str(&render_record_trait_methods(
            &class_name,
            &record.fields,
            &record.traits,
        ));

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
                let variant_name = safe_dart_identifier(&to_lower_camel(&variant.name));
                if let Some(lit) = &variant.discr {
                    out.push_str(&format!(
                        "  {variant_name}({}),\n",
                        render_discr_literal(lit)
                    ));
                } else {
                    out.push_str(&format!("  {variant_name},\n"));
                }
            }
            if enum_.is_non_exhaustive {
                out.push_str(
                    "  /// Unknown variant for forward-compatibility with non-exhaustive enums.\n",
                );
                if enum_.has_discr_type {
                    out.push_str("  unknown(-1);\n");
                } else {
                    out.push_str("  unknown,\n");
                }
            }
            if enum_.has_discr_type {
                // Enhanced enum with discriminant values (Dart 2.17+).
                if !enum_.is_non_exhaustive {
                    // Replace trailing comma on last variant with semicolon.
                    let trimmed = out.trim_end().trim_end_matches(',');
                    out.truncate(trimmed.len());
                    out.push_str(";\n");
                }
                out.push('\n');
                out.push_str(&format!("  const {enum_name}(this.value);\n\n"));
                out.push_str("  /// The raw discriminant value of this enum variant.\n");
                out.push_str("  final int value;\n");
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
                        .map(|t| map_uniffi_type_to_dart(t, custom_types))
                        .unwrap_or_else(|| "void".to_string());
                    let signature_return = if method.is_async {
                        format!("Future<{return_type}>")
                    } else {
                        return_type.clone()
                    };
                    let args = render_callable_args_signature(&method.args, enums, custom_types);
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
                .map(|t| map_uniffi_type_to_dart(t, custom_types))
                .unwrap_or_else(|| "void".to_string());
            let signature_return = if method.is_async {
                format!("Future<{return_type}>")
            } else {
                return_type.clone()
            };
            let args = render_callable_args_signature(&method.args, enums, custom_types);
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
                    map_uniffi_type_to_dart(&field.type_, custom_types)
                ));
            }
            out.push_str(&render_sealed_variant_trait_methods(
                &class_name,
                &variant.fields,
                &enum_.traits,
            ));
            out.push_str("}\n\n");
        }
        if enum_.is_non_exhaustive {
            let unknown_class = format!("{enum_name}Unknown");
            out.push_str(
                "/// Unknown variant for forward-compatibility with non-exhaustive enums.\n",
            );
            out.push_str(&format!(
                "final class {unknown_class} extends {enum_name} {{\n"
            ));
            out.push_str(&format!("  const {unknown_class}();\n"));
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
                        map_uniffi_type_to_dart(&field.type_, custom_types)
                    ));
                }
            }
            out.push_str("}\n\n");
        }
        if enum_.is_non_exhaustive {
            let unknown_exception = format!("{exception_name}Unknown");
            out.push_str(
                "/// Unknown variant for forward-compatibility with non-exhaustive error enums.\n",
            );
            out.push_str(&format!(
                "final class {unknown_exception} extends {exception_name} {{\n"
            ));
            out.push_str(&format!("  const {unknown_exception}();\n"));
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
            if enum_.is_non_exhaustive {
                let unknown_exception = format!("{exception_name}Unknown");
                out.push_str(&format!("  return const {unknown_exception}();\n"));
            } else {
                out.push_str(&format!(
                    "  throw StateError('Unknown {enum_name} error variant while lifting exception: $value');\n"
                ));
            }
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
            if enum_.is_non_exhaustive {
                out.push_str(&format!("    {enum_name}.unknown => 'unknown',\n"));
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
            if enum_.is_non_exhaustive {
                out.push_str(&format!("      return {enum_name}.unknown;\n"));
            } else {
                out.push_str(&format!(
                    "      throw StateError('Unknown {} variant: $raw');\n",
                    enum_name
                ));
            }
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
                let expr = render_json_encode_expr(
                    &format!("value.{field_name}"),
                    &field.type_,
                    custom_types,
                );
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
                let decode = render_json_decode_expr(
                    &format!("map['{field_name}']"),
                    &field.type_,
                    custom_types,
                );
                out.push_str(&format!("        {field_name}: {decode},\n"));
            }
            out.push_str("      );\n");
        }
        out.push_str("    default:\n");
        if enum_.is_non_exhaustive {
            let unknown_class = format!("{enum_name}Unknown");
            out.push_str(&format!("      return const {unknown_class}();\n"));
        } else {
            out.push_str(&format!(
                "      throw StateError('Unknown {} variant tag: $tag');\n",
                enum_name
            ));
        }
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
                let expr = render_json_encode_expr(
                    &format!("value.{field_name}"),
                    &field.type_,
                    custom_types,
                );
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
                    let decode = render_json_decode_expr(
                        &format!("map['{field_name}']"),
                        &field.type_,
                        custom_types,
                    );
                    out.push_str(&format!("        {field_name}: {decode},\n"));
                }
                out.push_str("      );\n");
            }
        }
        out.push_str("    default:\n");
        if enum_.is_non_exhaustive {
            let unknown_exception = format!("{exception_name}Unknown");
            out.push_str(&format!("      return const {unknown_exception}();\n"));
        } else {
            out.push_str(&format!(
                "      throw StateError('Unknown {} exception tag: $tag');\n",
                exception_name
            ));
        }
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

/// Render `toString`, `operator ==`, and `hashCode` for a record class
/// when the corresponding traits are declared via `[Traits=(Display, Eq, Hash)]`.
fn render_record_trait_methods(class_name: &str, fields: &[UdlArg], traits: &[String]) -> String {
    let mut out = String::new();
    let has_display = traits.iter().any(|t| t == "Display");
    let has_eq = traits.iter().any(|t| t == "Eq");
    let has_hash = traits.iter().any(|t| t == "Hash");

    if has_display {
        out.push('\n');
        out.push_str("  @override\n");
        out.push_str("  String toString() {\n");
        let field_parts: Vec<String> = fields
            .iter()
            .map(|f| {
                let name = safe_dart_identifier(&to_lower_camel(&f.name));
                format!("{name}: ${name}")
            })
            .collect();
        out.push_str(&format!(
            "    return '{class_name}({})';\n",
            field_parts.join(", ")
        ));
        out.push_str("  }\n");
    }

    if has_eq {
        out.push('\n');
        out.push_str("  @override\n");
        out.push_str("  bool operator ==(Object other) =>\n");
        out.push_str("      identical(this, other) ||\n");
        let field_comparisons: Vec<String> = fields
            .iter()
            .map(|f| {
                let name = safe_dart_identifier(&to_lower_camel(&f.name));
                format!("{name} == other.{name}")
            })
            .collect();
        if field_comparisons.is_empty() {
            out.push_str(&format!("      other is {class_name};\n"));
        } else {
            out.push_str(&format!(
                "      other is {class_name} && {};\n",
                field_comparisons.join(" && ")
            ));
        }
    }

    if has_hash {
        out.push('\n');
        out.push_str("  @override\n");
        let field_names: Vec<String> = fields
            .iter()
            .map(|f| safe_dart_identifier(&to_lower_camel(&f.name)))
            .collect();
        if field_names.is_empty() {
            out.push_str("  int get hashCode => runtimeType.hashCode;\n");
        } else if field_names.len() <= 20 {
            out.push_str(&format!(
                "  int get hashCode => Object.hash({});\n",
                field_names.join(", ")
            ));
        } else {
            out.push_str(&format!(
                "  int get hashCode => Object.hashAll([{}]);\n",
                field_names.join(", ")
            ));
        }
    }

    out
}

/// Render `toString`, `operator ==`, and `hashCode` for a sealed enum variant class
/// when the corresponding traits are declared via `[Traits=(Display, Eq, Hash)]`.
fn render_sealed_variant_trait_methods(
    class_name: &str,
    fields: &[UdlArg],
    traits: &[String],
) -> String {
    // Reuse the same logic as records -- the generated methods are structurally identical.
    render_record_trait_methods(class_name, fields, traits)
}

/// Render a discriminant literal as a Dart integer literal.
fn render_discr_literal(lit: &Literal) -> String {
    match lit {
        Literal::UInt(v, _, _) => v.to_string(),
        Literal::Int(v, _, _) => v.to_string(),
        _ => "0".to_string(), // unreachable for valid discriminants
    }
}
