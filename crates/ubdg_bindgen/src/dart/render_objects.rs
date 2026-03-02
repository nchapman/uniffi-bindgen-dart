use super::*;

pub(super) fn render_object_classes(
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
                if is_runtime_async_rust_future_compatible_constructor(
                    ctor,
                    callback_interfaces,
                    records,
                    enums,
                ) {
                    out.push_str(&format!("    return {invoke_expr};\n"));
                } else {
                    out.push_str(&format!("    return Future(() => {invoke_expr});\n"));
                }
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
