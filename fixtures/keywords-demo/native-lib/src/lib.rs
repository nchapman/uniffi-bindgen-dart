use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static SUPER_STATE: LazyLock<Mutex<HashMap<u64, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[no_mangle]
pub extern "C" fn rust_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

/// `class(switch) -> switch * 2`
#[no_mangle]
pub extern "C" fn class(switch_: u32) -> u32 {
    switch_ * 2
}

/// `is(sealed) -> !sealed`
#[no_mangle]
pub extern "C" fn is(sealed_: bool) -> bool {
    !sealed_
}

/// `return(var) -> "return:" + var`
#[no_mangle]
pub extern "C" fn r#return(var_: *const c_char) -> *mut c_char {
    let s = unsafe { CStr::from_ptr(var_).to_str().unwrap_or("") };
    let out = format!("return:{}", s);
    CString::new(out).unwrap().into_raw()
}

/// Create a new Super object handle. Internal counter starts at 0.
#[no_mangle]
pub extern "C" fn super_new() -> u64 {
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    SUPER_STATE.lock().unwrap().insert(handle, 0);
    handle
}

/// Free a Super object handle.
#[no_mangle]
pub extern "C" fn super_free(handle: u64) {
    SUPER_STATE.lock().unwrap().remove(&handle);
}

/// `super.class(var) -> "super:" + var`
#[no_mangle]
pub extern "C" fn super_class(handle: u64, var_: *const c_char) -> *mut c_char {
    let state = SUPER_STATE.lock().unwrap();
    if !state.contains_key(&handle) {
        return std::ptr::null_mut();
    }
    let s = unsafe { CStr::from_ptr(var_).to_str().unwrap_or("") };
    let out = format!("super:{}", s);
    CString::new(out).unwrap().into_raw()
}

/// `super.return() -> handle as u32`
#[no_mangle]
pub extern "C" fn super_return(handle: u64) -> u32 {
    let state = SUPER_STATE.lock().unwrap();
    *state.get(&handle).unwrap_or(&0)
}
