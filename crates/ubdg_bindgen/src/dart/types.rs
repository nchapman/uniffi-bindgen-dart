use std::collections::{HashMap, HashSet};

use uniffi_bindgen::interface::{ffi::FfiType, DefaultValue, Type};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlFunction {
    pub(super) name: String,
    pub(super) ffi_symbol: Option<String>,
    pub(super) ffi_arg_types: Vec<FfiType>,
    pub(super) ffi_return_type: Option<FfiType>,
    pub(super) ffi_has_rust_call_status: bool,
    pub(super) runtime_unsupported: Option<String>,
    pub(super) docstring: Option<String>,
    pub(super) is_async: bool,
    pub(super) return_type: Option<Type>,
    pub(super) throws_type: Option<Type>,
    pub(super) args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlArg {
    pub(super) name: String,
    pub(super) type_: Type,
    pub(super) docstring: Option<String>,
    pub(super) default: Option<DefaultValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlObject {
    pub(super) name: String,
    pub(super) docstring: Option<String>,
    pub(super) constructors: Vec<UdlObjectConstructor>,
    pub(super) methods: Vec<UdlObjectMethod>,
    pub(super) trait_methods: UdlObjectTraitMethods,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlObjectConstructor {
    pub(super) name: String,
    pub(super) ffi_symbol: Option<String>,
    pub(super) ffi_arg_types: Vec<FfiType>,
    pub(super) ffi_return_type: Option<FfiType>,
    pub(super) ffi_has_rust_call_status: bool,
    pub(super) runtime_unsupported: Option<String>,
    pub(super) docstring: Option<String>,
    pub(super) is_async: bool,
    pub(super) args: Vec<UdlArg>,
    pub(super) throws_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlObjectMethod {
    pub(super) name: String,
    pub(super) ffi_symbol: Option<String>,
    pub(super) ffi_arg_types: Vec<FfiType>,
    pub(super) ffi_return_type: Option<FfiType>,
    pub(super) ffi_has_rust_call_status: bool,
    pub(super) runtime_unsupported: Option<String>,
    pub(super) docstring: Option<String>,
    pub(super) is_async: bool,
    pub(super) return_type: Option<Type>,
    pub(super) throws_type: Option<Type>,
    pub(super) args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct UdlObjectTraitMethods {
    pub(super) display: Option<String>,
    pub(super) debug: Option<String>,
    pub(super) hash: Option<String>,
    pub(super) eq: Option<String>,
    pub(super) ne: Option<String>,
    pub(super) ord_cmp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlCallbackInterface {
    pub(super) name: String,
    pub(super) docstring: Option<String>,
    pub(super) methods: Vec<UdlCallbackMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlCallbackMethod {
    pub(super) name: String,
    pub(super) docstring: Option<String>,
    pub(super) is_async: bool,
    pub(super) return_type: Option<Type>,
    pub(super) throws_type: Option<Type>,
    pub(super) args: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlRecord {
    pub(super) name: String,
    pub(super) docstring: Option<String>,
    pub(super) fields: Vec<UdlArg>,
    pub(super) methods: Vec<UdlObjectMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlEnum {
    pub(super) name: String,
    pub(super) docstring: Option<String>,
    pub(super) is_error: bool,
    pub(super) variants: Vec<UdlEnumVariant>,
    pub(super) methods: Vec<UdlObjectMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlEnumVariant {
    pub(super) name: String,
    pub(super) docstring: Option<String>,
    pub(super) fields: Vec<UdlArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UdlApiChecksum {
    pub(super) symbol: String,
    pub(super) expected: u16,
}

impl UdlFunction {
    pub(super) fn uses_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(uniffi_type_uses_bytes)
            || self.args.iter().any(|a| uniffi_type_uses_bytes(&a.type_))
    }

    pub(super) fn uses_runtime_string(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_string_like_type)
            || self
                .args
                .iter()
                .any(|a| is_runtime_string_like_type(&a.type_))
    }

    pub(super) fn returns_runtime_string(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_string_like_type)
    }

    pub(super) fn uses_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_like_type)
            || self
                .args
                .iter()
                .any(|a| is_runtime_bytes_like_type(&a.type_))
    }

    pub(super) fn returns_runtime_bytes(&self) -> bool {
        self.return_type
            .as_ref()
            .is_some_and(is_runtime_bytes_like_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct UdlMetadata {
    pub(super) namespace: Option<String>,
    pub(super) local_module_path: String,
    pub(super) namespace_docstring: Option<String>,
    pub(super) uniffi_contract_version: Option<u32>,
    pub(super) ffi_uniffi_contract_version_symbol: Option<String>,
    pub(super) api_checksums: Vec<UdlApiChecksum>,
    pub(super) functions: Vec<UdlFunction>,
    pub(super) objects: Vec<UdlObject>,
    pub(super) callback_interfaces: Vec<UdlCallbackInterface>,
    pub(super) records: Vec<UdlRecord>,
    pub(super) enums: Vec<UdlEnum>,
}

pub(super) struct ApiOverrides {
    pub(super) rename: HashMap<String, String>,
    pub(super) exclude: HashSet<String>,
}

impl ApiOverrides {
    pub(super) fn new(rename: &HashMap<String, String>, exclude: &[String]) -> Self {
        Self {
            rename: rename.clone(),
            exclude: exclude.iter().cloned().collect(),
        }
    }

    pub(super) fn fn_key(name: &str) -> String {
        name.to_string()
    }

    pub(super) fn object_key(object: &str) -> String {
        object.to_string()
    }

    pub(super) fn object_member_key(object: &str, member: &str) -> String {
        format!("{object}.{member}")
    }

    pub(super) fn renamed_or_default(&self, key: &str, default: impl FnOnce() -> String) -> String {
        self.rename.get(key).cloned().unwrap_or_else(default)
    }

    pub(super) fn excluded(&self, key: &str) -> bool {
        self.exclude.contains(key)
    }
}
