#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod interop {
    #[no_mangle]
    pub fn free(ptr: *mut cty::c_void) {}

    #[no_mangle]
    pub fn malloc(amt: cty::size_t) -> *mut cty::c_void {
        core::ptr::null_mut()
    }

    #[no_mangle]
    pub fn __assert_func(file: *const char, line: usize, func: *const char, expr: *const char) {
        unsafe {
            let str_file = CStr::from_ptr(file as *const _).to_str().unwrap();
            let str_func = CStr::from_ptr(func as *const _).to_str().unwrap();
            panic!("Assertion in {} - {}:{}", str_func, str_file, line);
        }
    }

    use cstr_core::CStr;

    #[no_mangle]
    pub fn strlen(__s: *const cty::c_char) -> cty::c_uint {
        unsafe {
            // so lazy
            let c_str = CStr::from_ptr(__s);
            c_str.to_bytes().len() as u32
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
