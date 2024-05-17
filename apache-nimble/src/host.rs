use core::mem::MaybeUninit;
use core::ptr::addr_of;

use crate::raw;

#[cfg(not(feature = "controller"))]
#[no_mangle]
extern "C" fn ble_transport_to_ll_acl_impl() {
    unimplemented!()
}

#[cfg(not(feature = "controller"))]
#[no_mangle]
extern "C" fn ble_transport_to_ll_cmd_impl() {
    unimplemented!()
}

// #[cfg(not(feature = "controller"))]
// #[no_mangle]
// extern "C" fn ble_transport_to_ll_iso_impl() {
//     unimplemented!()
// }

pub(crate) static mut DEFLT_EVQ: MaybeUninit<raw::ble_npl_eventq> = MaybeUninit::uninit();

pub async unsafe fn nimble_port_run() -> ! {
    loop {
        let ev = raw::ble_npl_eventq_get(addr_of!(DEFLT_EVQ) as *mut _, u32::MAX).await;
        raw::ble_npl_event_run(ev);
    }
}

#[no_mangle]
extern "C" fn nimble_port_get_dflt_eventq() -> *mut raw::ble_npl_eventq {
    unsafe { addr_of!(DEFLT_EVQ) as *mut _ }
}
