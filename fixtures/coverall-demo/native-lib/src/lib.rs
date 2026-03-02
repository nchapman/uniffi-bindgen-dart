use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static NUM_ALIVE: AtomicU64 = AtomicU64::new(0);

/// Per-Coveralls instance state.
struct CoverallsState {
    name: String,
    strong_count: u64,
}

static COVERALLS: LazyLock<Mutex<HashMap<u64, CoverallsState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Per-Patch instance state.
static PATCHES: LazyLock<Mutex<HashMap<u64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Per-FalliblePatch instance state.
static FALLIBLE_PATCHES: LazyLock<Mutex<HashMap<u64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Per-ThreadsafeCounter state.
static COUNTERS: LazyLock<Mutex<HashMap<u64, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Bytes buffer matching the Dart _RustBuffer layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustBuffer {
    pub data: *mut u8,
    pub len: u64,
}

// --- Utility ---

fn alloc_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn c_str(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_str().unwrap_or("").to_string() }
}

fn c_string_out(s: &str) -> *mut c_char {
    CString::new(s).unwrap().into_raw()
}

fn json_out(value: &Value) -> *mut c_char {
    c_string_out(&serde_json::to_string(value).unwrap())
}

fn ok_envelope<T: Into<Value>>(val: T) -> *mut c_char {
    let env = serde_json::json!({"ok": val.into()});
    json_out(&env)
}

fn err_envelope(err: &Value) -> *mut c_char {
    let env = serde_json::json!({"err": err});
    json_out(&env)
}

// --- Free functions ---

#[no_mangle]
pub extern "C" fn rust_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_bytes_free(buf: RustBuffer) {
    if !buf.data.is_null() && buf.len > 0 {
        unsafe {
            drop(Vec::from_raw_parts(buf.data, buf.len as usize, buf.len as usize));
        }
    }
}

#[no_mangle]
pub extern "C" fn get_num_alive() -> u64 {
    NUM_ALIVE.load(Ordering::Relaxed)
}

/// Create a SimpleDict with sample values.
#[no_mangle]
pub extern "C" fn create_some_dict() -> *mut c_char {
    let dict = serde_json::json!({
        "text": "hello",
        "maybeCount": 42,
        "flag": true,
        "color": "red",
        "tags": ["a", "b"],
        "counts": {"x": 1, "y": 2},
        "maybeText": "present",
        "maybePatch": null
    });
    json_out(&dict)
}

/// Create a SimpleDict whose optional fields are null.
#[no_mangle]
pub extern "C" fn create_none_dict() -> *mut c_char {
    let dict = serde_json::json!({
        "text": "none",
        "maybeCount": 0,
        "flag": false,
        "color": "blue",
        "tags": [],
        "counts": {},
        "maybeText": null,
        "maybePatch": null
    });
    json_out(&dict)
}

/// Return a MaybeSimpleDict based on a selector.
#[no_mangle]
pub extern "C" fn get_maybe_simple_dict(index: i8) -> *mut c_char {
    if index == 0 {
        let val = serde_json::json!({
            "tag": "nah"
        });
        json_out(&val)
    } else {
        let dict = serde_json::json!({
            "text": "from_index",
            "maybeCount": index as u64,
            "flag": true,
            "color": "green",
            "tags": ["tag"],
            "counts": {"n": index as u64},
            "maybeText": null,
            "maybePatch": null
        });
        let val = serde_json::json!({
            "tag": "yeah",
            "value": dict
        });
        json_out(&val)
    }
}

/// Round-trip a string; throws ComplexError on certain inputs.
#[no_mangle]
pub extern "C" fn println(value: *const c_char) -> *mut c_char {
    let s = c_str(value);
    if s == "os_error" {
        return err_envelope(&serde_json::json!({"tag": "osError", "code": 42, "extendedCode": 0}));
    }
    if s == "permission" {
        return err_envelope(&serde_json::json!({"tag": "permissionDenied", "reason": "nope"}));
    }
    if s == "unknown" {
        return err_envelope(&serde_json::json!({"tag": "unknownError"}));
    }
    ok_envelope(Value::String(s))
}

