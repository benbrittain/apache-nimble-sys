#![no_std]
#![allow(unused)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(improper_ctypes)]

#[cfg(not(any(feature = "port-layer-embassy")))]
compile_error!("Please choose a port layer to use.");

#[repr(C)]
pub(crate) enum COption<T> {
    Some(T),
    None,
}

#[cfg_attr(feature = "nrf52840", path = "drivers/nrf5x.rs")]
mod driver;

// Note: can't use cfg_attr for the port layers since cbindgen won't be able to parse it

#[cfg(feature = "port-layer-embassy")]
#[path = "port-layers/embassy.rs"]
mod embassy_port;

#[cfg(feature = "port-layer-embassy")]
pub use embassy_port::*;

mod interop {
    #[cfg(feature = "host")]
    extern crate alloc;

    #[cfg(feature = "host")]
    use alloc::alloc::{alloc_zeroed, dealloc, realloc as alloc_realloc, Layout};

    #[cfg(feature = "host")]
    #[no_mangle]
    unsafe extern "C" fn free(ptr: *mut u8) {
        let layout = Layout::new::<u16>();
        dealloc(ptr, layout);
    }

    #[cfg(feature = "host")]
    #[no_mangle]
    unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
        let layout = Layout::from_size_align(size, 16).unwrap();
        let new_ptr = alloc_realloc(ptr, layout, size);
        new_ptr
    }

    #[cfg(feature = "host")]
    #[no_mangle]
    unsafe extern "C" fn malloc(amt: usize) -> *mut u8 {
        let layout = Layout::from_size_align(amt, 16).unwrap();
        let ptr = alloc_zeroed(layout);
        ptr
    }

    #[no_mangle]
    unsafe extern "C" fn __assert_func(
        file: *const cty::c_char,
        line: cty::c_int,
        func: *const cty::c_char,
        expr: *const cty::c_char,
    ) {
        let str_file = core::ffi::CStr::from_ptr(file as _).to_str().unwrap();
        let str_func = core::ffi::CStr::from_ptr(func as _).to_str().unwrap();
        let expr = core::ffi::CStr::from_ptr(expr as _).to_str().unwrap();
        panic!(
            "assertion ({}) failed in {} - {}:{}",
            expr, str_func, str_file, line
        );
    }

    #[no_mangle]
    unsafe extern "C" fn strlen(s: *const cty::c_char) -> cty::c_uint {
        let c_str = core::ffi::CStr::from_ptr(s as _);
        c_str.to_bytes().len() as u32
    }
}
