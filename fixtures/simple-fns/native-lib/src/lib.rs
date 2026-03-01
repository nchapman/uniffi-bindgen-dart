use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU32, Ordering};

static TICK_COUNT: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn add(left: u32, right: u32) -> u32 {
    left + right
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
pub extern "C" fn rust_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(value);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn negate(value: i32) -> i32 {
    -value
}

#[unsafe(no_mangle)]
pub extern "C" fn is_even(value: i32) -> bool {
    value % 2 == 0
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
