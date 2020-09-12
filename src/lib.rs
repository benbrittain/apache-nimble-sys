#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

mod interop {
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
