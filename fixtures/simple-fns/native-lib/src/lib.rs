use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU32, Ordering};

static TICK_COUNT: AtomicU32 = AtomicU32::new(0);
static FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static BYTES_FREE_COUNT: AtomicU32 = AtomicU32::new(0);
static BYTES_VEC_FREE_COUNT: AtomicU32 = AtomicU32::new(0);

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

#[unsafe(no_mangle)]
pub extern "C" fn add(left: u32, right: u32) -> u32 {
    left + right
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
