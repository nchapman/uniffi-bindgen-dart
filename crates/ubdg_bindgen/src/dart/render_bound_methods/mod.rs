use super::config::CustomTypeConfig;
use super::types::ApiOverrides;
use super::*;
use std::collections::HashMap;

mod context;
mod functions;
mod objects;
mod setup;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_bound_methods(
    functions: &[UdlFunction],
    objects: &[UdlObject],
    callback_interfaces: &[UdlCallbackInterface],
    ffi_namespace: &str,
    local_module_path: &str,
    records: &[UdlRecord],
    enums: &[UdlEnum],
    custom_types: &HashMap<String, CustomTypeConfig>,
    api_overrides: &ApiOverrides,
) -> String {
    let mut out = String::new();
    let runtime_functions =
        setup::merge_record_enum_methods(functions, records, enums, local_module_path);
    let has_runtime_ffibuffer_fallback =
        setup::has_runtime_ffibuffer_fallback(&runtime_functions, objects);
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
    let needs_string_free = setup::needs_runtime_string_free(
        functions,
        objects,
        callback_interfaces,
        records,
        enums,
        needs_async_rust_future,
    );
    let needs_bytes_free = setup::needs_runtime_bytes_free(functions, objects, records, enums);
    let needs_bytes_vec_free =
        setup::needs_runtime_bytes_vec_free(functions, objects, records, enums);

    setup::render_ffi_lookup_fields(
        &mut out,
        ffi_namespace,
        objects,
        &callback_runtime_interfaces,
        needs_string_free,
        needs_bytes_free,
        needs_bytes_vec_free,
        has_runtime_ffibuffer_fallback,
    );

    let ctx = context::RenderMethodContext {
        ffi_namespace,
        local_module_path,
        objects,
        records,
        enums,
        callback_interfaces,
        custom_types,
        api_overrides,
    };

    functions::render_toplevel_functions(&mut out, &runtime_functions, &ctx);

    objects::render_object_members(&mut out, &ctx);

    out
}
