#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

extern crate alloc;

mod interop {
    use alloc::alloc::{alloc_zeroed, dealloc, realloc as alloc_realloc, Layout};

    #[no_mangle]
    pub unsafe fn free(ptr: *mut u8) {
        let layout = Layout::new::<u16>();
        dealloc(ptr, layout);
    }

    #[no_mangle]
    pub unsafe fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
        let layout = Layout::from_size_align(size, 16).unwrap();
        let new_ptr = alloc_realloc(ptr, layout, size);
        new_ptr
    }

    #[no_mangle]
    pub unsafe fn malloc(amt: usize) -> *mut u8 {
        let layout = Layout::from_size_align(amt, 16).unwrap();
        let ptr = alloc_zeroed(layout);
        ptr
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
