// Not implemented: listify (sequence<u32> round-trip).
// The C ABI for typed sequences requires buffer layout matching that is
// complex to hand-write. The listify binding is covered by the golden test;
// runtime testing focuses on the simpler container types below.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde_json::Value;

/// Bytes buffer matching the Dart _RustBuffer layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustBuffer {
    pub data: *mut u8,
    pub len: u64,
}

/// Vector of RustBuffers matching the Dart _RustBufferVec layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RustBufferVec {
    pub data: *mut RustBuffer,
    pub len: u64,
}

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
pub extern "C" fn rust_bytes_vec_free(vec: RustBufferVec) {
    if !vec.data.is_null() && vec.len > 0 {
        unsafe {
            drop(Vec::from_raw_parts(vec.data, vec.len as usize, vec.len as usize));
        }
    }
}

/// Round-trip a sequence of byte buffers.
#[no_mangle]
pub extern "C" fn chunk(input: RustBufferVec) -> RustBufferVec {
    let count = input.len as usize;
    if count == 0 || input.data.is_null() {
        return RustBufferVec {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    let in_slice = unsafe { std::slice::from_raw_parts(input.data, count) };
    let mut out_bufs: Vec<RustBuffer> = Vec::with_capacity(count);
    for item in in_slice {
        let item_len = item.len as usize;
        if item_len == 0 || item.data.is_null() {
            out_bufs.push(RustBuffer {
                data: std::ptr::null_mut(),
                len: 0,
            });
        } else {
            let src = unsafe { std::slice::from_raw_parts(item.data, item_len) };
            let mut copy = src.to_vec();
            let ptr = copy.as_mut_ptr();
            let len = copy.len() as u64;
            std::mem::forget(copy);
            out_bufs.push(RustBuffer { data: ptr, len });
        }
    }
    let ptr = out_bufs.as_mut_ptr();
    let len = out_bufs.len() as u64;
    std::mem::forget(out_bufs);
    RustBufferVec { data: ptr, len }
}

/// Round-trip a map<string, u64>, adding a "total" key.
#[no_mangle]
pub extern "C" fn counts(items: *const c_char) -> *mut c_char {
    let json_str = unsafe { CStr::from_ptr(items).to_str().unwrap_or("{}") };
    let parsed: Value = serde_json::from_str(json_str).unwrap_or(Value::Object(Default::default()));
    let map = parsed.as_object().cloned().unwrap_or_default();
    let mut total: u64 = 0;
    let mut result = serde_json::Map::new();
    for (k, v) in &map {
        let n = v.as_u64().unwrap_or(0);
        total += n;
        result.insert(k.clone(), Value::Number(n.into()));
    }
    result.insert("total".to_string(), Value::Number(total.into()));
    let out = serde_json::to_string(&Value::Object(result)).unwrap();
    CString::new(out).unwrap().into_raw()
}

/// Round-trip an optional string.
#[no_mangle]
pub extern "C" fn maybe_name(value: *const c_char) -> *mut c_char {
    if value.is_null() {
        return std::ptr::null_mut();
    }
    let s = unsafe { CStr::from_ptr(value).to_str().unwrap_or("") };
    CString::new(s).unwrap().into_raw()
}
