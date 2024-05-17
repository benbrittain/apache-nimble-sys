use core::fmt::Debug;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, Ordering};

use bt_hci::cmd::controller_baseband::HostBufferSize;
use bt_hci::cmd::{AsyncCmd, Cmd, SyncCmd};
use bt_hci::controller::{CmdError, ControllerCmdAsync, ControllerCmdSync};
use bt_hci::data::{AclPacket, AclPacketHeader};
use bt_hci::event::{CommandComplete, Event, EventPacketHeader};
use bt_hci::param::Error;
use bt_hci::{ControllerToHostPacket, FromHciBytes, PacketKind, ReadHci, WriteHci};
use defmt::{error, trace, Debug2Format};
use embassy_futures::{join, yield_now};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;

use crate::{raw, ready_to_send, OsError, OsMbuf};

#[cfg(not(feature = "host"))]
#[no_mangle]
extern "C" fn ble_transport_to_hs_evt_impl(buf: *mut cty::c_void) -> cty::c_int {
    let ptr = buf as *const raw::ble_hci_ev;
    let len = unsafe { (*ptr).length as usize };

    let mut data = [0; HCI_PKT_BUF_SIZE];

    unsafe {
        data[0] = (*ptr).opcode;
        data[1] = (*ptr).length;
        data[2..(2 + len)].copy_from_slice((*ptr).data.as_slice(len));
    }

    // ignore no-op event from the controller
    if let Ok((
        ControllerToHostPacket::Event(Event::CommandComplete(CommandComplete {
            cmd_opcode, ..
        })),
        _,
    )) = ControllerToHostPacket::from_hci_bytes_with_kind(PacketKind::Event, &data)
    {
        if cmd_opcode.to_raw() == 0 {
            return 0;
        }
    }

    READ_CHANNEL.try_send(PacketKind::Event).map_or_else(
        |_| {
            error!("event to host being overwritten. this should not happen.");
            OsError::NoMem as i32
        },
        |_| unsafe {
            READ_BUFFER.copy_from_slice(&data);
            0
        },
    )
}

#[cfg(not(feature = "host"))]
#[no_mangle]
extern "C" fn ble_transport_to_hs_acl_impl(om: *mut raw::os_mbuf) -> cty::c_int {
    READ_CHANNEL.try_send(PacketKind::AclData).map_or_else(
        |_| {
            error!("acl to host being overwritten. this should not happen.");
            OsError::NoMem as i32
        },
        |_| unsafe {
            READ_BUFFER.fill(0);
            let parse_result =
                AclPacketHeader::read_hci::<OsMbuf>(om.into(), READ_BUFFER.as_mut_slice())
                    .map_err(|_| OsError::Invalid)
                    .and_then(|hdr| {
                        let buf = &mut READ_BUFFER[hdr.size()..(hdr.size() + hdr.data_len())];
                        <OsMbuf as embedded_io::Read>::read_exact(&mut om.into(), buf)
                            .map_err(|_| OsError::Invalid)?;
                        AclPacket::from_header_hci_bytes(hdr, buf).map_err(|_| OsError::Invalid)
                    });

            if let Err(e) = parse_result {
                error!(
                    "could not fully parse ACL packet to send to host. dropping packet: {}",
                    Debug2Format(&e)
                );
                return OsError::NoMem as i32;
            };
            raw::os_mbuf_free_chain(om);
            0
        },
    )
}

// This isn't used in the controller
// #[no_mangle]
// extern "C" fn ble_transport_to_hs_iso_impl(om: *mut raw::os_mbuf) -> cty::c_int {
//     unimplemented!()
// }

extern "C" {
    static mut g_ble_ll_tx_power: cty::int8_t;
    fn ble_ll_tx_power_round(a: cty::c_int) -> cty::c_int;
}

