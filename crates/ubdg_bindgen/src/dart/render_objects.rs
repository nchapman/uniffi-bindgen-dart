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
        if object.has_callback_interface {
            out.push_str(&render_trait_interface(
                object,
                callback_interfaces,
                ffi_class_name,
                api_overrides,
                records,
                enums,
            ));
        } else {
            out.push_str(&render_plain_object(
                object,
                callback_interfaces,
                ffi_class_name,
                api_overrides,
                records,
                enums,
            ));
        }
    }

    out
}

/// Render a `[Trait, WithForeign]` interface.
///
/// Produces:
/// 1. An abstract interface class with trait method signatures.
/// 2. A `_Impl` final class that wraps a Rust-originated handle.
/// 3. A vtable struct, callback bridge, and codec that supports both
///    Rust-backed and Dart-backed implementations.
fn render_trait_interface(
    object: &UdlObject,
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    api_overrides: &ApiOverrides,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let object_name = api_overrides
        .renamed_or_default(&ApiOverrides::object_key(&object.name), || {
            to_upper_camel(&object.name)
        });
    let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
    let impl_class_name = format!("_{object_name}Impl");
    let free_field = format!("_{}Free", object_lower);
    let token_name = format!("_{}FinalizerToken", object_name);
    let bridge_name = trait_callback_bridge_class_name(&object.name);
    let vtable_struct_name = trait_callback_vtable_struct_name(&object.name);

    let mut out = String::new();

    // 1. Abstract interface class
    out.push('\n');
    out.push_str(&render_doc_comment(object.docstring.as_deref(), ""));
    out.push_str(&format!("abstract interface class {object_name} {{\n"));
    for method in &object.methods {
        if is_uniffi_trait_method_name(&method.name) {
            continue;
        }
        if api_overrides.excluded(&ApiOverrides::object_member_key(&object.name, &method.name)) {
            continue;
        }
        let method_name = safe_dart_identifier(&api_overrides.renamed_or_default(
            &ApiOverrides::object_member_key(&object.name, &method.name),
            || to_lower_camel(&method.name),
        ));
        let value_return_type = method
            .return_type
            .as_ref()
            .map(map_uniffi_type_to_dart)
            .unwrap_or_else(|| "void".to_string());
        let signature_return = if method.is_async {
            format!("Future<{value_return_type}>")
        } else {
            value_return_type
        };
        let args = render_callable_args_signature(&method.args, enums);
        out.push_str(&render_doc_comment(method.docstring.as_deref(), "  "));
        out.push_str(&format!("  {signature_return} {method_name}({args});\n"));
    }
    out.push_str("}\n\n");

    // 2. Finalizer token + concrete implementation class
    out.push_str(&format!(
        "final class {token_name} {{\n  const {token_name}(this.free, this.handle);\n  final void Function(int) free;\n  final int handle;\n}}\n\n"
    ));
    let mut impl_implements = vec![object_name.clone()];
    if object.is_error {
        impl_implements.push("Exception".to_string());
    }
    if object.trait_methods.ord_cmp.is_some() {
        impl_implements.push(format!("Comparable<{object_name}>"));
    }
    out.push_str(&format!(
        "final class {impl_class_name} implements {} {{\n",
        impl_implements.join(", ")
    ));
    out.push_str(&format!(
        "  {impl_class_name}._(this._ffi, this._handle) {{\n"
    ));
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
        "      throw StateError('{impl_class_name} is closed');\n"
    ));
    out.push_str("    }\n");
    out.push_str("  }\n\n");

    // Constructors on the impl class
    for ctor in &object.constructors {
        if api_overrides.excluded(&ApiOverrides::object_member_key(&object.name, &ctor.name)) {
            continue;
        }
        if !ctor
            .args
            .iter()
            .all(|a| is_runtime_ffi_compatible_type(&a.type_, records, enums))
        {
            emit_constructor_skip_warning(&mut out, &impl_class_name, &ctor.name, &ctor.args, "  ");
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
            format!("Future<{impl_class_name}>")
        } else {
            impl_class_name.clone()
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

    // Instance methods on the impl class
    for method in &object.methods {
        if is_uniffi_trait_method_name(&method.name) {
            continue;
        }
        if api_overrides.excluded(&ApiOverrides::object_member_key(&object.name, &method.name)) {
            continue;
        }
        let has_callback_args =
            has_runtime_callback_args_in_args(&method.args, callback_interfaces, records, enums);
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
            emit_method_skip_warning(&mut out, &impl_class_name, &method.name, &method.args, "  ");
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
        out.push_str("  @override\n");
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

    // Trait method overrides (toString, hashCode, ==, compareTo)
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
        out.push_str(&format!("      return '{impl_class_name}(closed)';\n"));
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
        out.push_str(&format!("    if (other is! {impl_class_name}) {{\n"));
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
        out.push_str(&format!("    if (other is! {impl_class_name}) {{\n"));
        out.push_str("      throw ArgumentError('Can only compare Rust-backed instances');\n");
        out.push_str("    }\n");
        out.push_str("    other._ensureOpen();\n");
        out.push_str(&format!(
            "    return _ffi.{invoke_name}(_handle, other._handle);\n"
        ));
        out.push_str("  }\n\n");
    }
    out.push_str("}\n\n");

    // 3. VTable struct for callback dispatch
    out.push_str(&render_trait_vtable_struct(
        object,
        &vtable_struct_name,
        records,
        enums,
    ));

    // 4. Callback bridge class
    out.push_str(&render_trait_callback_bridge(
        object,
        &object_name,
        &bridge_name,
        &vtable_struct_name,
        records,
        enums,
    ));

    // 5. FfiCodec that handles both Rust-backed and Dart-backed instances
    let object_type_name = to_upper_camel(&object.name);
    let codec_name = format!("{object_type_name}FfiCodec");
    let init_done_field = trait_callback_init_done_field_name(&object.name);
    out.push_str(&format!("final class {codec_name} {{\n"));
    out.push_str(&format!("  const {codec_name}._();\n\n"));
    out.push_str(&format!("  static int lower({object_name} value) {{\n"));
    out.push_str(&format!("    if (value is {impl_class_name}) {{\n"));
    out.push_str("      return value._handle;\n");
    out.push_str("    }\n");
    // Dart-implemented: register in handle map and return handle
    out.push_str(&format!("    _bindings().{init_done_field};\n"));
    out.push_str(&format!(
        "    return {bridge_name}.instance.register(value);\n"
    ));
    out.push_str("  }\n\n");
    out.push_str(&format!(
        "  static {object_name} lift(int handle) => {impl_class_name}._(_bindings(), handle);\n"
    ));
    out.push_str("}\n");

    out
}

/// Render a standard (non-trait) object.
fn render_plain_object(
    object: &UdlObject,
    callback_interfaces: &[UdlCallbackInterface],
    ffi_class_name: &str,
    api_overrides: &ApiOverrides,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let object_name = api_overrides
        .renamed_or_default(&ApiOverrides::object_key(&object.name), || {
            to_upper_camel(&object.name)
        });
    let object_lower = safe_dart_identifier(&to_lower_camel(&object.name));
    let free_field = format!("_{}Free", object_lower);
    let token_name = format!("_{}FinalizerToken", object_name);
    let mut out = String::new();
    out.push('\n');
    out.push_str(&format!(
        "final class {token_name} {{\n  const {token_name}(this.free, this.handle);\n  final void Function(int) free;\n  final int handle;\n}}\n\n"
    ));
    out.push_str(&render_doc_comment(object.docstring.as_deref(), ""));
    let mut implements = Vec::new();
    if object.is_error {
        implements.push("Exception".to_string());
    }
    if object.trait_methods.ord_cmp.is_some() {
        implements.push(format!("Comparable<{object_name}>"));
    }
    if implements.is_empty() {
        out.push_str(&format!("final class {object_name} {{\n"));
    } else {
        out.push_str(&format!(
            "final class {object_name} implements {} {{\n",
            implements.join(", ")
        ));
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
            emit_constructor_skip_warning(&mut out, &object_name, &ctor.name, &ctor.args, "  ");
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
        if api_overrides.excluded(&ApiOverrides::object_member_key(&object.name, &method.name)) {
            continue;
        }
        let has_callback_args =
            has_runtime_callback_args_in_args(&method.args, callback_interfaces, records, enums);
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
            emit_method_skip_warning(&mut out, &object_name, &method.name, &method.args, "  ");
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

    out
}

/// Render the VTable struct for a trait interface.
fn render_trait_vtable_struct(
    object: &UdlObject,
    vtable_struct_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "final class {vtable_struct_name} extends ffi.Struct {{\n"
    ));
    out.push_str(
        "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function(ffi.Uint64 handle)>> uniffiFree;\n\n",
    );
    out.push_str(
        "  external ffi.Pointer<ffi.NativeFunction<ffi.Uint64 Function(ffi.Uint64 handle)>> uniffiClone;\n\n",
    );
    for method in &object.methods {
        if is_uniffi_trait_method_name(&method.name) {
            continue;
        }
        let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
        let mut ffi_args = vec!["ffi.Uint64 handle".to_string()];
        for arg in &method.args {
            let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
            let arg_native =
                map_runtime_native_ffi_type(&arg.type_, records, enums).unwrap_or("ffi.Uint64");
            ffi_args.push(format!("{arg_native} {arg_name}"));
        }
        if let Some(return_type) = method.return_type.as_ref() {
            let out_type =
                map_runtime_native_ffi_type(return_type, records, enums).unwrap_or("ffi.Uint64");
            ffi_args.push(format!("ffi.Pointer<{out_type}> outReturn"));
        } else {
            ffi_args.push("ffi.Pointer<ffi.Void> outReturn".to_string());
        }
        ffi_args.push("ffi.Pointer<_RustCallStatus> outStatus".to_string());
        out.push_str(&format!(
            "  external ffi.Pointer<ffi.NativeFunction<ffi.Void Function({})>> {method_name};\n\n",
            ffi_args.join(", ")
        ));
    }
    out.push_str("}\n\n");
    out
}

