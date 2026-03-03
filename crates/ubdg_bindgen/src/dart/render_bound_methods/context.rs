use std::collections::HashMap;

use super::super::config::CustomTypeConfig;
use super::super::*;

/// Shared context threaded through all render_bound_methods sub-functions.
///
/// Holds immutable references to the module-level data needed by the
/// function/constructor/method rendering loops, reducing parameter counts.
pub(super) struct RenderMethodContext<'a> {
    pub(super) ffi_namespace: &'a str,
    pub(super) local_module_path: &'a str,
    pub(super) objects: &'a [UdlObject],
    pub(super) records: &'a [UdlRecord],
    pub(super) enums: &'a [UdlEnum],
    pub(super) callback_interfaces: &'a [UdlCallbackInterface],
    pub(super) custom_types: &'a HashMap<String, CustomTypeConfig>,
}