/// Reimplementation of nimble's ble_ll_task.
/// [`NimbleController::new`] instead.
async unsafe fn ble_ll_task() -> ! {
    if raw::ble_phy_init() != 0 {
        panic!("could not initialize phy")
    };

    g_ble_ll_tx_power = ble_ll_tx_power_round(u32::min(
        raw::MYNEWT_VAL_BLE_LL_TX_PWR_DBM,
        raw::MYNEWT_VAL_BLE_LL_TX_PWR_MAX_DBM,
    ) as i32) as i8;

    // Note: we can't update g_ble_ll_tx_power_phy_current like in the original function, because
    // it's a C static. Hopefully it's not an issue :)

    loop {
        let ev = raw::ble_npl_eventq_get(&mut raw::g_ble_ll_data.ll_evq as _, u32::MAX).await;

        // we want to wait until the read channel is empty and all command responses have been
        // processed before we run an event, otherwise we risk overwriting data that hasn't
        // been received by the host yet.
        join::join(ready_to_send(&READ_CHANNEL).await, async {
            while let Some(()) = CMD_SIGNAL.try_take() {
                CMD_SIGNAL.signal(());
                yield_now().await;
            }
        })
        .await;

        raw::ble_npl_event_run(ev);
    }
}

pub struct NimbleController {
    cmd_lock: Mutex<CriticalSectionRawMutex, ()>,
}

pub struct NimbleControllerTask {
    _init: (),
}

impl NimbleControllerTask {
    pub async fn run(&self) -> ! {
        unsafe { ble_ll_task().await }
    }
}

const HCI_PKT_BUF_SIZE: usize = raw::BLE_ACL_MAX_PKT_SIZE as usize
    + raw::BLE_HCI_DATA_HDR_SZ as usize
    + size_of::<raw::os_mbuf_pkthdr>()
    + size_of::<raw::ble_mbuf_hdr>()
    + size_of::<raw::os_mbuf>();

static NIMBLE_CONTROLLER_IN_USE: AtomicBool = AtomicBool::new(false);
static mut READ_BUFFER: [u8; HCI_PKT_BUF_SIZE] = [0; HCI_PKT_BUF_SIZE];
static READ_CHANNEL: Channel<CriticalSectionRawMutex, PacketKind, 1> = Channel::new();
static CMD_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

impl NimbleController {
    pub fn new() -> Self {
        NIMBLE_CONTROLLER_IN_USE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .expect("attempted to create more than one nimble controller task");

        Self {
            cmd_lock: Mutex::new(()),
        }
    }

    pub fn create_task(&self) -> NimbleControllerTask {
        NimbleControllerTask { _init: () }
    }

    /// This is a little hack for now to get this to work with the `trouble` crate
    fn transform<C: Cmd>(&self, _cmd: &C, cmd_data: &mut [u8]) {
        if C::OPCODE.to_raw() == HostBufferSize::OPCODE.to_raw() {
            // position of host_sync_data_packet_len
            cmd_data[5] = 0;
            // position of host_sync_data_packets
            cmd_data[8] = 0;
        }
    }

    /// Send a command to be queued on the nimble controller's event queue. It will eventually be
    /// consumed by [`ble_ll_task`], and send a response back to the host via
    /// [`ble_transport_to_hs_evt_impl`].
    ///
    /// To wait for a response, we use [`CMD_SIGNAL`]. This only works with the assumption that the
    /// nimble controller can only handle one command at a time.
    async fn execute_command<C: Cmd + Debug>(
        &self,
        cmd: &C,
    ) -> Result<Event<'_>, CmdError<OsError>> {
        // allocate space for cmd
        let _lock = self.cmd_lock.lock().await;
        let ptr = unsafe { raw::ble_transport_alloc_cmd() };
        if core::ptr::eq(ptr, core::ptr::null_mut()) {
            return Err(CmdError::Io(OsError::NoMem));
        }
        trace!("sending cmd: {}", Debug2Format(cmd));

