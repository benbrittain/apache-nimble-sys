use nrf52840_pac as pac;

use defmt::{error, trace};
use pac::interrupt;

pub fn set_isr(irqn: cty::c_int, addr: ::core::option::Option<unsafe extern "C" fn()>) {
    match irqn {
        _ if irqn == pac::Interrupt::RADIO as i32 => unsafe { RADIO_HANDLER = addr },
        _ if irqn == pac::Interrupt::RNG as i32 => unsafe { RNG_HANDLER = addr },
        _ if irqn == pac::Interrupt::RTC0 as i32 => unsafe { RTC0_HANDLER = addr },
        _ => {
            error!("Nimble attempted to set unhandled irqn: {}", irqn);
        }
    }
}

static mut RADIO_HANDLER: ::core::option::Option<unsafe extern "C" fn()> = None;

#[interrupt]
unsafe fn RADIO() {
    if let Some(handler) = RADIO_HANDLER {
        handler()
    }
}

static mut RNG_HANDLER: ::core::option::Option<unsafe extern "C" fn()> = None;

#[interrupt]
unsafe fn RNG() {
    if let Some(handler) = RNG_HANDLER {
        handler()
    }
}

static mut RTC0_HANDLER: ::core::option::Option<unsafe extern "C" fn()> = None;

#[interrupt]
unsafe fn RTC0() {
    if let Some(handler) = RTC0_HANDLER {
        handler()
    }
}

/// This critical section is used internally by nimble through
/// [`crate::ble_npl_hw_enter_critical`], and [`crate::ble_npl_hw_exit_critical`]. The
/// implementation here disables all interrupts.
pub(crate) mod cs_internal {
    use core::arch::asm;
    use core::sync::atomic::{compiler_fence, AtomicBool, Ordering};

    pub static CS_FLAG: AtomicBool = AtomicBool::new(false);

    /// Returns true if active
    pub fn is_primask_active() -> bool {
        let primask: u32;
        unsafe {
            asm!("mrs {}, PRIMASK", out(reg) primask);
        }
        primask & 1 == 0
    }

    /// Safety: acquire calls must have a corresponding release, properly nested.
    pub unsafe fn acquire() -> bool {
        let active = is_primask_active();

        unsafe {
            asm!("cpsid i");
        }

        compiler_fence(Ordering::SeqCst);

        CS_FLAG.store(true, Ordering::Relaxed);

        active
    }

    /// Safety: release calls must have a corresponding acquire, properly nested.
    pub unsafe fn release(active: bool) {
        if active {
            CS_FLAG.store(false, Ordering::Relaxed);
        }

        compiler_fence(Ordering::SeqCst);

        if active {
            asm!("cpsie i");
        }
    }

    #[inline]
    pub unsafe fn with_fn<R>(f: impl FnOnce() -> R) -> R {
        let active = acquire();

        let r = f();

        release(active);

        r
    }
}

#[cfg(feature = "critical-section")]
/// Critical section implementation is adapted from `nrf-softdevice`, disabling all interrupts
/// except those used by the nimble controller. This implementation is intended to be used by
/// application code through the [`critical_section`] crate. This allows the reserved interrupts to
/// still run.
mod cs_impl {

    use core::arch::asm;
    use core::sync::atomic::{compiler_fence, AtomicBool, Ordering};

    use super::pac::{Interrupt, NVIC};

    const RESERVED_IRQS: u32 = (1 << (Interrupt::RADIO as u8))
        | (1 << (Interrupt::RTC0 as u8))
        | (1 << (Interrupt::RNG as u8));

    pub static CS_FLAG: AtomicBool = AtomicBool::new(false);
    static mut CS_MASK: [u32; 2] = [0; 2];

    struct CriticalSection;
    critical_section::set_impl!(CriticalSection);

    unsafe impl critical_section::Impl for CriticalSection {
        /// It can be possible to enter a nested critical section such that the outer CS uses the
        /// [`super::cs_internal`], and the inner CS uses this implementation. This can be
        /// problematic because we would be enabling
        unsafe fn acquire() -> bool {
            let nvic = &*NVIC::PTR;
            let nested_cs = CS_FLAG.load(Ordering::SeqCst);

            if !nested_cs {
                let in_internal_cs = super::cs_internal::CS_FLAG.load(Ordering::Relaxed);
                super::cs_internal::with_fn(|| {
                    if !in_internal_cs {
                        CS_FLAG.store(true, Ordering::Relaxed);
                        CS_MASK[0] = nvic.icer[0].read();
                        CS_MASK[1] = nvic.icer[1].read();
                        nvic.icer[0].write(!RESERVED_IRQS);
                        nvic.icer[1].write(0xFFFF_FFFF);
                    }
                });
            }

            compiler_fence(Ordering::SeqCst);

            nested_cs
        }

        unsafe fn release(nested_cs: bool) {
            compiler_fence(Ordering::SeqCst);

            let nvic = &*NVIC::PTR;
            if !nested_cs {
                let in_internal_cs = super::cs_internal::CS_FLAG.load(Ordering::Relaxed);
                super::cs_internal::with_fn(|| {
                    if !in_internal_cs {
                        CS_FLAG.store(false, Ordering::Relaxed);
                        nvic.iser[0].write(CS_MASK[0] & !RESERVED_IRQS);
                        nvic.iser[1].write(CS_MASK[1]);
                    }
                });
            }
        }
    }
}
