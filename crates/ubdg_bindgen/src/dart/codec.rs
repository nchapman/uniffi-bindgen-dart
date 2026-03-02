use uniffi_bindgen::interface::Type;

use super::*;

pub(super) fn render_json_encode_expr(value_expr: &str, type_: &Type) -> String {
    match type_ {
        Type::Timestamp => format!("{value_expr}.toUtc().microsecondsSinceEpoch"),
        Type::Duration => format!("{value_expr}.inMicroseconds"),
        Type::Bytes => format!("base64Encode({value_expr})"),
        Type::Optional { inner_type } => {
            let inner = render_json_encode_expr("__tmp", inner_type);
            format!(
                "{value_expr} == null ? null : (() {{ final __tmp = {value_expr}!; return {inner}; }})()"
            )
        }
        Type::Sequence { inner_type } => {
            let inner = render_json_encode_expr("item", inner_type);
            format!("{value_expr}.map((item) => {inner}).toList()")
        }
        // Only reached for string-keyed maps; non-string maps use the binary codec path.
        Type::Map {
            key_type,
            value_type,
        } => {
            debug_assert!(
                is_runtime_string_type(key_type),
                "render_json_encode_expr called for non-string map key: {:?}",
                key_type
            );
            let inner = render_json_encode_expr("value", value_type);
            format!("{value_expr}.map((key, value) => MapEntry(key, {inner}))")
        }
        Type::Custom { builtin, .. } => render_json_encode_expr(value_expr, builtin),
        Type::Record { .. } => format!("{value_expr}.toJson()"),
        Type::Object { name, .. } => {
            format!("{}FfiCodec.lower({value_expr})", to_upper_camel(name))
        }
        Type::Enum { name, .. } => {
            format!("{}FfiCodec.encode({value_expr})", to_upper_camel(name))
        }
        _ => value_expr.to_string(),
    }
}

pub(super) fn render_json_decode_expr(value_expr: &str, type_: &Type) -> String {
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
        // Only reached for string-keyed maps; non-string maps use the binary codec path.
        Type::Map {
            key_type,
            value_type,
        } => {
            debug_assert!(
                is_runtime_string_type(key_type),
                "render_json_decode_expr called for non-string map key: {:?}",
                key_type
            );
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

/// Returns true when every field in the list can be serialized by the binary codec.
fn all_fields_binary_supported(fields: &[UdlArg], enums: &[UdlEnum]) -> bool {
    fields
        .iter()
        .all(|f| is_binary_supported_type(&f.type_, enums))
}

fn is_binary_supported_type(type_: &Type, enums: &[UdlEnum]) -> bool {
    match type_ {
        Type::Custom { builtin, .. } => is_binary_supported_type(builtin, enums),
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
        | Type::String
        | Type::Bytes
        | Type::Timestamp
        | Type::Duration => true,
        Type::Optional { inner_type } => is_binary_supported_type(inner_type, enums),
        Type::Sequence { inner_type } => is_binary_supported_type(inner_type, enums),
        Type::Map {
            key_type,
            value_type,
        } => {
            is_binary_supported_type(key_type, enums) && is_binary_supported_type(value_type, enums)
        }
        Type::Record { .. } => true,
        Type::Enum { .. } if is_runtime_enum_type(type_, enums) => true,
        _ => false,
    }
}

pub(super) fn render_uniffi_binary_helpers(records: &[UdlRecord], enums: &[UdlEnum]) -> String {
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
        let supported = all_fields_binary_supported(&record.fields, enums);
        out.push_str(&format!(
            "Uint8List _uniffiEncode{type_name}({type_name} value) {{\n"
        ));
        if supported {
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
        } else {
            out.push_str(&format!(
                "  throw UnsupportedError('UniFFI binary encode not fully supported for {type_name}');\n"
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!(
            "{type_name} _uniffiDecode{type_name}(Uint8List bytes) {{\n"
        ));
        if supported {
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
        } else {
            out.push_str(&format!(
                "  throw UnsupportedError('UniFFI binary decode not fully supported for {type_name}');\n"
            ));
        }
        out.push_str("}\n\n");
    }

    for enum_ in enums {
        let type_name = to_upper_camel(&enum_.name);
        let is_flat_enum = !enum_.is_error && enum_.variants.iter().all(|v| v.fields.is_empty());
        let all_variants_supported = enum_
            .variants
            .iter()
            .all(|v| all_fields_binary_supported(&v.fields, enums));
        out.push_str(&format!(
            "Uint8List _uniffiEncode{type_name}({type_name} value) {{\n"
        ));
        if !all_variants_supported {
            out.push_str(&format!(
                "  throw UnsupportedError('UniFFI binary encode not fully supported for {type_name}');\n"
            ));
            out.push_str("}\n\n");
            out.push_str(&format!(
                "{type_name} _uniffiDecode{type_name}(Uint8List bytes) {{\n"
            ));
            out.push_str(&format!(
                "  throw UnsupportedError('UniFFI binary decode not fully supported for {type_name}');\n"
            ));
            out.push_str("}\n\n");
            continue;
        }
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
            if enum_.is_non_exhaustive {
                out.push_str(&format!(
                    "    {type_name}.unknown => throw StateError('Cannot encode unknown {type_name} variant'),\n"
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
        if enum_.is_non_exhaustive {
            if is_flat_enum {
                out.push_str(&format!("      value = {type_name}.unknown;\n"));
            } else {
                let unknown_class = format!("{type_name}Unknown");
                out.push_str(&format!("      value = const {unknown_class}();\n"));
            }
            out.push_str("      return value;\n");
        } else {
            out.push_str(&format!(
                "      throw StateError('Unknown {type_name} variant tag: $tag');\n"
            ));
        }
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

pub(super) fn render_uniffi_binary_write_statement(
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
        Type::Map { key_type, value_type } => {
            let key_stmt =
                render_uniffi_binary_write_statement(key_type, "entry.key", writer, enums, &(indent.to_string() + "  "));
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

pub(super) fn render_uniffi_binary_read_expression(
    type_: &Type,
    reader: &str,
    enums: &[UdlEnum],
) -> String {
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
        Type::Map {
            key_type,
            value_type,
        } => {
            let key = render_uniffi_binary_read_expression(key_type, reader, enums);
            let key_type_name = map_uniffi_type_to_dart(key_type);
            let value = render_uniffi_binary_read_expression(value_type, reader, enums);
            let value_type_name = map_uniffi_type_to_dart(value_type);
            format!(
                "(() {{ final int __len = {reader}.readI32(); final out = <{key_type_name}, {value_type_name}>{{}}; for (var i = 0; i < __len; i++) {{ final key = {key}; final value = {value}; out[key] = value; }} return out; }})()"
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