        // serialize cmd
        let cmd_data = unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, cmd.size()) };
        if let Err(e) = cmd.write_hci(cmd_data) {
            error!("failed to convert cmd into raw bytes: {}", Debug2Format(&e));
            unsafe { raw::ble_transport_free(ptr) };
            return Err(CmdError::Io(OsError::NoMem));
        }
        self.transform(cmd, unsafe {
            core::slice::from_raw_parts_mut(ptr as *mut u8, cmd.size())
        });

        // queue the command in the nimble controller
        let ret = unsafe { raw::ble_transport_to_ll_cmd_impl(ptr) };
        if ret != 0 {
            error!("failed to queue command, dropping command");
            unsafe { raw::ble_transport_free(ptr) };
            return Err(CmdError::Io(OsError::Invalid));
        }

        // wait until we receive a status or command complete
        CMD_SIGNAL.wait().await;

        // free the buffer to let other commands run
        unsafe { raw::ble_transport_free(ptr) };

        // parse the response data
        let hdr = match EventPacketHeader::from_hci_bytes(unsafe { READ_BUFFER.as_slice() }) {
            Ok((hdr, _)) => hdr,
            Err(e) => {
                error!(
                    "unexpected error when parsing event header: {}",
                    Debug2Format(&e),
                );
                return Err(CmdError::Hci(Error::INVALID_HCI_PARAMETERS));
            }
        };
        let buf =
            unsafe { &READ_BUFFER[..(size_of::<EventPacketHeader>() + hdr.params_len as usize)] };
        Event::from_hci_bytes(buf)
            .map(|p| {
                trace!("response data: cmd {} response bytes {}", 
                    Debug2Format(cmd),
                    buf
                );
                p.0
            })
            .map_err(|e| {
                error!(
                    "unexpected error when parsing command response data: cmd {} error {} response bytes {}",
                    Debug2Format(cmd),
                    Debug2Format(&e),
                    buf
                );
                CmdError::Hci(Error::INVALID_HCI_PARAMETERS)
            })
    }
}

impl Default for NimbleController {
    fn default() -> Self {
        Self::new()
    }
}

impl bt_hci::controller::Controller for NimbleController {
    type Error = OsError;

    async fn write_acl_data(
        &self,
        packet: &bt_hci::data::AclPacket<'_>,
    ) -> Result<(), Self::Error> {
        trace!("sending acl to controller");
        unsafe {
            let om = raw::ble_transport_alloc_acl_from_hs();

            if om.is_null() {
                error!("could not allocate space for an acl packet to send to controller");
                return Err(OsError::NoMem);
            }

            if let Err(e) = packet.write_hci::<OsMbuf>(om.into()) {
                error!(
                    "could not serialize acl packet: acl {} error {}",
                    Debug2Format(&packet),
                    Debug2Format(&e)
                );
                raw::os_mbuf_free_chain(om);
                return Err(e);
            };

            let ret = raw::ble_transport_to_ll_acl_impl(om);
            if ret != 0 {
                error!(
                    "controller did not handle acl data successfully: acl {} error {}",
                    Debug2Format(&packet),
                    ret
                );
                raw::os_mbuf_free_chain(om);
            }
        }
        Ok(())
    }

    async fn write_sync_data(
        &self,
        _packet: &bt_hci::data::SyncPacket<'_>,
    ) -> Result<(), Self::Error> {
        unimplemented!()
    }

    async fn write_iso_data(
        &self,
        packet: &bt_hci::data::IsoPacket<'_>,
    ) -> Result<(), Self::Error> {
        trace!("sending iso to controller");
        unsafe {
            let om = raw::ble_transport_alloc_iso_from_hs();
            if let Err(e) = packet.write_hci::<OsMbuf>(om.into()) {
                error!(
                    "could not serialize iso packet: iso {} error {}",
                    Debug2Format(&packet),
                    Debug2Format(&e)
                );
                raw::os_mbuf_free_chain(om);
                return Err(e);
            };

            let ret = raw::ble_transport_to_ll_iso_impl(om);
            if ret != 0 {
                error!(
                    "controller did not handle iso data successfully: iso {} error {}",
                    Debug2Format(&packet),
                    ret
                );
                raw::os_mbuf_free_chain(om);
            }
        }
        Ok(())
    }