/// Divide by text-parsed divisor.
#[no_mangle]
pub extern "C" fn divide_by_text(value: f64, divisor: *const c_char) -> *mut c_char {
    let div_str = c_str(divisor);
    match div_str.parse::<f64>() {
        Ok(d) if d == 0.0 => {
            err_envelope(&serde_json::json!({"tag": "osError", "code": 1, "extendedCode": 0}))
        }
        Ok(d) => ok_envelope(Value::from(value / d)),
        Err(_) => {
            err_envelope(&serde_json::json!({"tag": "permissionDenied", "reason": "not a number"}))
        }
    }
}

/// Reverse bytes.
#[no_mangle]
pub extern "C" fn reverse_bytes(input: RustBuffer) -> RustBuffer {
    if input.data.is_null() || input.len == 0 {
        return RustBuffer {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    let src = unsafe { std::slice::from_raw_parts(input.data, input.len as usize) };
    let mut reversed: Vec<u8> = src.iter().rev().copied().collect();
    let ptr = reversed.as_mut_ptr();
    let len = reversed.len() as u64;
    std::mem::forget(reversed);
    RustBuffer { data: ptr, len }
}

/// Create a Patch with a given color.
#[no_mangle]
pub extern "C" fn create_patch(color: *const c_char) -> u64 {
    let color_str = c_str(color);
    let handle = alloc_handle();
    PATCHES.lock().unwrap().insert(handle, color_str);
    handle
}

// --- Patch object ---

#[no_mangle]
pub extern "C" fn patch_new(color: *const c_char) -> u64 {
    let color_str = c_str(color);
    let handle = alloc_handle();
    PATCHES.lock().unwrap().insert(handle, color_str);
    handle
}

#[no_mangle]
pub extern "C" fn patch_free(handle: u64) {
    PATCHES.lock().unwrap().remove(&handle);
}

#[no_mangle]
pub extern "C" fn patch_get_color(handle: u64) -> *mut c_char {
    let patches = PATCHES.lock().unwrap();
    let color = patches.get(&handle).cloned().unwrap_or_else(|| "red".to_string());
    c_string_out(&color)
}

// --- FalliblePatch object ---

#[no_mangle]
pub extern "C" fn falliblepatch_new(color: *const c_char, should_fail: bool) -> *mut c_char {
    if should_fail {
        return err_envelope(&serde_json::json!({"tag": "tooManyHoles"}));
    }
    let color_str = c_str(color);
    let handle = alloc_handle();
    FALLIBLE_PATCHES.lock().unwrap().insert(handle, color_str);
    ok_envelope(Value::from(handle))
}

#[no_mangle]
pub extern "C" fn falliblepatch_free(handle: u64) {
    FALLIBLE_PATCHES.lock().unwrap().remove(&handle);
}

#[no_mangle]
pub extern "C" fn falliblepatch_get_color(handle: u64) -> *mut c_char {
    let patches = FALLIBLE_PATCHES.lock().unwrap();
    let color = patches.get(&handle).cloned().unwrap_or_else(|| "red".to_string());
    c_string_out(&color)
}

// --- Coveralls object ---

#[no_mangle]
pub extern "C" fn coveralls_new(name: *const c_char) -> u64 {
    let n = c_str(name);
    let handle = alloc_handle();
    COVERALLS.lock().unwrap().insert(
        handle,
        CoverallsState {
            name: n,
            strong_count: 1,
        },
    );
    NUM_ALIVE.fetch_add(1, Ordering::Relaxed);
    handle
}

#[no_mangle]
pub extern "C" fn coveralls_fallible_new(name: *const c_char, should_fail: bool) -> *mut c_char {
    if should_fail {
        return err_envelope(&serde_json::json!({"tag": "tooManyHoles"}));
    }
    let n = c_str(name);
    let handle = alloc_handle();
    COVERALLS.lock().unwrap().insert(
        handle,
        CoverallsState {
            name: n,
            strong_count: 1,
        },
    );
    NUM_ALIVE.fetch_add(1, Ordering::Relaxed);
    ok_envelope(Value::from(handle))
}

#[no_mangle]
pub extern "C" fn coveralls_free(handle: u64) {
    if COVERALLS.lock().unwrap().remove(&handle).is_some() {
        NUM_ALIVE.fetch_sub(1, Ordering::Relaxed);
    }
}

#[no_mangle]
pub extern "C" fn coveralls_get_name(handle: u64) -> *mut c_char {
    let state = COVERALLS.lock().unwrap();
    let name = state
        .get(&handle)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    c_string_out(&name)
}

#[no_mangle]
pub extern "C" fn coveralls_set_name(handle: u64, name: *const c_char) {
    let n = c_str(name);
    if let Some(s) = COVERALLS.lock().unwrap().get_mut(&handle) {
        s.name = n;
    }
}

#[no_mangle]
pub extern "C" fn coveralls_strong_count(handle: u64) -> u64 {
    let state = COVERALLS.lock().unwrap();
    state.get(&handle).map(|s| s.strong_count).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn coveralls_clone_me(handle: u64) -> u64 {
    let state = COVERALLS.lock().unwrap();
    let name = state
        .get(&handle)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    drop(state);

    let new_handle = alloc_handle();
    COVERALLS.lock().unwrap().insert(
        new_handle,
        CoverallsState {
            name,
            strong_count: 1,
        },
    );
    NUM_ALIVE.fetch_add(1, Ordering::Relaxed);
    new_handle
}

#[no_mangle]
pub extern "C" fn coveralls_maybe_throw(handle: u64, should_throw: bool) -> *mut c_char {
    let _state = COVERALLS.lock().unwrap();
    if !_state.contains_key(&handle) {
        return err_envelope(&serde_json::json!({"tag": "tooManyHoles"}));
    }
    if should_throw {
        return err_envelope(&serde_json::json!({"tag": "tooManyHoles"}));
    }
    ok_envelope(Value::Bool(true))
}

#[no_mangle]
pub extern "C" fn coveralls_maybe_throw_complex(handle: u64, selector: i8) -> *mut c_char {
    let _state = COVERALLS.lock().unwrap();
    if !_state.contains_key(&handle) {
        return err_envelope(&serde_json::json!({"tag": "unknownError"}));
    }
    match selector {
        0 => ok_envelope(Value::Bool(true)),
        1 => err_envelope(&serde_json::json!({"tag": "osError", "code": 10, "extendedCode": 20})),
        2 => err_envelope(&serde_json::json!({"tag": "permissionDenied", "reason": "access denied"})),
        _ => err_envelope(&serde_json::json!({"tag": "unknownError"})),
    }
}

#[no_mangle]
pub extern "C" fn coveralls_reverse_bytes(handle: u64, input: RustBuffer) -> RustBuffer {
    let _state = COVERALLS.lock().unwrap();
    if !_state.contains_key(&handle) || input.data.is_null() || input.len == 0 {
        return RustBuffer {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    drop(_state);
    reverse_bytes(input)
}

#[no_mangle]
pub extern "C" fn coveralls_get_metadata(handle: u64) -> *mut c_char {
    let state = COVERALLS.lock().unwrap();
    let name = state
        .get(&handle)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let metadata = serde_json::json!({
        "name": name,
        "version": null
    });
    json_out(&metadata)
}

/// Return a map with non-string keys via RustBuffer binary encoding.
#[no_mangle]
pub extern "C" fn get_int_map(key: u32, value: u64) -> RustBuffer {
    // Binary format: i32 length, then for each entry: u32 key, u64 value
    // Big-endian, matching the UniFFI binary codec protocol.
    let mut buf = Vec::new();
    // Map length (1 entry)
    buf.extend_from_slice(&1i32.to_be_bytes());
    // Key (u32)
    buf.extend_from_slice(&key.to_be_bytes());
    // Value (u64)
    buf.extend_from_slice(&value.to_be_bytes());

    let len = buf.len() as u64;
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    RustBuffer { data: ptr, len }
}

/// Return an optional u32 — JSON-encoded as "42" or "null".
#[no_mangle]
pub extern "C" fn get_maybe_count(return_value: bool) -> *mut c_char {
    if return_value {
        c_string_out("42")
    } else {
        c_string_out("null")
    }
}

/// Return an optional boolean — JSON-encoded as "true" or "null".
#[no_mangle]
pub extern "C" fn get_maybe_flag(return_value: bool) -> *mut c_char {
    if return_value {
        c_string_out("true")
    } else {
        c_string_out("null")
    }
}

/// Return an optional SimpleDict — JSON string or nullptr.
#[no_mangle]
pub extern "C" fn get_maybe_dict(return_value: bool) -> *mut c_char {
    if return_value {
        let dict = serde_json::json!({
            "text": "hello",
            "maybeCount": 42,
            "flag": true,
            "color": "red",
            "tags": ["a"],
            "counts": {"x": 1},
            "maybeText": null,
            "maybePatch": null
        });
        json_out(&dict)
    } else {
        std::ptr::null_mut()
    }
}

/// Describe an optional SimpleDict parameter.
#[no_mangle]
pub extern "C" fn describe_maybe_dict(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        c_string_out("null")
    } else {
        let s = c_str(input);
        c_string_out(&format!("dict:{}", s))
    }
}

/// Return an optional Color enum — JSON string or nullptr.
#[no_mangle]
pub extern "C" fn get_maybe_color(return_value: bool) -> *mut c_char {
    if return_value {
        c_string_out("\"red\"")
    } else {
        std::ptr::null_mut()
    }
}

/// Describe an optional Color enum parameter.
#[no_mangle]
pub extern "C" fn describe_maybe_color(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        c_string_out("null")
    } else {
        let s = c_str(input);
        c_string_out(&format!("color:{}", s))
    }
}

// --- Coveralls optional-object methods ---

/// Per-handle "other" reference.
static OTHER_REFS: LazyLock<Mutex<HashMap<u64, Option<u64>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[no_mangle]
pub extern "C" fn coveralls_take_other(handle: u64, other: u64) {
    let other_val = if other == 0 { None } else { Some(other) };
    OTHER_REFS.lock().unwrap().insert(handle, other_val);
}

#[no_mangle]
pub extern "C" fn coveralls_get_other(handle: u64) -> u64 {
    OTHER_REFS
        .lock()
        .unwrap()
        .get(&handle)
        .and_then(|o| *o)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn coveralls_get_tags(handle: u64) -> *mut c_char {
    let state = COVERALLS.lock().unwrap();
    let name = state
        .get(&handle)
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let tags = serde_json::json!([name, null, "tag"]);
    json_out(&tags)
}

// --- ThreadsafeCounter ---

#[no_mangle]
pub extern "C" fn threadsafecounter_new() -> u64 {
    let handle = alloc_handle();
    COUNTERS.lock().unwrap().insert(handle, 0);
    handle
}

#[no_mangle]
pub extern "C" fn threadsafecounter_free(handle: u64) {
    COUNTERS.lock().unwrap().remove(&handle);
}

#[no_mangle]
pub extern "C" fn threadsafecounter_increment(handle: u64) {
    if let Some(count) = COUNTERS.lock().unwrap().get_mut(&handle) {
        *count += 1;
    }
}

#[no_mangle]
pub extern "C" fn threadsafecounter_get_count(handle: u64) -> u64 {
    COUNTERS.lock().unwrap().get(&handle).copied().unwrap_or(0)
}
