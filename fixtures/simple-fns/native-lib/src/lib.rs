use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

static TICK_COUNT: AtomicU32 = AtomicU32::new(0);
static FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static BYTES_FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static BYTES_VEC_FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static NEXT_COUNTER_HANDLE: AtomicU32 = AtomicU32::new(1);
static NEXT_STRING_FUTURE_HANDLE: AtomicU64 = AtomicU64::new(1);
static COUNTERS: LazyLock<Mutex<HashMap<u64, i32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static COUNTER_LABELS: LazyLock<Mutex<HashMap<u64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static STRING_FUTURES: LazyLock<Mutex<HashMap<u64, StringFutureState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const RUST_CALL_STATUS_SUCCESS: i8 = 0;
const RUST_CALL_STATUS_UNEXPECTED_ERROR: i8 = 2;
const RUST_CALL_STATUS_CANCELLED: i8 = 3;
const RUST_FUTURE_POLL_READY: i8 = 0;
const RUST_FUTURE_POLL_WAKE: i8 = 1;

#[repr(C)]
pub struct RustCallStatus {
    pub code: i8,
    pub error_buf: *mut c_char,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StringFuturePollState {
    PendingWake,
    Ready,
}

#[derive(Clone, Debug)]
struct StringFutureState {
    poll_state: StringFuturePollState,
    cancelled: bool,
    result: String,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustBuffer {
    pub data: *mut u8,
    pub len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustBufferOpt {
    pub is_some: u8,
    pub value: RustBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustBufferVec {
    pub data: *mut RustBuffer,
    pub len: u64,
}

fn vec_into_rust_buffer(mut data: Vec<u8>) -> RustBuffer {
    if data.is_empty() {
        return RustBuffer {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }

    let out = RustBuffer {
        data: data.as_mut_ptr(),
        len: data.len() as u64,
    };
    std::mem::forget(data);
    out
}

fn rust_buffer_to_vec(buf: RustBuffer) -> Vec<u8> {
    if buf.data.is_null() || buf.len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(buf.data, buf.len as usize).to_vec() }
    }
}

fn vec_into_rust_buffer_vec(mut items: Vec<RustBuffer>) -> RustBufferVec {
    if items.is_empty() {
        return RustBufferVec {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    let out = RustBufferVec {
        data: items.as_mut_ptr(),
        len: items.len() as u64,
    };
    std::mem::forget(items);
    out
}

fn enqueue_string_future(result: String) -> u64 {
    let handle = NEXT_STRING_FUTURE_HANDLE.fetch_add(1, Ordering::Relaxed);
    STRING_FUTURES.lock().expect("string futures lock").insert(
        handle,
        StringFutureState {
            poll_state: StringFuturePollState::PendingWake,
            cancelled: false,
            result,
        },
    );
    handle
}

fn write_out_status(
    out_status: *mut RustCallStatus,
    code: i8,
    error_buf: *mut c_char,
) -> *mut RustCallStatus {
    if out_status.is_null() {
        return out_status;
    }
    unsafe {
        (*out_status).code = code;
        (*out_status).error_buf = error_buf;
    }
    out_status
}

#[unsafe(no_mangle)]
pub extern "C" fn add(left: u32, right: u32) -> u32 {
    left + right
}

#[unsafe(no_mangle)]
pub extern "C" fn echo_person(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let payload = unsafe { CStr::from_ptr(input) }
        .to_string_lossy()
        .into_owned();
    CString::new(payload).expect("valid CString").into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn add_seconds(when: i64, seconds: i64) -> i64 {
    when + (seconds * 1_000_000)
}

#[unsafe(no_mangle)]
pub extern "C" fn add_u64(left: u64, right: u64) -> u64 {
    left + right
}

#[unsafe(no_mangle)]
pub extern "C" fn bytes_echo(input: RustBuffer) -> RustBuffer {
    let out = rust_buffer_to_vec(input);
    vec_into_rust_buffer(out)
}

#[unsafe(no_mangle)]
pub extern "C" fn bytes_maybe_echo(input: RustBufferOpt) -> RustBufferOpt {
    if input.is_some == 0 {
        return RustBufferOpt {
            is_some: 0,
            value: RustBuffer {
                data: std::ptr::null_mut(),
                len: 0,
            },
        };
    }
    RustBufferOpt {
        is_some: 1,
        value: vec_into_rust_buffer(rust_buffer_to_vec(input.value)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bytes_chunks_echo(input: RustBufferVec) -> RustBufferVec {
    if input.data.is_null() || input.len == 0 {
        return RustBufferVec {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    let in_items = unsafe { std::slice::from_raw_parts(input.data, input.len as usize) };
    let out_items = in_items
        .iter()
        .copied()
        .map(rust_buffer_to_vec)
        .map(vec_into_rust_buffer)
        .collect::<Vec<_>>();
    vec_into_rust_buffer_vec(out_items)
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_bytes_free(value: RustBuffer) {
    if value.data.is_null() {
        return;
    }
    BYTES_FREE_COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe {
        let _ = Vec::from_raw_parts(value.data, value.len as usize, value.len as usize);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_bytes_vec_free(value: RustBufferVec) {
    if value.data.is_null() {
        return;
    }
    BYTES_VEC_FREE_COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe {
        let _ = Vec::from_raw_parts(value.data, value.len as usize, value.len as usize);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_bytes_free_count() {
    BYTES_FREE_COUNT.store(0, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn bytes_free_count() -> u32 {
    BYTES_FREE_COUNT.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_bytes_vec_free_count() {
    BYTES_VEC_FREE_COUNT.store(0, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn bytes_vec_free_count() -> u32 {
    BYTES_VEC_FREE_COUNT.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn cycle_color(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let value = unsafe { CStr::from_ptr(input) }.to_string_lossy();
    let next = match value.as_ref() {
        "red" => "green",
        "green" => "blue",
        "blue" => "red",
        _ => return std::ptr::null_mut(),
    };
    CString::new(next).expect("valid CString").into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn evolve_outcome(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let payload = unsafe { CStr::from_ptr(input) }
        .to_string_lossy()
        .into_owned();
    let Ok(value) = serde_json::from_str::<Value>(&payload) else {
        return std::ptr::null_mut();
    };
    let Some(tag) = value.get("tag").and_then(Value::as_str) else {
        return std::ptr::null_mut();
    };

    let out = match tag {
        "success" => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            serde_json::json!({
                "tag": "failure",
                "code": message.len() as i64,
                "reason": message
            })
        }
        "failure" => {
            let code = value
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let reason = value
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            serde_json::json!({
                "tag": "success",
                "message": format!("{code}:{reason}")
            })
        }
        _ => return std::ptr::null_mut(),
    };

    CString::new(out.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn greet(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        return CString::new("hello, <null>")
            .expect("valid CString")
            .into_raw();
    }

    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    CString::new(format!("hello, {name}"))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn async_greet(name: *const c_char) -> u64 {
    let message = if name.is_null() {
        "async, <null>".to_string()
    } else {
        let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
        format!("async, {name}")
    };
    enqueue_string_future(message)
}

#[unsafe(no_mangle)]
pub extern "C" fn broken_greet() -> *mut c_char {
    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn maybe_greet(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        return std::ptr::null_mut();
    }

    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    CString::new(format!("maybe, {name}"))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    FREE_COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe {
        let _ = CString::from_raw(value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_free_count() {
    FREE_COUNT.store(0, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn free_count() -> u32 {
    FREE_COUNT.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_string(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    let mut futures = STRING_FUTURES.lock().expect("string futures lock");
    let Some(state) = futures.get_mut(&handle) else {
        callback(callback_data, RUST_FUTURE_POLL_READY);
        return;
    };
    if state.cancelled {
        callback(callback_data, RUST_FUTURE_POLL_READY);
        return;
    }

    match state.poll_state {
        StringFuturePollState::PendingWake => {
            state.poll_state = StringFuturePollState::Ready;
            callback(callback_data, RUST_FUTURE_POLL_WAKE);
        }
        StringFuturePollState::Ready => {
            callback(callback_data, RUST_FUTURE_POLL_READY);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_string(handle: u64) {
    if let Some(state) = STRING_FUTURES
        .lock()
        .expect("string futures lock")
        .get_mut(&handle)
    {
        state.cancelled = true;
        state.poll_state = StringFuturePollState::Ready;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_string(
    handle: u64,
    out_status: *mut RustCallStatus,
) -> *mut c_char {
    let futures = STRING_FUTURES.lock().expect("string futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing string future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return std::ptr::null_mut();
    };

    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return std::ptr::null_mut();
    }

    write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
    CString::new(state.result.clone())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_string(handle: u64) {
    STRING_FUTURES
        .lock()
        .expect("string futures lock")
        .remove(&handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn checked_divide(left: i32, right: i32) -> *mut c_char {
    let envelope = if right == 0 {
        serde_json::json!({
            "err": {
                "tag": "divisionByZero"
            }
        })
    } else if left < 0 || right < 0 {
        serde_json::json!({
            "err": {
                "tag": "negativeInput",
                "value": if left < 0 { left } else { right },
            }
        })
    } else {
        serde_json::json!({
            "ok": left / right
        })
    };

    CString::new(envelope.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_new(initial: u32) -> u64 {
    let handle = NEXT_COUNTER_HANDLE.fetch_add(1, Ordering::Relaxed) as u64;
    COUNTERS
        .lock()
        .expect("counter map lock")
        .insert(handle, initial as i32);
    COUNTER_LABELS
        .lock()
        .expect("counter labels lock")
        .insert(handle, String::new());
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_with_person(seed: *const c_char) -> u64 {
    let initial = if seed.is_null() {
        0
    } else {
        let payload = unsafe { CStr::from_ptr(seed) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<Value>(&payload)
            .ok()
            .and_then(|v| v.get("age").and_then(Value::as_u64))
            .unwrap_or_default() as u32
    };
    counter_new(initial)
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_free(handle: u64) {
    COUNTERS.lock().expect("counter map lock").remove(&handle);
    COUNTER_LABELS
        .lock()
        .expect("counter labels lock")
        .remove(&handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_add_value(handle: u64, amount: u32) {
    if let Some(value) = COUNTERS.lock().expect("counter map lock").get_mut(&handle) {
        *value += amount as i32;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_current_value(handle: u64) -> u32 {
    COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_set_label(handle: u64, label: *const c_char) {
    if label.is_null() {
        return;
    }
    let label = unsafe { CStr::from_ptr(label) }
        .to_string_lossy()
        .into_owned();
    COUNTER_LABELS
        .lock()
        .expect("counter labels lock")
        .insert(handle, label);
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_maybe_label(handle: u64, suffix: *const c_char) -> *mut c_char {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    let suffix = if suffix.is_null() {
        "none".to_string()
    } else {
        unsafe { CStr::from_ptr(suffix) }
            .to_string_lossy()
            .into_owned()
    };
    CString::new(format!("counter:{value}:{suffix}"))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_ingest_person(handle: u64, input: *const c_char) {
    if input.is_null() {
        return;
    }
    let payload = unsafe { CStr::from_ptr(input) }
        .to_string_lossy()
        .into_owned();
    let Ok(value) = serde_json::from_str::<Value>(&payload) else {
        return;
    };
    let age = value.get("age").and_then(Value::as_i64).unwrap_or_default() as i32;
    if let Some(existing) = COUNTERS.lock().expect("counter map lock").get_mut(&handle) {
        *existing = age;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_flip_outcome(_handle: u64, input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let payload = unsafe { CStr::from_ptr(input) }
        .to_string_lossy()
        .into_owned();
    let Ok(value) = serde_json::from_str::<Value>(&payload) else {
        return std::ptr::null_mut();
    };
    let Some(tag) = value.get("tag").and_then(Value::as_str) else {
        return std::ptr::null_mut();
    };
    let out = match tag {
        "success" => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            serde_json::json!({
                "tag": "failure",
                "code": message.len() as i64,
                "reason": message
            })
        }
        "failure" => {
            let code = value
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let reason = value
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            serde_json::json!({
                "tag": "success",
                "message": format!("{code}:{reason}")
            })
        }
        _ => return std::ptr::null_mut(),
    };
    CString::new(out.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_bytes_len(_handle: u64, input: RustBuffer) -> u32 {
    rust_buffer_to_vec(input).len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_optional_bytes_len(_handle: u64, input: RustBufferOpt) -> u32 {
    if input.is_some == 0 {
        0
    } else {
        rust_buffer_to_vec(input.value).len() as u32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_chunks_total_len(_handle: u64, input: RustBufferVec) -> u32 {
    if input.data.is_null() || input.len == 0 {
        return 0;
    }
    let items = unsafe { std::slice::from_raw_parts(input.data, input.len as usize) };
    items
        .iter()
        .copied()
        .map(rust_buffer_to_vec)
        .map(|v| v.len() as u32)
        .sum()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_describe(handle: u64) -> *mut c_char {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    let label = COUNTER_LABELS
        .lock()
        .expect("counter labels lock")
        .get(&handle)
        .cloned()
        .unwrap_or_default();
    let text = if label.is_empty() {
        format!("counter:{value}")
    } else {
        format!("counter:{value}:{label}")
    };
    CString::new(text).expect("valid CString").into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_describe(handle: u64) -> u64 {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    let label = COUNTER_LABELS
        .lock()
        .expect("counter labels lock")
        .get(&handle)
        .cloned()
        .unwrap_or_default();
    let text = if label.is_empty() {
        format!("async:counter:{value}")
    } else {
        format!("async:counter:{value}:{label}")
    };
    enqueue_string_future(text)
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_snapshot_person(handle: u64) -> *mut c_char {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    let payload = serde_json::json!({
        "name": "counter",
        "age": value.max(0) as u32
    });
    CString::new(payload.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_snapshot_outcome(handle: u64) -> *mut c_char {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    let payload = if value % 2 == 0 {
        serde_json::json!({
            "tag": "success",
            "message": format!("even:{value}")
        })
    } else {
        serde_json::json!({
            "tag": "failure",
            "code": value,
            "reason": "odd"
        })
    };
    CString::new(payload.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_snapshot_bytes(handle: u64) -> RustBuffer {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    vec_into_rust_buffer(format!("bytes:{value}").into_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_divide_by(handle: u64, divisor: i32) -> *mut c_char {
    let mut counters = COUNTERS.lock().expect("counter map lock");
    let value = counters.get(&handle).copied().unwrap_or_default();
    let envelope = if divisor == 0 {
        serde_json::json!({
            "err": { "tag": "divisionByZero" }
        })
    } else if divisor < 0 {
        serde_json::json!({
            "err": { "tag": "negativeInput", "value": divisor }
        })
    } else {
        let quotient = value / divisor;
        counters.insert(handle, quotient);
        serde_json::json!({ "ok": quotient })
    };

    CString::new(envelope.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_risky_outcome(handle: u64, divisor: i32) -> *mut c_char {
    let mut counters = COUNTERS.lock().expect("counter map lock");
    let value = counters.get(&handle).copied().unwrap_or_default();
    let envelope = if divisor == 0 {
        serde_json::json!({
            "err": { "tag": "divisionByZero" }
        })
    } else if divisor < 0 {
        serde_json::json!({
            "err": { "tag": "negativeInput", "value": divisor }
        })
    } else {
        let quotient = value / divisor;
        counters.insert(handle, quotient);
        let outcome_json = serde_json::json!({
            "tag": "success",
            "message": format!("q:{quotient}")
        })
        .to_string();
        serde_json::json!({ "ok": outcome_json })
    };

    CString::new(envelope.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn negate(value: i32) -> i32 {
    -value
}

#[unsafe(no_mangle)]
pub extern "C" fn subtract_i64(left: i64, right: i64) -> i64 {
    left - right
}

#[unsafe(no_mangle)]
pub extern "C" fn multiply_duration(value: i64, factor: u32) -> i64 {
    value * factor as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn is_even(value: i32) -> bool {
    value % 2 == 0
}

#[unsafe(no_mangle)]
pub extern "C" fn scale32(value: f32, factor: f32) -> f32 {
    value * factor
}

#[unsafe(no_mangle)]
pub extern "C" fn scale(value: f64, factor: f64) -> f64 {
    value * factor
}

#[unsafe(no_mangle)]
pub extern "C" fn tick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn current_tick() -> u32 {
    TICK_COUNT.load(Ordering::Relaxed)
}
