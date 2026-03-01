use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::hash::{Hash, Hasher};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

static TICK_COUNT: AtomicU32 = AtomicU32::new(0);
static FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static BYTES_FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static BYTES_VEC_FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static ASYNC_CANCEL_COUNT: AtomicU32 = AtomicU32::new(0);
static ASYNC_FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static NEXT_COUNTER_HANDLE: AtomicU32 = AtomicU32::new(1);
static NEXT_ASYNC_FUTURE_HANDLE: AtomicU64 = AtomicU64::new(1);
static COUNTERS: LazyLock<Mutex<HashMap<u64, i32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static COUNTER_LABELS: LazyLock<Mutex<HashMap<u64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ASYNC_FUTURES: LazyLock<Mutex<HashMap<u64, AsyncFutureState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ASYNC_ADDER_OFFSETS: LazyLock<Mutex<HashMap<u64, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ADDER_VTABLE: LazyLock<Mutex<Option<AdderVTable>>> = LazyLock::new(|| Mutex::new(None));
static FORMATTER_VTABLE: LazyLock<Mutex<Option<FormatterVTable>>> = LazyLock::new(|| Mutex::new(None));

const RUST_CALL_STATUS_SUCCESS: i8 = 0;
const RUST_CALL_STATUS_UNEXPECTED_ERROR: i8 = 2;
const RUST_CALL_STATUS_CANCELLED: i8 = 3;
const RUST_FUTURE_POLL_READY: i8 = 0;
const RUST_FUTURE_POLL_WAKE: i8 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustCallStatus {
    pub code: i8,
    pub error_buf: *mut c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ForeignFutureResultU32 {
    pub return_value: u32,
    pub call_status: RustCallStatus,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ForeignFutureResultString {
    pub return_value: *mut c_char,
    pub call_status: RustCallStatus,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ForeignFutureDroppedCallbackStruct {
    pub handle: u64,
    pub callback: Option<extern "C" fn(u64)>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AdderVTable {
    pub uniffi_free: extern "C" fn(u64),
    pub uniffi_clone: extern "C" fn(u64) -> u64,
    pub add: extern "C" fn(u64, u32, u32, *mut u32, *mut RustCallStatus),
    pub add_async: extern "C" fn(
        u64,
        u32,
        u32,
        extern "C" fn(u64, ForeignFutureResultU32),
        u64,
        *mut ForeignFutureDroppedCallbackStruct,
    ),
    pub checked_add: extern "C" fn(u64, u32, u32, *mut u32, *mut RustCallStatus),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FormatterVTable {
    pub uniffi_free: extern "C" fn(u64),
    pub uniffi_clone: extern "C" fn(u64) -> u64,
    pub format:
        extern "C" fn(u64, *const c_char, *const c_char, *const c_char, *mut *mut c_char, *mut RustCallStatus),
    pub format_async: extern "C" fn(
        u64,
        *const c_char,
        *const c_char,
        *const c_char,
        extern "C" fn(u64, ForeignFutureResultString),
        u64,
        *mut ForeignFutureDroppedCallbackStruct,
    ),
    pub format_async_optional: extern "C" fn(
        u64,
        *const c_char,
        *const c_char,
        *const c_char,
        extern "C" fn(u64, ForeignFutureResultString),
        u64,
        *mut ForeignFutureDroppedCallbackStruct,
    ),
    pub format_async_person: extern "C" fn(
        u64,
        *const c_char,
        *const c_char,
        *const c_char,
        extern "C" fn(u64, ForeignFutureResultString),
        u64,
        *mut ForeignFutureDroppedCallbackStruct,
    ),
    pub format_async_outcome: extern "C" fn(
        u64,
        *const c_char,
        *const c_char,
        *const c_char,
        extern "C" fn(u64, ForeignFutureResultString),
        u64,
        *mut ForeignFutureDroppedCallbackStruct,
    ),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AsyncFuturePollState {
    PendingWake,
    Ready,
}

#[derive(Clone, Debug)]
enum AsyncFutureResult {
    String(String),
    U32(u32),
    U64(u64),
    Bytes(Vec<u8>),
    BytesOpt(Option<Vec<u8>>),
    BytesVec(Vec<Vec<u8>>),
    PendingString,
    PendingU32,
    Failed(String),
    Void,
}

#[derive(Clone, Debug)]
struct AsyncFutureState {
    poll_state: AsyncFuturePollState,
    cancelled: bool,
    ready: bool,
    result: AsyncFutureResult,
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

fn empty_rust_buffer() -> RustBuffer {
    RustBuffer {
        data: std::ptr::null_mut(),
        len: 0,
    }
}

fn empty_rust_buffer_opt() -> RustBufferOpt {
    RustBufferOpt {
        is_some: 0,
        value: empty_rust_buffer(),
    }
}

fn empty_rust_buffer_vec() -> RustBufferVec {
    RustBufferVec {
        data: std::ptr::null_mut(),
        len: 0,
    }
}

fn vec_into_rust_buffer(mut data: Vec<u8>) -> RustBuffer {
    if data.is_empty() {
        return empty_rust_buffer();
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
        return empty_rust_buffer_vec();
    }
    let out = RustBufferVec {
        data: items.as_mut_ptr(),
        len: items.len() as u64,
    };
    std::mem::forget(items);
    out
}

fn enqueue_async_future(result: AsyncFutureResult) -> u64 {
    let handle = NEXT_ASYNC_FUTURE_HANDLE.fetch_add(1, Ordering::Relaxed);
    ASYNC_FUTURES.lock().expect("async futures lock").insert(
        handle,
        AsyncFutureState {
            poll_state: AsyncFuturePollState::PendingWake,
            cancelled: false,
            ready: true,
            result,
        },
    );
    handle
}

fn enqueue_pending_u32_future() -> u64 {
    let handle = NEXT_ASYNC_FUTURE_HANDLE.fetch_add(1, Ordering::Relaxed);
    ASYNC_FUTURES.lock().expect("async futures lock").insert(
        handle,
        AsyncFutureState {
            poll_state: AsyncFuturePollState::PendingWake,
            cancelled: false,
            ready: false,
            result: AsyncFutureResult::PendingU32,
        },
    );
    handle
}

fn enqueue_pending_string_future() -> u64 {
    let handle = NEXT_ASYNC_FUTURE_HANDLE.fetch_add(1, Ordering::Relaxed);
    ASYNC_FUTURES.lock().expect("async futures lock").insert(
        handle,
        AsyncFutureState {
            poll_state: AsyncFuturePollState::PendingWake,
            cancelled: false,
            ready: false,
            result: AsyncFutureResult::PendingString,
        },
    );
    handle
}

fn enqueue_string_future(result: String) -> u64 {
    enqueue_async_future(AsyncFutureResult::String(result))
}

fn enqueue_u32_future(result: u32) -> u64 {
    enqueue_async_future(AsyncFutureResult::U32(result))
}

fn enqueue_u64_future(result: u64) -> u64 {
    enqueue_async_future(AsyncFutureResult::U64(result))
}

fn enqueue_bytes_future(result: Vec<u8>) -> u64 {
    enqueue_async_future(AsyncFutureResult::Bytes(result))
}

fn enqueue_bytes_opt_future(result: Option<Vec<u8>>) -> u64 {
    enqueue_async_future(AsyncFutureResult::BytesOpt(result))
}

fn enqueue_bytes_vec_future(result: Vec<Vec<u8>>) -> u64 {
    enqueue_async_future(AsyncFutureResult::BytesVec(result))
}

fn enqueue_void_future() -> u64 {
    enqueue_async_future(AsyncFutureResult::Void)
}

fn poll_async_future(handle: u64, callback: extern "C" fn(u64, i8), callback_data: u64) {
    let mut futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get_mut(&handle) else {
        callback(callback_data, RUST_FUTURE_POLL_READY);
        return;
    };
    if state.cancelled {
        callback(callback_data, RUST_FUTURE_POLL_READY);
        return;
    }
    if !state.ready {
        callback(callback_data, RUST_FUTURE_POLL_WAKE);
        return;
    }

    match state.poll_state {
        AsyncFuturePollState::PendingWake => {
            state.poll_state = AsyncFuturePollState::Ready;
            callback(callback_data, RUST_FUTURE_POLL_WAKE);
        }
        AsyncFuturePollState::Ready => {
            callback(callback_data, RUST_FUTURE_POLL_READY);
        }
    }
}

fn cancel_async_future(handle: u64) {
    if let Some(state) = ASYNC_FUTURES
        .lock()
        .expect("async futures lock")
        .get_mut(&handle)
    {
        state.cancelled = true;
        state.ready = true;
        state.poll_state = AsyncFuturePollState::Ready;
        ASYNC_CANCEL_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

fn free_async_future(handle: u64) {
    let removed = ASYNC_FUTURES
        .lock()
        .expect("async futures lock")
        .remove(&handle);
    if removed.is_some() {
        ASYNC_FREE_COUNT.fetch_add(1, Ordering::Relaxed);
    }
    ASYNC_ADDER_OFFSETS
        .lock()
        .expect("async adder offsets lock")
        .remove(&handle);
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
pub extern "C" fn adder_callback_init(vtable: *const AdderVTable) {
    if vtable.is_null() {
        return;
    }
    let value = unsafe { *vtable };
    *ADDER_VTABLE.lock().expect("adder vtable lock") = Some(value);
}

fn invoke_adder_callback(adder: u64, left: u32, right: u32) -> Option<u32> {
    let Some(vtable) = *ADDER_VTABLE.lock().expect("adder vtable lock") else {
        return None;
    };
    let callback_handle = (vtable.uniffi_clone)(adder);
    let mut out = 0_u32;
    let mut status = RustCallStatus {
        code: RUST_CALL_STATUS_SUCCESS,
        error_buf: std::ptr::null_mut(),
    };
    (vtable.add)(callback_handle, left, right, &mut out, &mut status);
    (vtable.uniffi_free)(callback_handle);
    if status.code == RUST_CALL_STATUS_SUCCESS {
        Some(out)
    } else {
        None
    }
}

fn invoke_checked_adder_callback(adder: u64, left: u32, right: u32) -> (Option<u32>, i8) {
    let Some(vtable) = *ADDER_VTABLE.lock().expect("adder vtable lock") else {
        return (None, RUST_CALL_STATUS_UNEXPECTED_ERROR);
    };
    let callback_handle = (vtable.uniffi_clone)(adder);
    let mut out = 0_u32;
    let mut status = RustCallStatus {
        code: RUST_CALL_STATUS_SUCCESS,
        error_buf: std::ptr::null_mut(),
    };
    (vtable.checked_add)(callback_handle, left, right, &mut out, &mut status);
    (vtable.uniffi_free)(callback_handle);
    if !status.error_buf.is_null() {
        unsafe {
            let _ = CString::from_raw(status.error_buf);
        }
    }
    (Some(out), status.code)
}

extern "C" fn complete_async_adder(callback_data: u64, result: ForeignFutureResultU32) {
    if !result.call_status.error_buf.is_null() {
        unsafe {
            let _ = CString::from_raw(result.call_status.error_buf);
        }
    }
    let offset = ASYNC_ADDER_OFFSETS
        .lock()
        .expect("async adder offsets lock")
        .remove(&callback_data)
        .unwrap_or_default();
    let mut futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get_mut(&callback_data) else {
        return;
    };
    if result.call_status.code == RUST_CALL_STATUS_SUCCESS {
        state.result = AsyncFutureResult::U32(result.return_value.saturating_add(offset));
    } else {
        state.result = AsyncFutureResult::Failed("async adder callback failed".to_string());
    }
    state.ready = true;
    state.poll_state = AsyncFuturePollState::PendingWake;
}

#[unsafe(no_mangle)]
pub extern "C" fn apply_adder(adder: u64, left: u32, right: u32) -> u32 {
    invoke_adder_callback(adder, left, right).map(|out| out + 1).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn async_apply_adder(adder: u64, left: u32, right: u32) -> u64 {
    let Some(vtable) = *ADDER_VTABLE.lock().expect("adder vtable lock") else {
        return enqueue_u32_future(0);
    };
    let handle = enqueue_pending_u32_future();
    ASYNC_ADDER_OFFSETS
        .lock()
        .expect("async adder offsets lock")
        .insert(handle, 2);
    let callback_handle = (vtable.uniffi_clone)(adder);
    let mut dropped = ForeignFutureDroppedCallbackStruct {
        handle: 0,
        callback: None,
    };
    (vtable.add_async)(
        callback_handle,
        left,
        right,
        complete_async_adder,
        handle,
        &mut dropped,
    );
    (vtable.uniffi_free)(callback_handle);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn checked_apply_adder(adder: u64, left: u32, right: u32) -> *mut c_char {
    let (result, status_code) = invoke_checked_adder_callback(adder, left, right);
    let envelope = if status_code == RUST_CALL_STATUS_SUCCESS {
        serde_json::json!({
            "ok": result.unwrap_or(0)
        })
    } else if left == 0 {
        serde_json::json!({
            "err": {
                "tag": "negativeInput",
                "value": -1
            }
        })
    } else {
        serde_json::json!({
            "err": {
                "tag": "divisionByZero"
            }
        })
    };
    CString::new(envelope.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn formatter_callback_init(vtable: *const FormatterVTable) {
    if vtable.is_null() {
        return;
    }
    let value = unsafe { *vtable };
    *FORMATTER_VTABLE.lock().expect("formatter vtable lock") = Some(value);
}

extern "C" fn complete_async_formatter(callback_data: u64, result: ForeignFutureResultString) {
    if !result.call_status.error_buf.is_null() {
        unsafe {
            let _ = CString::from_raw(result.call_status.error_buf);
        }
    }
    let mut futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get_mut(&callback_data) else {
        if !result.return_value.is_null() {
            unsafe {
                let _ = CString::from_raw(result.return_value);
            }
        }
        return;
    };
    if result.call_status.code == RUST_CALL_STATUS_SUCCESS {
        if result.return_value.is_null() {
            state.result = AsyncFutureResult::Failed("async formatter callback returned null".to_string());
        } else {
            let value = unsafe { CStr::from_ptr(result.return_value) }
                .to_string_lossy()
                .into_owned();
            unsafe {
                let _ = CString::from_raw(result.return_value);
            }
            state.result = AsyncFutureResult::String(value);
        }
    } else {
        if !result.return_value.is_null() {
            unsafe {
                let _ = CString::from_raw(result.return_value);
            }
        }
        state.result = AsyncFutureResult::Failed("async formatter callback failed".to_string());
    }
    state.ready = true;
    state.poll_state = AsyncFuturePollState::PendingWake;
}

extern "C" fn complete_async_formatter_len(callback_data: u64, result: ForeignFutureResultString) {
    if !result.call_status.error_buf.is_null() {
        unsafe {
            let _ = CString::from_raw(result.call_status.error_buf);
        }
    }
    let mut futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get_mut(&callback_data) else {
        if !result.return_value.is_null() {
            unsafe {
                let _ = CString::from_raw(result.return_value);
            }
        }
        return;
    };
    if result.call_status.code == RUST_CALL_STATUS_SUCCESS {
        let len = if result.return_value.is_null() {
            0
        } else {
            let value = unsafe { CStr::from_ptr(result.return_value) }
                .to_string_lossy()
                .into_owned();
            unsafe {
                let _ = CString::from_raw(result.return_value);
            }
            value.len() as u32
        };
        state.result = AsyncFutureResult::U32(len);
    } else {
        if !result.return_value.is_null() {
            unsafe {
                let _ = CString::from_raw(result.return_value);
            }
        }
        state.result = AsyncFutureResult::Failed("async formatter callback failed".to_string());
    }
    state.ready = true;
    state.poll_state = AsyncFuturePollState::PendingWake;
}

#[unsafe(no_mangle)]
pub extern "C" fn apply_formatter(
    formatter: u64,
    prefix: *const c_char,
    person: *const c_char,
    outcome: *const c_char,
) -> u32 {
    let Some(vtable) = *FORMATTER_VTABLE.lock().expect("formatter vtable lock") else {
        return 0;
    };
    let callback_handle = (vtable.uniffi_clone)(formatter);
    let mut out: *mut c_char = std::ptr::null_mut();
    let mut status = RustCallStatus {
        code: RUST_CALL_STATUS_SUCCESS,
        error_buf: std::ptr::null_mut(),
    };
    (vtable.format)(
        callback_handle,
        prefix,
        person,
        outcome,
        &mut out,
        &mut status,
    );
    (vtable.uniffi_free)(callback_handle);
    if status.code != RUST_CALL_STATUS_SUCCESS {
        if !status.error_buf.is_null() {
            unsafe {
                let _ = CString::from_raw(status.error_buf);
            }
        }
        return 0;
    }
    if out.is_null() {
        return 0;
    }
    let value = unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned();
    unsafe {
        let _ = CString::from_raw(out);
    }
    value.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn async_apply_formatter(
    formatter: u64,
    prefix: *const c_char,
    person: *const c_char,
    outcome: *const c_char,
) -> u64 {
    let Some(vtable) = *FORMATTER_VTABLE.lock().expect("formatter vtable lock") else {
        return enqueue_string_future(String::new());
    };
    let handle = enqueue_pending_string_future();
    let callback_handle = (vtable.uniffi_clone)(formatter);
    let mut dropped = ForeignFutureDroppedCallbackStruct {
        handle: 0,
        callback: None,
    };
    (vtable.format_async)(
        callback_handle,
        prefix,
        person,
        outcome,
        complete_async_formatter,
        handle,
        &mut dropped,
    );
    (vtable.uniffi_free)(callback_handle);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn async_apply_formatter_optional_len(
    formatter: u64,
    prefix: *const c_char,
    person: *const c_char,
    outcome: *const c_char,
) -> u64 {
    let Some(vtable) = *FORMATTER_VTABLE.lock().expect("formatter vtable lock") else {
        return enqueue_u32_future(0);
    };
    let handle = enqueue_pending_u32_future();
    let callback_handle = (vtable.uniffi_clone)(formatter);
    let mut dropped = ForeignFutureDroppedCallbackStruct {
        handle: 0,
        callback: None,
    };
    (vtable.format_async_optional)(
        callback_handle,
        prefix,
        person,
        outcome,
        complete_async_formatter_len,
        handle,
        &mut dropped,
    );
    (vtable.uniffi_free)(callback_handle);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn async_apply_formatter_person_len(
    formatter: u64,
    prefix: *const c_char,
    person: *const c_char,
    outcome: *const c_char,
) -> u64 {
    let Some(vtable) = *FORMATTER_VTABLE.lock().expect("formatter vtable lock") else {
        return enqueue_u32_future(0);
    };
    let handle = enqueue_pending_u32_future();
    let callback_handle = (vtable.uniffi_clone)(formatter);
    let mut dropped = ForeignFutureDroppedCallbackStruct {
        handle: 0,
        callback: None,
    };
    (vtable.format_async_person)(
        callback_handle,
        prefix,
        person,
        outcome,
        complete_async_formatter_len,
        handle,
        &mut dropped,
    );
    (vtable.uniffi_free)(callback_handle);
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn async_apply_formatter_outcome_len(
    formatter: u64,
    prefix: *const c_char,
    person: *const c_char,
    outcome: *const c_char,
) -> u64 {
    let Some(vtable) = *FORMATTER_VTABLE.lock().expect("formatter vtable lock") else {
        return enqueue_u32_future(0);
    };
    let handle = enqueue_pending_u32_future();
    let callback_handle = (vtable.uniffi_clone)(formatter);
    let mut dropped = ForeignFutureDroppedCallbackStruct {
        handle: 0,
        callback: None,
    };
    (vtable.format_async_outcome)(
        callback_handle,
        prefix,
        person,
        outcome,
        complete_async_formatter_len,
        handle,
        &mut dropped,
    );
    (vtable.uniffi_free)(callback_handle);
    handle
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
        return empty_rust_buffer_opt();
    }
    RustBufferOpt {
        is_some: 1,
        value: vec_into_rust_buffer(rust_buffer_to_vec(input.value)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bytes_chunks_echo(input: RustBufferVec) -> RustBufferVec {
    if input.data.is_null() || input.len == 0 {
        return empty_rust_buffer_vec();
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
pub extern "C" fn async_bytes_echo(input: RustBuffer) -> u64 {
    enqueue_bytes_future(rust_buffer_to_vec(input))
}

#[unsafe(no_mangle)]
pub extern "C" fn async_bytes_maybe_echo(input: RustBufferOpt) -> u64 {
    if input.is_some == 0 {
        enqueue_bytes_opt_future(None)
    } else {
        enqueue_bytes_opt_future(Some(rust_buffer_to_vec(input.value)))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn async_bytes_chunks_echo(input: RustBufferVec) -> u64 {
    if input.data.is_null() || input.len == 0 {
        return enqueue_bytes_vec_future(Vec::new());
    }
    let in_items = unsafe { std::slice::from_raw_parts(input.data, input.len as usize) };
    let out_items = in_items
        .iter()
        .copied()
        .map(rust_buffer_to_vec)
        .collect::<Vec<_>>();
    enqueue_bytes_vec_future(out_items)
}

#[unsafe(no_mangle)]
pub extern "C" fn async_counts(items: *const c_char) -> u64 {
    let counts = if items.is_null() {
        HashMap::<String, u32>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(items) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, u32>>(&payload).unwrap_or_default()
    };
    let mut out = counts;
    let total = out.values().copied().sum::<u32>();
    out.insert("total".to_string(), total);
    enqueue_string_future(serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn async_label_echo(input: *const c_char) -> u64 {
    let label = if input.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned()
    };
    enqueue_string_future(format!("label:{label}"))
}

#[unsafe(no_mangle)]
pub extern "C" fn count_add(left: u32, right: u32) -> u32 {
    left + right
}

#[unsafe(no_mangle)]
pub extern "C" fn async_count_add(left: u32, right: u32) -> u64 {
    enqueue_u32_future(left + right)
}

#[unsafe(no_mangle)]
pub extern "C" fn blob_echo(input: RustBuffer) -> RustBuffer {
    vec_into_rust_buffer(rust_buffer_to_vec(input))
}

#[unsafe(no_mangle)]
pub extern "C" fn async_blob_echo(input: RustBuffer) -> u64 {
    enqueue_bytes_future(rust_buffer_to_vec(input))
}

#[unsafe(no_mangle)]
pub extern "C" fn blob_maybe_echo(input: RustBufferOpt) -> RustBufferOpt {
    if input.is_some == 0 {
        return empty_rust_buffer_opt();
    }
    RustBufferOpt {
        is_some: 1,
        value: vec_into_rust_buffer(rust_buffer_to_vec(input.value)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn async_blob_maybe_echo(input: RustBufferOpt) -> u64 {
    if input.is_some == 0 {
        enqueue_bytes_opt_future(None)
    } else {
        enqueue_bytes_opt_future(Some(rust_buffer_to_vec(input.value)))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn count_map_echo(items: *const c_char) -> *mut c_char {
    let mut counts = if items.is_null() {
        HashMap::<String, u32>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(items) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, u32>>(&payload).unwrap_or_default()
    };
    let total = counts.values().copied().sum::<u32>();
    counts.insert("total".to_string(), total);
    CString::new(serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string()))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn async_count_map_echo(items: *const c_char) -> u64 {
    let mut counts = if items.is_null() {
        HashMap::<String, u32>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(items) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, u32>>(&payload).unwrap_or_default()
    };
    let total = counts.values().copied().sum::<u32>();
    counts.insert("total".to_string(), total);
    enqueue_string_future(serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn count_buckets_echo(input: *const c_char) -> *mut c_char {
    let mut buckets = if input.is_null() {
        HashMap::<String, Vec<u32>>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, Vec<u32>>>(&payload).unwrap_or_default()
    };
    let total = buckets
        .values()
        .flat_map(|values| values.iter())
        .copied()
        .sum::<u32>();
    buckets.insert("totals".to_string(), vec![total]);
    CString::new(serde_json::to_string(&buckets).unwrap_or_else(|_| "{}".to_string()))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn async_count_buckets_echo(input: *const c_char) -> u64 {
    let mut buckets = if input.is_null() {
        HashMap::<String, Vec<u32>>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, Vec<u32>>>(&payload).unwrap_or_default()
    };
    let total = buckets
        .values()
        .flat_map(|values| values.iter())
        .copied()
        .sum::<u32>();
    buckets.insert("totals".to_string(), vec![total]);
    enqueue_string_future(serde_json::to_string(&buckets).unwrap_or_else(|_| "{}".to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn maybe_blob_map_echo(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return CString::new("{}").expect("valid CString").into_raw();
    }
    let payload = unsafe { CStr::from_ptr(input) }
        .to_string_lossy()
        .into_owned();
    CString::new(payload).expect("valid CString").into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn async_maybe_blob_map_echo(input: *const c_char) -> u64 {
    if input.is_null() {
        return enqueue_string_future("{}".to_string());
    }
    let payload = unsafe { CStr::from_ptr(input) }
        .to_string_lossy()
        .into_owned();
    enqueue_string_future(payload)
}

#[unsafe(no_mangle)]
pub extern "C" fn async_fail_string() -> u64 {
    enqueue_async_future(AsyncFutureResult::Failed(
        "forced async failure".to_string(),
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn async_never_string() -> u64 {
    enqueue_pending_string_future()
}

#[unsafe(no_mangle)]
pub extern "C" fn async_counter_create_new(initial: u32) -> u64 {
    enqueue_u64_future(counter_new(initial))
}

#[unsafe(no_mangle)]
pub extern "C" fn async_cancel_count() -> u32 {
    ASYNC_CANCEL_COUNT.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn async_free_count() -> u32 {
    ASYNC_FREE_COUNT.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn reset_async_future_counts() {
    ASYNC_CANCEL_COUNT.store(0, Ordering::Relaxed);
    ASYNC_FREE_COUNT.store(0, Ordering::Relaxed);
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
pub extern "C" fn async_add(left: u32, right: u32) -> u64 {
    enqueue_u32_future(left + right)
}

#[unsafe(no_mangle)]
pub extern "C" fn async_tick() -> u64 {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    enqueue_void_future()
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
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_string(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_string(
    handle: u64,
    out_status: *mut RustCallStatus,
) -> *mut c_char {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
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
    match &state.result {
        AsyncFutureResult::String(result) => CString::new(result.as_str())
            .expect("valid CString")
            .into_raw(),
        AsyncFutureResult::Failed(message) => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new(message.as_str()).expect("valid CString").into_raw(),
            );
            std::ptr::null_mut()
        }
        AsyncFutureResult::PendingString => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("string callback future not completed")
                    .expect("valid CString")
                    .into_raw(),
            );
            std::ptr::null_mut()
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for string")
                    .expect("valid CString")
                    .into_raw(),
            );
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_string(handle: u64) {
    free_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_u32(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_u32(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_u32(handle: u64, out_status: *mut RustCallStatus) -> u32 {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing u32 future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return 0;
    };
    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return 0;
    }
    match &state.result {
        AsyncFutureResult::U32(value) => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
            *value
        }
        AsyncFutureResult::Failed(message) => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new(message.as_str()).expect("valid CString").into_raw(),
            );
            0
        }
        AsyncFutureResult::PendingU32 => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("u32 callback future not completed")
                    .expect("valid CString")
                    .into_raw(),
            );
            0
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for u32")
                    .expect("valid CString")
                    .into_raw(),
            );
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_u32(handle: u64) {
    free_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_u64(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_u64(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_u64(handle: u64, out_status: *mut RustCallStatus) -> u64 {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing u64 future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return 0;
    };
    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return 0;
    }
    match &state.result {
        AsyncFutureResult::U64(value) => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
            *value
        }
        AsyncFutureResult::Failed(message) => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new(message.as_str()).expect("valid CString").into_raw(),
            );
            0
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for u64")
                    .expect("valid CString")
                    .into_raw(),
            );
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_u64(handle: u64) {
    free_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_bytes(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_bytes(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_bytes(
    handle: u64,
    out_status: *mut RustCallStatus,
) -> RustBuffer {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing bytes future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return empty_rust_buffer();
    };
    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return empty_rust_buffer();
    }
    match &state.result {
        AsyncFutureResult::Bytes(value) => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
            vec_into_rust_buffer(value.clone())
        }
        AsyncFutureResult::Failed(message) => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new(message.as_str()).expect("valid CString").into_raw(),
            );
            empty_rust_buffer()
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for bytes")
                    .expect("valid CString")
                    .into_raw(),
            );
            empty_rust_buffer()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_bytes(handle: u64) {
    free_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_bytes_opt(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_bytes_opt(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_bytes_opt(
    handle: u64,
    out_status: *mut RustCallStatus,
) -> RustBufferOpt {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing bytes_opt future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return empty_rust_buffer_opt();
    };
    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return empty_rust_buffer_opt();
    }
    match &state.result {
        AsyncFutureResult::BytesOpt(Some(value)) => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
            RustBufferOpt {
                is_some: 1,
                value: vec_into_rust_buffer(value.clone()),
            }
        }
        AsyncFutureResult::BytesOpt(None) => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
            empty_rust_buffer_opt()
        }
        AsyncFutureResult::Failed(message) => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new(message.as_str()).expect("valid CString").into_raw(),
            );
            empty_rust_buffer_opt()
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for bytes_opt")
                    .expect("valid CString")
                    .into_raw(),
            );
            empty_rust_buffer_opt()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_bytes_opt(handle: u64) {
    free_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_bytes_vec(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_bytes_vec(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_bytes_vec(
    handle: u64,
    out_status: *mut RustCallStatus,
) -> RustBufferVec {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing bytes_vec future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return empty_rust_buffer_vec();
    };
    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return empty_rust_buffer_vec();
    }
    match &state.result {
        AsyncFutureResult::BytesVec(values) => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
            let out = values
                .iter()
                .cloned()
                .map(vec_into_rust_buffer)
                .collect::<Vec<_>>();
            vec_into_rust_buffer_vec(out)
        }
        AsyncFutureResult::Failed(message) => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new(message.as_str()).expect("valid CString").into_raw(),
            );
            empty_rust_buffer_vec()
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for bytes_vec")
                    .expect("valid CString")
                    .into_raw(),
            );
            empty_rust_buffer_vec()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_bytes_vec(handle: u64) {
    free_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_poll_void(
    handle: u64,
    callback: extern "C" fn(u64, i8),
    callback_data: u64,
) {
    poll_async_future(handle, callback, callback_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_cancel_void(handle: u64) {
    cancel_async_future(handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_complete_void(handle: u64, out_status: *mut RustCallStatus) {
    let futures = ASYNC_FUTURES.lock().expect("async futures lock");
    let Some(state) = futures.get(&handle) else {
        write_out_status(
            out_status,
            RUST_CALL_STATUS_UNEXPECTED_ERROR,
            CString::new("missing void future handle")
                .expect("valid CString")
                .into_raw(),
        );
        return;
    };
    if state.cancelled {
        write_out_status(out_status, RUST_CALL_STATUS_CANCELLED, std::ptr::null_mut());
        return;
    }
    match state.result {
        AsyncFutureResult::Void => {
            write_out_status(out_status, RUST_CALL_STATUS_SUCCESS, std::ptr::null_mut());
        }
        _ => {
            write_out_status(
                out_status,
                RUST_CALL_STATUS_UNEXPECTED_ERROR,
                CString::new("invalid async result type for void")
                    .expect("valid CString")
                    .into_raw(),
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_future_free_void(handle: u64) {
    free_async_future(handle);
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
pub extern "C" fn counter_apply_adder_with(handle: u64, adder: u64, left: u32, right: u32) -> u32 {
    let out = invoke_adder_callback(adder, left, right).unwrap_or(0);
    let base = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default() as u32;
    base + out
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_apply_adder_with(handle: u64, adder: u64, left: u32, right: u32) -> u64 {
    let Some(vtable) = *ADDER_VTABLE.lock().expect("adder vtable lock") else {
        return enqueue_u32_future(0);
    };
    let base = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default() as u32;
    let future_handle = enqueue_pending_u32_future();
    ASYNC_ADDER_OFFSETS
        .lock()
        .expect("async adder offsets lock")
        .insert(future_handle, base);
    let callback_handle = (vtable.uniffi_clone)(adder);
    let mut dropped = ForeignFutureDroppedCallbackStruct {
        handle: 0,
        callback: None,
    };
    (vtable.add_async)(
        callback_handle,
        left,
        right,
        complete_async_adder,
        future_handle,
        &mut dropped,
    );
    (vtable.uniffi_free)(callback_handle);
    future_handle
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_checked_apply_adder_with(
    handle: u64,
    adder: u64,
    left: u32,
    right: u32,
) -> *mut c_char {
    let (result, status_code) = invoke_checked_adder_callback(adder, left, right);
    let base = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default() as u32;
    let envelope = if status_code == RUST_CALL_STATUS_SUCCESS {
        serde_json::json!({
            "ok": base + result.unwrap_or(0)
        })
    } else if right == 0 {
        serde_json::json!({
            "err": { "tag": "divisionByZero" }
        })
    } else if left == 0 {
        serde_json::json!({
            "err": { "tag": "negativeInput", "value": -1 }
        })
    } else {
        serde_json::json!({ "err": { "tag": "divisionByZero" } })
    };
    CString::new(envelope.to_string())
        .expect("valid CString")
        .into_raw()
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

fn counter_display_text(handle: u64) -> String {
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
    if label.is_empty() {
        format!("counter:{value}")
    } else {
        format!("counter:{value}:{label}")
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_describe(handle: u64) -> *mut c_char {
    CString::new(counter_display_text(handle))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_uniffi_trait_display(handle: u64) -> *mut c_char {
    CString::new(counter_display_text(handle))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_uniffi_trait_hash(handle: u64) -> u64 {
    let text = counter_display_text(handle);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_uniffi_trait_eq(handle: u64, other: u64) -> bool {
    counter_display_text(handle) == counter_display_text(other)
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_uniffi_trait_ne(handle: u64, other: u64) -> bool {
    !counter_uniffi_trait_eq(handle, other)
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_uniffi_trait_ord_cmp(handle: u64, other: u64) -> i8 {
    use std::cmp::Ordering;

    match counter_display_text(handle).cmp(&counter_display_text(other)) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
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
pub extern "C" fn counter_async_value(handle: u64) -> u64 {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    enqueue_u32_future(value.max(0) as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_snapshot_bytes(handle: u64) -> u64 {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    enqueue_bytes_future(format!("async-bytes:{value}").into_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_counts(handle: u64, items: *const c_char) -> u64 {
    let mut counts = if items.is_null() {
        HashMap::<String, u32>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(items) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, u32>>(&payload).unwrap_or_default()
    };
    let counter_value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    counts.insert("counter".to_string(), counter_value);
    let total = counts.values().copied().sum::<u32>();
    counts.insert("total".to_string(), total);
    enqueue_string_future(serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_label_echo(handle: u64, input: *const c_char) -> u64 {
    let label = if input.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned()
    };
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default();
    enqueue_string_future(format!("counter:{value}:{label}"))
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_count_plus(handle: u64, amount: u32) -> u32 {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    value + amount
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_count_plus(handle: u64, amount: u32) -> u64 {
    let value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    enqueue_u32_future(value + amount)
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_blob_echo(_handle: u64, input: RustBuffer) -> RustBuffer {
    vec_into_rust_buffer(rust_buffer_to_vec(input))
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_blob_echo(_handle: u64, input: RustBuffer) -> u64 {
    enqueue_bytes_future(rust_buffer_to_vec(input))
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_blob_maybe_echo(_handle: u64, input: RustBufferOpt) -> RustBufferOpt {
    if input.is_some == 0 {
        return empty_rust_buffer_opt();
    }
    RustBufferOpt {
        is_some: 1,
        value: vec_into_rust_buffer(rust_buffer_to_vec(input.value)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_blob_maybe_echo(_handle: u64, input: RustBufferOpt) -> u64 {
    if input.is_some == 0 {
        enqueue_bytes_opt_future(None)
    } else {
        enqueue_bytes_opt_future(Some(rust_buffer_to_vec(input.value)))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_count_map_echo(handle: u64, items: *const c_char) -> *mut c_char {
    let mut counts = if items.is_null() {
        HashMap::<String, u32>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(items) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, u32>>(&payload).unwrap_or_default()
    };
    let counter_value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    counts.insert("counter".to_string(), counter_value);
    let total = counts.values().copied().sum::<u32>();
    counts.insert("total".to_string(), total);
    CString::new(serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string()))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_count_map_echo(handle: u64, items: *const c_char) -> u64 {
    let mut counts = if items.is_null() {
        HashMap::<String, u32>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(items) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, u32>>(&payload).unwrap_or_default()
    };
    let counter_value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    counts.insert("counter".to_string(), counter_value);
    let total = counts.values().copied().sum::<u32>();
    counts.insert("total".to_string(), total);
    enqueue_string_future(serde_json::to_string(&counts).unwrap_or_else(|_| "{}".to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_count_buckets_echo(handle: u64, input: *const c_char) -> *mut c_char {
    let mut buckets = if input.is_null() {
        HashMap::<String, Vec<u32>>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, Vec<u32>>>(&payload).unwrap_or_default()
    };
    let counter_value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    buckets.insert("counter".to_string(), vec![counter_value]);
    let total = buckets
        .values()
        .flat_map(|values| values.iter())
        .copied()
        .sum::<u32>();
    buckets.insert("totals".to_string(), vec![total]);
    CString::new(serde_json::to_string(&buckets).unwrap_or_else(|_| "{}".to_string()))
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_count_buckets_echo(handle: u64, input: *const c_char) -> u64 {
    let mut buckets = if input.is_null() {
        HashMap::<String, Vec<u32>>::new()
    } else {
        let payload = unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<HashMap<String, Vec<u32>>>(&payload).unwrap_or_default()
    };
    let counter_value = COUNTERS
        .lock()
        .expect("counter map lock")
        .get(&handle)
        .copied()
        .unwrap_or_default()
        .max(0) as u32;
    buckets.insert("counter".to_string(), vec![counter_value]);
    let total = buckets
        .values()
        .flat_map(|values| values.iter())
        .copied()
        .sum::<u32>();
    buckets.insert("totals".to_string(), vec![total]);
    enqueue_string_future(serde_json::to_string(&buckets).unwrap_or_else(|_| "{}".to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_maybe_blob_map_echo(handle: u64, input: *const c_char) -> *mut c_char {
    let mut value = if input.is_null() {
        serde_json::json!({})
    } else {
        let payload = unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<Value>(&payload).unwrap_or_else(|_| serde_json::json!({}))
    };
    if let Value::Object(ref mut obj) = value {
        let _ = handle;
        obj.insert("counter".to_string(), Value::Null);
    }
    CString::new(value.to_string())
        .expect("valid CString")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn counter_async_maybe_blob_map_echo(handle: u64, input: *const c_char) -> u64 {
    let mut value = if input.is_null() {
        serde_json::json!({})
    } else {
        let payload = unsafe { CStr::from_ptr(input) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str::<Value>(&payload).unwrap_or_else(|_| serde_json::json!({}))
    };
    if let Value::Object(ref mut obj) = value {
        let _ = handle;
        obj.insert("counter".to_string(), Value::Null);
    }
    enqueue_string_future(value.to_string())
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