    async fn read<'a>(
        &self,
        buf: &'a mut [u8],
    ) -> Result<bt_hci::ControllerToHostPacket<'a>, Self::Error> {
        let len = buf.len();
        loop {
            // This should be safe because references to buf aren't being carried across loop iterations
            let buf = unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr(), len) };
            let kind = READ_CHANNEL.receive().await;
            // Access to mutable static buffer should be safe since it's guarded by a channel with a
            // capacity of 1. The buffer data must be consumed here before any new writes can happen to
            // the buffer.
            unsafe {
                buf.copy_from_slice(&READ_BUFFER[..len]);
            }
            match ControllerToHostPacket::from_hci_bytes_with_kind(kind, buf) {
                Ok((ControllerToHostPacket::Event(Event::CommandComplete(_)), _))
                | Ok((ControllerToHostPacket::Event(Event::CommandStatus(_)), _)) => {
                    CMD_SIGNAL.signal(());
                    continue;
                }
                Ok(value) => {
                    trace!("reading packet from controller: {}", Debug2Format(&value.0));
                    return Ok(value.0);
                }
                Err(e) => {
                    error!("error reading packet from controller: {}", Debug2Format(&e));
                    return Err(OsError::Invalid);
                }
            }
        }
    }
}

impl<C: SyncCmd + Debug> ControllerCmdSync<C> for NimbleController
where
    C::Return: Debug,
{
    async fn exec(
        &self,
        cmd: &C,
    ) -> Result<<C as SyncCmd>::Return, bt_hci::controller::CmdError<Self::Error>> {
        let response = self.execute_command(cmd).await?;

        match response {
            Event::CommandComplete(c) => {
                if c.cmd_opcode == C::OPCODE {
                    c.to_result::<C>()
                        .map(|r| {
                            trace!(
                                "command successfully returned. cmd {} response {}",
                                Debug2Format(&cmd),
                                Debug2Format(&r)
                            );
                            r
                        })
                        .map_err(|e| {
                            error!(
                                "command responded with an error: cmd {} response {}",
                                Debug2Format(&cmd),
                                Debug2Format(&e)
                            );
                            CmdError::Hci(e)
                        })
                } else {
                    error!(
                        "received response for unrelated command. intended command: {} received event: {}",
                        Debug2Format(&cmd),
                        Debug2Format(&c)
                    );
                    Err(CmdError::Io(OsError::InvalidParameter))
                }
            }
            r => {
                error!(
                    "unexpected response when executing sync ble cmd: {} response: {}",
                    Debug2Format(&cmd),
                    Debug2Format(&r)
                );
                Err(CmdError::Io(OsError::InvalidParameter))
            }
        }
    }
}

impl<C: AsyncCmd + Debug> ControllerCmdAsync<C> for NimbleController {
    async fn exec(&self, cmd: &C) -> Result<(), CmdError<Self::Error>> {
        let response = self.execute_command(cmd).await?;

        match response {
            Event::CommandStatus(c) => {
                if c.cmd_opcode.to_raw() == C::OPCODE.to_raw() {
                    trace!(
                        "async command successfully started: cmd {} response {}",
                        Debug2Format(&cmd),
                        Debug2Format(&c)
                    );
                    Ok(())
                } else {
                    error!(
                        "received response for unrelated command. intended cmd {} received response {}",
                        Debug2Format(&cmd),
                        Debug2Format(&c)
                    );
                    Err(CmdError::Io(OsError::InvalidParameter))
                }
            }
            r => {
                error!(
                    "unexpected response when executing async ble cmd: cmd {} response {}",
                    Debug2Format(&cmd),
                    Debug2Format(&r),
                );
                Err(CmdError::Io(OsError::InvalidParameter))
            }
        }
    }
}
