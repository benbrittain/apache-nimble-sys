use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{RawWaker, RawWakerVTable, Waker};

use critical_section::{acquire, release, RestoreState};
use defmt::{error, trace};
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::channel::Channel;
use embassy_sync::waitqueue::AtomicWaker;
use embassy_time::Timer;
use embassy_time_driver::{now, schedule_wake};

use crate::driver;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// NOTE: try not to use defmt log macros in functions that can be called from an interrupt context.
// This can cause the program to crash. I suspect that when the `critical-section` feature is
// enabled, the global logger can be taken reentrantly. For example, if a RADIO interrupt handler
// is triggered while a task is calling a defmt macro like `info!`, and another defmt log occurs
// within the interrupt handler, then defmt will panic due to it being used reentrantly.

// Init

#[no_mangle]
pub extern "C" fn ble_npl_os_started() -> bool {
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn ble_npl_get_current_task_id(evq: *mut ble_npl_eventq) -> *mut () {
    unimplemented!()
}

// Event Queue

struct EventQueueState {
    q: Channel<CriticalSectionRawMutex, *mut ble_npl_event, 16>,
    taken: AtomicBool,
}

const EQ: EventQueueState = EventQueueState {
    q: Channel::new(),
    taken: AtomicBool::new(false),
};

static mut EQ_POOL: [EventQueueState; 8] = [EQ; 8];

#[repr(C)]
#[no_mangle]
pub struct ble_npl_eventq {
    ch: &'static EventQueueState,
    len: u8,
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_eventq_init(evq: *mut ble_npl_eventq) {
    // trace!("eventq init: {}", evq);
    evq.write_bytes(0, 1);
    if let Some(q) = EQ_POOL.iter().find(|q| {
        q.taken
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }) {
        (*evq).ch = q;
        (*evq).len = 0;
    } else {
        panic!("no more event queues")
    };
}

/// Async replacement for nimble's ble_npl_eventq_get function.
///
/// We need to yield / context switch to other tasks from this function. Normally, this would be an
/// `extern "C"` function like the rest of the port layer functions, but unfortunately there isn't
/// a way (AFAIK) to yield for other tasks in a non-async function. As a result, functions that
/// call to `ble_npl_eventq_get` in the nimble code will need to be re-written in rust to be async.
/// (not a lot thankfully)
pub async unsafe fn ble_npl_eventq_get(
    evq: *mut ble_npl_eventq,
    tmo: ble_npl_time_t,
) -> *mut ble_npl_event {
    // trace!("eventq get: {}", evq);

    // it can be possible for the event queue to be in a state where there is always something to
    // dequeue between iterations, which causes other tasks to not be able to run.
    embassy_futures::yield_now().await;

    let ev = if (tmo == 0) {
        (*evq).ch.q.try_receive().unwrap_or(core::ptr::null_mut())
    } else if (tmo == u32::MAX) {
        (*evq).ch.q.receive().await
    } else {
        match select((*evq).ch.q.receive(), Timer::after_millis(tmo as u64)).await {
            Either::First(ptr) => ptr,
            Either::Second(_) => core::ptr::null_mut(),
        }
    };

    if !ev.is_null() {
        (*ev).queued = false;
        (*evq).len -= 1;
    }

    ev
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_eventq_put(evq: *mut ble_npl_eventq, ev: *mut ble_npl_event) {
    // trace!("eventq put: evq {} ev {}", evq, ev);

    (*ev).queued = true;
    (*evq).len += 1;
    if let Err(e) = (*evq).ch.q.try_send(ev) {
        panic!("event queue error: {:?}", e)
    };
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_eventq_remove(evq: *mut ble_npl_eventq, ev: *mut ble_npl_event) {
    // trace!("eventq remove: evq {} ev {}", evq, ev);

    // for the same reason as the FreeRTOS port, we have no way of picking out a specific item from
    // the channel, so we just remove everything and put them back, except for the item we want to
    // remove.
    driver::cs_internal::with_fn(|| {
        for _ in 0..(*evq).len {
            let pulled = (*evq).ch.q.try_receive().unwrap();

            if core::ptr::eq(pulled, ev) {
                (*ev).queued = false;
                (*evq).len -= 1;
                continue;
            }

            if let Err(e) = (*evq).ch.q.try_send(pulled) {
                panic!(
                    "unexpected error when putting events back into the queue: {:?}",
                    e
                )
            };
        }
    });
}

// Events

#[repr(C)]
#[no_mangle]
pub struct ble_npl_event {
    event_fn: *mut ble_npl_event_fn,
    arg_ptr: *mut (),
    queued: bool,
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_event_run(ev: *mut ble_npl_event) {
    // trace!("event run: {}", ev);
    let event_fn = core::mem::transmute::<_, ble_npl_event_fn>((*ev).event_fn);
    if let Some(handler) = event_fn {
        handler(ev)
    }
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_event_init(
    ev: *mut ble_npl_event,
    fn_: *mut ble_npl_event_fn,
    arg: *mut (),
) {
    trace!("event init: {}", ev);
    ev.write_bytes(0, 1);
    (*ev).queued = false;
    (*ev).arg_ptr = arg;
    (*ev).event_fn = fn_;
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_event_is_queued(ev: *mut ble_npl_event) -> bool {
    // trace!("is_queued: {}", ev);
    (*ev).queued
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_event_get_arg(ev: *mut ble_npl_event) -> *mut () {
    // trace!("get_arg: {}", ev);
    (*ev).arg_ptr
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_event_set_arg(ev: *mut ble_npl_event, arg: *mut ()) {
    // trace!("set_arg: {}", ev);
    (*ev).arg_ptr = arg;
}

// Mutexes

#[repr(C)]
#[no_mangle]
pub struct ble_npl_mutex {
    dummy: &'static (),
}

#[no_mangle]
pub extern "C" fn ble_npl_mutex_init(mu: *mut ble_npl_mutex) -> ble_npl_error_t {
    // trace!("mutex init");
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn ble_npl_mutex_pend(
    mu: *mut ble_npl_mutex,
    timeout: ble_npl_time_t,
) -> ble_npl_error_t {
    // trace!("mutex pend");
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn ble_npl_mutex_release(mu: *mut ble_npl_mutex) -> ble_npl_error_t {
    // trace!("mutex release");
    unimplemented!()
}

// Semaphores

#[repr(C)]
#[no_mangle]
pub struct ble_npl_sem {
    dummy: &'static (),
}

#[no_mangle]
pub extern "C" fn ble_npl_sem_init(sem: *mut ble_npl_sem, tokens: u16) -> ble_npl_error_t {
    // trace!("sem init");
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn ble_npl_sem_pend(
    sem: *mut ble_npl_sem,
    timeout: ble_npl_time_t,
) -> ble_npl_error_t {
    // trace!("sem pend");
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn ble_npl_sem_release(sem: *mut ble_npl_sem) -> ble_npl_error_t {
    // trace!("sem release");
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn ble_npl_sem_get_count(sem: *mut ble_npl_sem) -> u16 {
    // trace!("sem count");
    unimplemented!()
}

// Callouts

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, callout_cb, callout_cb, |_| {});

#[repr(C)]
#[no_mangle]
pub struct ble_npl_callout {
    active: bool,
    expires_at: u32,
    event_queue: *mut ble_npl_eventq,
    event: ble_npl_event,
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_callout_init(
    c: *mut ble_npl_callout,
    evq: *mut ble_npl_eventq,
    ev_cb: *mut ble_npl_event_fn,
    ev_arg: *mut (),
) {
    // trace!(
    //     "callout init: co {} evq {} cb {} arg {}",
    //     c,
    //     evq,
    //     ev_cb,
    //     ev_arg
    // );
    c.write_bytes(0, 1);
    (*c).active = false;
    (*c).event_queue = evq;
    ble_npl_event_init(&mut (*c).event as _, ev_cb, ev_arg);
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_callout_reset(
    c: *mut ble_npl_callout,
    ticks: ble_npl_time_t,
) -> ble_npl_error_t {
    // trace!("callout reset: {}", c);
    (*c).expires_at = now() as u32 + ticks;
    schedule_wake(
        (*c).expires_at as u64,
        &Waker::from_raw(RawWaker::new(c as _, &VTABLE)),
    );
    (*c).active = true;
    ble_npl_error_BLE_NPL_ENOENT
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_callout_stop(co: *mut ble_npl_callout) {
    // trace!("callout stop: {}", co);
    (*co).active = false;
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_callout_is_active(c: *mut ble_npl_callout) -> bool {
    // trace!("callout is active: {}", c);
    (*c).active
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_callout_get_ticks(co: *mut ble_npl_callout) -> ble_npl_time_t {
    // trace!("callout ticks: {}", co);
    (*co).expires_at
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_callout_remaining_ticks(
    co: *mut ble_npl_callout,
    time: ble_npl_time_t,
) -> ble_npl_time_t {
    (time - (*co).expires_at)
}

fn clone_waker(data: *const ()) -> RawWaker {
    RawWaker::new(data, &VTABLE)
}

fn callout_cb(ctx: *const ()) {
    unsafe {
        let co: *mut ble_npl_callout = ctx as *mut ble_npl_callout;
        if ((*co).active) {
            ble_npl_eventq_put((*co).event_queue, &mut (*co).event as _)
        }
        (*co).active = false;
    }
}

// Timing

#[no_mangle]
pub extern "C" fn ble_npl_time_get() -> u32 {
    // trace!("time get");
    now() as u32
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_time_ms_to_ticks(
    ms: u32,
    out_ticks: *mut ble_npl_time_t,
) -> ble_npl_error_t {
    // trace!("time ms to ticks");
    *out_ticks = embassy_time::Instant::from_millis(ms as u64).as_ticks() as u32;
    ble_npl_error_BLE_NPL_OK
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_time_ticks_to_ms(
    ticks: ble_npl_time_t,
    out_ms: *mut u32,
) -> ble_npl_error_t {
    // trace!("time ticks to ms");
    *out_ms = embassy_time::Instant::from_ticks(ticks as u64).as_millis() as u32;
    ble_npl_error_BLE_NPL_OK
}

#[no_mangle]
pub extern "C" fn ble_npl_time_ms_to_ticks32(ms: u32) -> ble_npl_time_t {
    // trace!("time ms to ticks 32");
    embassy_time::Instant::from_millis(ms as u64).as_ticks() as u32
}

#[no_mangle]
pub extern "C" fn ble_npl_time_ticks_to_ms32(ticks: ble_npl_time_t) -> u32 {
    // trace!("time ticks to ms 32");
    embassy_time::Instant::from_ticks(ticks as u64).as_millis() as u32
}

/// Used to set up interrupt handlers. This is only really used for the controller driver.
#[no_mangle]
pub extern "C" fn ble_npl_hw_set_isr(
    irqn: cty::c_int,
    addr: ::core::option::Option<unsafe extern "C" fn()>,
) {
    driver::set_isr(irqn, addr);
}

// Critical Section

/// Note: it's possible for nimble to created nested critical sections. If we are trying to
/// `acquire` again when we are already in a critical section, we return 1, so that the
/// corresponding `release` call doesn't end the critical section early. We assume single core
/// usage.
#[no_mangle]
pub unsafe extern "C" fn ble_npl_hw_enter_critical() -> u32 {
    if driver::cs_internal::acquire() {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn ble_npl_hw_exit_critical(ctx: u32) {
    driver::cs_internal::release(ctx == 1);
}

#[no_mangle]
pub extern "C" fn ble_npl_hw_is_in_critical() -> bool {
    driver::cs_internal::CS_FLAG.load(Ordering::Relaxed)
}

// newlib
// These are only relevant since we need to link libc/newlib later on. (See build.rs for
// apache-nimble)

// #[no_mangle]
// pub extern "C" fn _sbrk() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _write() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _close() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _lseek() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _read() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _fstat() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _isatty() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _exit() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _open() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _kill() {
//     unimplemented!()
// }

// #[no_mangle]
// pub extern "C" fn _getpid() {
//     unimplemented!()
// }
