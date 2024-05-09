use std::env;
use std::path::PathBuf;

fn generate_bindings() {
    let builder = bindgen::Builder::default()
        .use_core()
        .ctypes_prefix("cty")
        .parse_callbacks(Box::new(
            bindgen::CargoCallbacks::new().rerun_on_header_files(false),
        ))
        .derive_debug(false)
        .layout_tests(false)
        .derive_copy(false)
        .derive_default(false)
        // These types need to be defined by us
        // Generic
        .blocklist_function("ble_npl_os_started")
        .blocklist_function("ble_npl_get_current_task_id")
        // Events
        .blocklist_type("ble_npl_event")
        .blocklist_type("ble_npl_eventq")
        .blocklist_function("ble_npl_eventq_init")
        .blocklist_function("ble_npl_eventq_get")
        .blocklist_function("ble_npl_eventq_put")
        .blocklist_function("ble_npl_eventq_remove")
        .blocklist_function("ble_npl_event_init")
        .blocklist_function("ble_npl_event_is_queued")
        .blocklist_function("ble_npl_event_get_arg")
        .blocklist_function("ble_npl_event_set_arg")
        .blocklist_function("ble_npl_eventq_is_empty")
        .blocklist_function("ble_npl_event_run")
        // Mutex
        .blocklist_type("ble_npl_mutex")
        .blocklist_function("ble_npl_mutex_init")
        .blocklist_function("ble_npl_mutex_pend")
        .blocklist_function("ble_npl_mutex_release")
        // Semaphore
        .blocklist_type("ble_npl_sem")
        .blocklist_function("ble_npl_sem_init")
        .blocklist_function("ble_npl_sem_pend")
        .blocklist_function("ble_npl_sem_release")
        .blocklist_function("ble_npl_sem_get_count")
        // Callout
        .blocklist_type("ble_npl_callout")
        .blocklist_function("ble_npl_callout_init")
        .blocklist_function("ble_npl_callout_reset")
        .blocklist_function("ble_npl_callout_stop")
        .blocklist_function("ble_npl_callout_is_active")
        .blocklist_function("ble_npl_callout_get_ticks")
        .blocklist_function("ble_npl_callout_remaining_ticks")
        .blocklist_function("ble_npl_callout_set_arg")
        // Time functions
        .blocklist_function("ble_npl_time_get")
        .blocklist_function("ble_npl_time_ms_to_ticks")
        .blocklist_function("ble_npl_time_ticks_to_ms")
        .blocklist_function("ble_npl_time_ms_to_ticks32")
        .blocklist_function("ble_npl_time_ticks_to_ms32")
        .blocklist_function("ble_npl_time_delay")
        // Hardware-specific
        .blocklist_function("ble_npl_hw_set_isr")
        .blocklist_function("ble_npl_hw_enter_critical")
        .blocklist_function("ble_npl_hw_exit_critical")
        .blocklist_function("ble_npl_hw_is_in_critical");

    // select headers to generate bindings for

    // headers to always generate bindings for: port layer, hci definitions, syscfg
    let builder = builder
        .clang_arg("-Iinclude")
        .clang_arg("-I../mynewt-nimble/nimble/include")
        .clang_arg("-I../mynewt-nimble/porting/nimble/include")
        .clang_arg("-I../mynewt-nimble/nimble/transport/include")
        .header("../mynewt-nimble/nimble/include/nimble/hci_common.h")
        .header("../mynewt-nimble/porting/nimble/include/hal/hal_timer.h")
        .header("include/syscfg/syscfg.h");

    // controller bindings
    let builder = if cfg!(feature = "controller") {
        builder
            .clang_arg("-DNIMBLE_CFG_CONTROLLER=1") // tinycrypt
            .clang_arg("-I../mynewt-nimble/nimble/controller/include") // transport
            .header("../mynewt-nimble/nimble/controller/include/controller/ble_fem.h")
            .header("../mynewt-nimble/nimble/controller/include/controller/ble_hw.h")
            .header("../mynewt-nimble/nimble/controller/include/controller/ble_ll.h")
    } else {
        builder
    };

    // host bindings
    let builder = if cfg!(feature = "host") {
        builder
            .clang_arg("-I../mynewt-nimble/nimble/host/include") // ble host
            .header("../mynewt-nimble/nimble/host/include/host/ble_hs.h")
            // GATT
            .header("../mynewt-nimble/nimble/host/services/gap/include/services/gap/ble_svc_gap.h")
            // GAP
            .header(
                "../mynewt-nimble/nimble/host/services/gatt/include/services/gatt/ble_svc_gatt.h",
            )
    } else {
        builder
    };

    let bindings = builder.generate().expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn main() {
    println!("cargo:rerun-if-changed=include");

    generate_bindings();
}