/// Render the callback bridge class for a trait interface.
fn render_trait_callback_bridge(
    object: &UdlObject,
    object_name: &str,
    bridge_name: &str,
    vtable_struct_name: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("final class {bridge_name} {{\n"));
    out.push_str(&format!("  {bridge_name}._();\n"));
    out.push_str(&format!(
        "  static final {bridge_name} instance = {bridge_name}._();\n\n"
    ));
    out.push_str(&format!(
        "  final Map<int, {object_name}> _callbacks = <int, {object_name}>{{}};\n"
    ));
    out.push_str("  final Map<int, int> _refCounts = <int, int>{};\n");
    out.push_str("  int _nextHandle = 1;\n\n");

    // register
    out.push_str(&format!("  int register({object_name} callback) {{\n"));
    out.push_str("    final int handle = _nextHandle;\n");
    out.push_str("    _nextHandle += 2;\n");
    out.push_str("    _callbacks[handle] = callback;\n");
    out.push_str("    _refCounts[handle] = 1;\n");
    out.push_str("    return handle;\n");
    out.push_str("  }\n\n");

    // release
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

    // cloneHandle
    out.push_str("  int cloneHandle(int handle) {\n");
    out.push_str("    final int? refs = _refCounts[handle];\n");
    out.push_str("    if (refs == null) {\n");
    out.push_str("      throw StateError('Invalid callback handle: $handle');\n");
    out.push_str("    }\n");
    out.push_str("    _refCounts[handle] = refs + 1;\n");
    out.push_str("    return handle;\n");
    out.push_str("  }\n\n");

    // lookup
    out.push_str(&format!(
        "  {object_name}? lookup(int handle) => _callbacks[handle];\n\n"
    ));

    // free native callable
    out.push_str(
        "  static final ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)> _freeNative = ffi.NativeCallable<ffi.Void Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
    );
    out.push_str("    instance.release(handle);\n");
    out.push_str("  });\n\n");

    // clone native callable
    out.push_str(
        "  static final ffi.NativeCallable<ffi.Uint64 Function(ffi.Uint64 handle)> _cloneNative = ffi.NativeCallable<ffi.Uint64 Function(ffi.Uint64 handle)>.isolateLocal((int handle) {\n",
    );
    out.push_str("    return instance.cloneHandle(handle);\n");
    out.push_str("  }, exceptionalReturn: 0);\n\n");

    // Per-method native callables
    for method in &object.methods {
        if is_uniffi_trait_method_name(&method.name) {
            continue;
        }
        let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
        let native_callable_name = format!("_{}Native", method_name);

        let mut ffi_args = vec!["ffi.Uint64 handle".to_string()];
        let mut dart_args = vec!["int handle".to_string()];
        let mut callback_args = Vec::new();

        for arg in &method.args {
            let arg_name = safe_dart_identifier(&to_lower_camel(&arg.name));
            let arg_native =
                map_runtime_native_ffi_type(&arg.type_, records, enums).unwrap_or("ffi.Uint64");
            let arg_dart = map_runtime_dart_ffi_type(&arg.type_, records, enums).unwrap_or("int");
            ffi_args.push(format!("{arg_native} {arg_name}"));
            dart_args.push(format!("{arg_dart} {arg_name}"));
            callback_args.push(render_callback_arg_decode_expr(
                &arg.type_, &arg_name, records, enums,
            ));
        }

        if let Some(return_type) = method.return_type.as_ref() {
            let out_type =
                map_runtime_native_ffi_type(return_type, records, enums).unwrap_or("ffi.Uint64");
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
            "    final {object_name}? callback = instance.lookup(handle);\n"
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
            let encoded = render_callback_return_encode_expr(return_type, "result", records, enums);
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
        out.push_str("      outStatus.ref\n");
        out.push_str("        ..code = _rustCallStatusUnexpectedError\n");
        out.push_str("        ..errorBuf = err.toString().toNativeUtf8();\n");
        out.push_str("    }\n");
        out.push_str("  });\n\n");
    }

    // createVTable
    out.push_str(&format!(
        "  static ffi.Pointer<{vtable_struct_name}> createVTable() {{\n"
    ));
    out.push_str(&format!(
        "    final ffi.Pointer<{vtable_struct_name}> vtablePtr = calloc<{vtable_struct_name}>();\n"
    ));
    out.push_str("    vtablePtr.ref\n");
    out.push_str("      ..uniffiFree = _freeNative.nativeFunction\n");
    out.push_str("      ..uniffiClone = _cloneNative.nativeFunction\n");
    for method in &object.methods {
        if is_uniffi_trait_method_name(&method.name) {
            continue;
        }
        let method_name = safe_dart_identifier(&to_lower_camel(&method.name));
        out.push_str(&format!(
            "      ..{method_name} = _{method_name}Native.nativeFunction\n"
        ));
    }
    out.push_str("    ;\n");
    out.push_str("    return vtablePtr;\n");
    out.push_str("  }\n");
    out.push_str("}\n\n");

    out
}

/// Naming helpers for trait callback interfaces.
pub(super) fn trait_callback_bridge_class_name(object_name: &str) -> String {
    format!("_{}TraitCallbackBridge", to_upper_camel(object_name))
}

pub(super) fn trait_callback_vtable_struct_name(object_name: &str) -> String {
    format!("_{}TraitVTable", to_upper_camel(object_name))
}

pub(super) fn trait_callback_init_symbol(object_name: &str) -> String {
    format!("{}_trait_callback_init", object_name.to_ascii_lowercase())
}

pub(super) fn trait_callback_init_field_name(object_name: &str) -> String {
    safe_dart_identifier(&format!(
        "_{}TraitCallbackInit",
        to_lower_camel(object_name)
    ))
}

pub(super) fn trait_callback_init_done_field_name(object_name: &str) -> String {
    safe_dart_identifier(&format!(
        "_{}TraitCallbackInitDone",
        to_lower_camel(object_name)
    ))
}

pub(super) fn trait_callback_vtable_field_name(object_name: &str) -> String {
    safe_dart_identifier(&format!(
        "_{}TraitCallbackVTable",
        to_lower_camel(object_name)
    ))
}
