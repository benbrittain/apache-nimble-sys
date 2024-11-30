#![no_std]

use core::future::Future;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

pub use apache_nimble_sys as raw;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Channel;

#[cfg(feature = "controller")]
pub mod controller;

#[cfg(feature = "host")]
pub mod host;

extern "C" {
    pub(crate) fn ble_ll_init();
    fn os_msys_init();
    fn os_mempool_module_init();
}

static NIMBLE_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn initialize_nimble() {
    NIMBLE_INITIALIZED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .expect("attempted to initialize nimble more than once");

    unsafe {
        #[cfg(feature = "controller")]
        {
            ble_ll_init();
        }

        #[cfg(feature = "host")]
        {
            raw::ble_npl_eventq_init(core::ptr::addr_of!(host::DEFLT_EVQ) as *mut _);
        }
        os_mempool_module_init();
        os_msys_init();
        raw::ble_transport_init();

        #[cfg(feature = "host")]
        {
            raw::ble_transport_hs_init();
        }

        #[cfg(feature = "controller")]
        {
            raw::hal_timer_init(5, core::ptr::null_mut());
            raw::os_cputime_init(32768);
            raw::ble_transport_ll_init();
        }
    }
}

async fn ready_to_send<M: RawMutex, T, const N: usize>(
    ch: &Channel<M, T, N>,
) -> ReadyToSend<'_, M, T, N> {
    ReadyToSend { ch }
}

struct ReadyToSend<'ch, M: RawMutex, T, const N: usize> {
    ch: &'ch Channel<M, T, N>,
}

impl<'ch, M: RawMutex, T, const N: usize> Future for ReadyToSend<'ch, M, T, N> {
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        self.ch.poll_ready_to_send(cx)
    }
}

#[derive(Debug)]
#[repr(u32)]
pub enum OsError {
    NoMem = raw::os_error_OS_ENOMEM,
    Invalid = raw::os_error_OS_EINVAL,
    InvalidParameter = raw::os_error_OS_INVALID_PARM,
    NotAligned = raw::os_error_OS_MEM_NOT_ALIGNED,
    BadMutex = raw::os_error_OS_BAD_MUTEX,
    Timeout = raw::os_error_OS_TIMEOUT,
    ErrInISR = raw::os_error_OS_ERR_IN_ISR,
    ErrPriviliged = raw::os_error_OS_ERR_PRIV,
    NotStarted = raw::os_error_OS_NOT_STARTED,
    NoEnt = raw::os_error_OS_ENOENT,
    Busy = raw::os_error_OS_EBUSY,
    Error = raw::os_error_OS_ERROR,
}

impl From<u32> for OsError {
    fn from(value: u32) -> Self {
        match value {
            raw::os_error_OS_ENOMEM => OsError::NoMem,
            raw::os_error_OS_EINVAL => OsError::Invalid,
            raw::os_error_OS_INVALID_PARM => OsError::InvalidParameter,
            raw::os_error_OS_MEM_NOT_ALIGNED => OsError::NotAligned,
            raw::os_error_OS_BAD_MUTEX => OsError::BadMutex,
            raw::os_error_OS_TIMEOUT => OsError::Timeout,
            raw::os_error_OS_ERR_IN_ISR => OsError::ErrInISR,
            raw::os_error_OS_ERR_PRIV => OsError::ErrPriviliged,
            raw::os_error_OS_NOT_STARTED => OsError::NotStarted,
            raw::os_error_OS_ENOENT => OsError::NoEnt,
            raw::os_error_OS_EBUSY => OsError::Busy,
            raw::os_error_OS_ERROR => OsError::Error,
            _ => OsError::Error,
        }
    }
}

impl embedded_io::Error for OsError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::WriteZero
    }
}

struct OsMbuf(*mut raw::os_mbuf);

impl From<*mut raw::os_mbuf> for OsMbuf {
    fn from(value: *mut raw::os_mbuf) -> Self {
        OsMbuf(value)
    }
}

impl embedded_io::ErrorType for OsMbuf {
    type Error = OsError;
}

impl embedded_io::Write for OsMbuf {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let amt = buf.len();
        let ret =
            unsafe { raw::os_mbuf_append(self.0, buf.as_ptr() as *const cty::c_void, amt as u16) };
        if ret == 0 {
            Ok(amt)
        } else {
            Err(OsError::from(ret as u32))
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_io::Read for OsMbuf {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let len = buf.len();
        unsafe {
            if raw::os_mbuf_copydata(self.0, 0, len as i32, buf.as_mut_ptr() as *mut cty::c_void)
                == -1
            {
                // Could not read all bytes
                return Err(OsError::NoMem);
            };
            raw::os_mbuf_adj(self.0, len as i32);
        }
        Ok(len)
    }
}
