#![allow(unused)]

use apache_nimble_sys::*;

// Init

#[no_mangle]
extern "C" fn ble_npl_os_started() -> bool {
    false
}

// Task

#[no_mangle]
extern "C" fn ble_npl_get_current_task_id(evq: *mut ble_npl_eventq) -> *mut () {
    core::ptr::null_mut()
}

// Event Queue

#[no_mangle]
extern "C" fn ble_npl_eventq_init(evq: *mut ble_npl_eventq) {}

#[no_mangle]
extern "C" fn ble_npl_eventq_get(
    evq: *mut ble_npl_eventq,
    tmo: ble_npl_time_t,
) -> *mut ble_npl_event {
    core::ptr::null_mut()
}

#[no_mangle]
extern "C" fn ble_npl_eventq_put(evq: *mut ble_npl_eventq, ev: *mut ble_npl_event) {}

#[no_mangle]
extern "C" fn ble_npl_eventq_remove(evq: *mut ble_npl_eventq, ev: *mut ble_npl_event) {}

// Events

#[no_mangle]
extern "C" fn ble_npl_event_run(ev: *mut ble_npl_event) {}

#[no_mangle]
extern "C" fn ble_npl_event_init(ev: *mut ble_npl_event, fn_: *mut ble_npl_event_fn, arg: *mut ()) {
}

#[no_mangle]
extern "C" fn ble_npl_event_is_queued(ev: *mut ble_npl_event) -> bool {
    false
}

#[no_mangle]
extern "C" fn ble_npl_event_get_arg(ev: *mut ble_npl_event) -> *mut () {
    core::ptr::null_mut()
}

#[no_mangle]
extern "C" fn ble_npl_event_set_arg(ev: *mut ble_npl_event, arg: *mut ()) {}

// Mutexes

#[no_mangle]
extern "C" fn ble_npl_mutex_init(mu: *mut ble_npl_mutex) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_mutex_pend(
    mu: *mut ble_npl_mutex,
    timeout: ble_npl_time_t,
) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_mutex_release(mu: *mut ble_npl_mutex) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

// Semaphores

#[no_mangle]
extern "C" fn ble_npl_sem_init(sem: *mut ble_npl_sem, tokens: u16) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_sem_pend(sem: *mut ble_npl_sem, timeout: ble_npl_time_t) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_sem_release(sem: *mut ble_npl_sem) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_sem_get_count(sem: *mut ble_npl_sem) -> u16 {
    0
}

// Callouts

#[no_mangle]
extern "C" fn ble_npl_callout_init(
    c: *mut ble_npl_callout,
    evq: *mut ble_npl_eventq,
    ev_cb: *mut ble_npl_event_fn,
    ev_arg: *mut (),
) {
}

#[no_mangle]
extern "C" fn ble_npl_callout_reset(
    c: *mut ble_npl_callout,
    ticks: ble_npl_time_t,
) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_callout_stop(co: *mut ble_npl_callout) {}

#[no_mangle]
extern "C" fn ble_npl_callout_is_active(c: *mut ble_npl_callout) -> bool {
    false
}

#[no_mangle]
extern "C" fn ble_npl_callout_get_ticks(co: *mut ble_npl_callout) -> ble_npl_time_t {
    0
}

// Timing

#[no_mangle]
extern "C" fn ble_npl_time_get() -> u32 {
    0
}

#[no_mangle]
extern "C" fn ble_npl_time_ms_to_ticks(ms: u32, out_ticks: *mut ble_npl_time_t) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_time_ticks_to_ms(ticks: ble_npl_time_t, out_ms: *mut u32) -> ble_npl_error_t {
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
extern "C" fn ble_npl_time_ms_to_ticks32(ms: u32) -> ble_npl_time_t {
    0
}

#[no_mangle]
extern "C" fn ble_npl_time_ticks_to_ms32(ticks: ble_npl_time_t) -> u32 {
    0
}

// Critical Section

#[no_mangle]
extern "C" fn ble_npl_hw_enter_critical() -> u32 {
    0
}

#[no_mangle]
extern "C" fn ble_npl_hw_exit_critical(ctx: u32) {}
