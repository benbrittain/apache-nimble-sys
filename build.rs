#![deny(warnings)]

use std::env;
use std::path::PathBuf;

extern crate cc;

fn main() {
    cc::Build::new()
        // host stack
        .file("mynewt-nimble/nimble/host/src/ble_att.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_misc.c")
        .file("mynewt-nimble/nimble/host/src/ble_att_clt.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_mqueue.c")
        .file("mynewt-nimble/nimble/host/src/ble_att_cmd.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_periodic_sync.c")
        .file("mynewt-nimble/nimble/host/src/ble_att_svr.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_pvcy.c")
        .file("mynewt-nimble/nimble/host/src/ble_eddystone.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_shutdown.c")
        .file("mynewt-nimble/nimble/host/src/ble_gap.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_startup.c")
        .file("mynewt-nimble/nimble/host/src/ble_gattc.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_stop.c")
        .file("mynewt-nimble/nimble/host/src/ble_gatts.c")
        .file("mynewt-nimble/nimble/host/src/ble_ibeacon.c")
        .file("mynewt-nimble/nimble/host/src/ble_gatts_lcl.c")
        .file("mynewt-nimble/nimble/host/src/ble_l2cap.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_adv.c")
        .file("mynewt-nimble/nimble/host/src/ble_l2cap_coc.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_atomic.c")
        .file("mynewt-nimble/nimble/host/src/ble_l2cap_sig.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs.c")
        .file("mynewt-nimble/nimble/host/src/ble_l2cap_sig_cmd.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_cfg.c")
        .file("mynewt-nimble/nimble/host/src/ble_monitor.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_conn.c")
        .file("mynewt-nimble/nimble/host/src/ble_sm_alg.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_flow.c")
        .file("mynewt-nimble/nimble/host/src/ble_sm.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_hci.c")
        .file("mynewt-nimble/nimble/host/src/ble_sm_cmd.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_hci_cmd.c")
        .file("mynewt-nimble/nimble/host/src/ble_sm_lgcy.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_hci_evt.c")
        .file("mynewt-nimble/nimble/host/src/ble_sm_sc.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_hci_util.c")
        .file("mynewt-nimble/nimble/host/src/ble_store.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_id.c")
        .file("mynewt-nimble/nimble/host/src/ble_store_util.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_log.c")
        .file("mynewt-nimble/nimble/host/src/ble_uuid.c")
        .file("mynewt-nimble/nimble/host/src/ble_hs_mbuf.c")
        // gap service
        .file("mynewt-nimble/nimble/host/services/gap/src/ble_svc_gap.c")
        .include("mynewt-nimble/nimble/host/services/gap/include")
        // porting layer
        .file("mynewt-nimble/porting/npl/dummy/src/hci_dummy.c")
        .file("mynewt-nimble/porting/nimble/src/nimble_port.c")
        .file("mynewt-nimble/porting/nimble/src/endian.c")
        .file("mynewt-nimble/porting/nimble/src/os_mbuf.c")
        .file("mynewt-nimble/porting/nimble/src/os_mempool.c")
        .file("mynewt-nimble/porting/nimble/src/os_msys_init.c")
        .file("mynewt-nimble/porting/nimble/src/mem.c")
        // tinycrypt
        .file("mynewt-nimble/ext/tinycrypt/src/aes_encrypt.c")
        .file("mynewt-nimble/ext/tinycrypt/src/utils.c")
        .file("mynewt-nimble/ext/tinycrypt/src/ccm_mode.c")
        // TODO more services
        .include("mynewt-nimble/nimble/host/include") // ble host
        .include("mynewt-nimble/porting/npl/dummy/include") // semaphore.h
        .include("mynewt-nimble/nimble/include") // nimble_npl.h
        .include("mynewt-nimble/porting/nimble/include") // os/os_mbuf.h
        .include("mynewt-nimble/porting/npl/linux/include") // console.h
        .include("mynewt-nimble/ext/tinycrypt/include") // tinycrypt
        .warnings(false)
        .compile("nimble-host");

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rustc-link-lib=nimble-host");

    let bindings = bindgen::Builder::default()
        .use_core()
        .ctypes_prefix("cty")
        .header("wrapper.h")
        .clang_arg("-Imynewt-nimble/nimble/host/include") // ble host
        .clang_arg("-Imynewt-nimble/porting/npl/dummy/include") // semaphore.h
        .clang_arg("-Imynewt-nimble/nimble/include") // nimble_npl.h
        .clang_arg("-Imynewt-nimble/porting/nimble/include") // os/os_mbuf.h
        .clang_arg("-Imynewt-nimble/porting/npl/linux/include") // console.h
        .clang_arg("-Imynewt-nimble/ext/tinycrypt/include") // tinycrypt
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
