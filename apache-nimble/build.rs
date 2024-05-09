use std::path::{Path, PathBuf};
use std::{env, fs};

fn add_c_files(builder: &mut cc::Build, path: &str) {
    let path = Path::new(path);
    for f in fs::read_dir(path).unwrap() {
        let f = f.unwrap();
        let path = f.path();
        if path.extension().is_some_and(|e| e == "c") {
            builder.file(path);
        }
    }
}

const PORT_LAYER_CRATE_DIR: &str = "../apache-nimble-sys";

#[cfg(feature = "port-layer-embassy")]
const DEFINE: &str = "DEFINE_EMBASSY";

const CHIP_FEATURES: &[(bool, &str)] = &[(cfg!(feature = "nrf52840"), "NRF52840_XXAA")];

fn set_target_flags(builder: &mut cc::Build) -> String {
    let target = env::var("TARGET").unwrap();

    if CHIP_FEATURES
        .iter()
        .filter(|(enabled, _)| (*enabled))
        .count()
        > 1
    {
        panic!("more than one driver enabled")
    }

    let (_, chip) = CHIP_FEATURES
        .iter()
        .find(|(enabled, _)| *enabled)
        .expect("please enable at least one chip feature flag");

    // driver setup
    match *chip {
        "NRF52840_XXAA" => {
            if cfg!(feature = "controller") {
                // Driver
                add_c_files(builder, "../mynewt-nimble/nimble/drivers/nrf5x/src/nrf52");
                add_c_files(builder, "../mynewt-nimble/nimble/drivers/nrf5x/src");
                builder.include("../mynewt-nimble/nimble/drivers/nrf5x/include");

                // CMSIS_5 (needed for nrfx)
                builder.include("../CMSIS_5/CMSIS/Core/Include");

                // nRFx
                builder.define(chip, None);
                builder.include("../nrfx");
                builder.include("../nrfx/mdk");
                builder.include("../nrfx/hal");
                builder.include("nrfx_include");
            }
        }
        _ => unreachable!(),
    }

    // get sysroot path
    let output = builder
        .get_compiler()
        .to_command()
        .arg("-print-sysroot")
        .output()
        .expect("could not get sysroot path");
    let string = String::from_utf8(output.stdout).unwrap();
    let sysroot = string.trim();

    // libc path
    match (target.as_str(), *chip) {
        ("thumbv7em-none-eabihf", "NRF52840_XXAA") => format!("{sysroot}/lib/thumb/v7e-m+fp/hard"),
        ("thumbv7em-none-eabi", "NRF52840_XXAA") => format!("{sysroot}/lib/thumb/v7e-m+fp/softfp"),
        _ => panic!("unsupported target and chip pair: ({}, {})", target, chip),
    }
}

fn compile_nimble(generated_port_layer_types: PathBuf) {
    let builder = &mut cc::Build::new();

    // Define port layer in use
    builder.define(DEFINE, None);

    // Transport
    add_c_files(builder, "../mynewt-nimble/nimble/transport/src");
    builder.include("../mynewt-nimble/nimble/transport/include");

    // Porting layer
    builder.include("../mynewt-nimble/nimble/include"); // nimble_npl.h
    builder
        // note: we don't compile nimble_port.c, and hal_timer.c is only compiled when the
        // controller is enabled.
        .file("../mynewt-nimble/porting/nimble/src/endian.c")
        .file("../mynewt-nimble/porting/nimble/src/os_mbuf.c")
        .file("../mynewt-nimble/porting/nimble/src/os_mempool.c")
        .file("../mynewt-nimble/porting/nimble/src/os_cputime.c")
        .file("../mynewt-nimble/porting/nimble/src/os_cputime_pwr2.c")
        .file("../mynewt-nimble/porting/nimble/src/os_msys_init.c")
        .file("../mynewt-nimble/porting/nimble/src/mem.c");
    // Generated types from cbindgen must be first out of the following three includes, so that the
    // dummy types aren't used.
    builder.include(generated_port_layer_types);
    builder.include(format!("{PORT_LAYER_CRATE_DIR}/include"));
    builder.include("../mynewt-nimble/porting/nimble/include");

    // Tinycrypt
    builder
        .file("../mynewt-nimble/ext/tinycrypt/src/aes_encrypt.c")
        .file("../mynewt-nimble/ext/tinycrypt/src/utils.c")
        .file("../mynewt-nimble/ext/tinycrypt/src/ccm_mode.c")
        .include("../mynewt-nimble/ext/tinycrypt/include");

    // Feature-specific components
    if cfg!(feature = "controller") {
        builder.file("../mynewt-nimble/porting/nimble/src/hal_timer.c");

        builder.define("NIMBLE_CFG_CONTROLLER", Some("1"));
        add_c_files(builder, "../mynewt-nimble/nimble/controller/src");
        builder.include("../mynewt-nimble/nimble/controller/include");
    }

    if cfg!(feature = "host") {
        add_c_files(builder, "../mynewt-nimble/nimble/host/src");
        builder.include("../mynewt-nimble/nimble/host/include");
        // GAP
        add_c_files(builder, "../mynewt-nimble/nimble/host/services/gatt/src");
        builder.include("../mynewt-nimble/nimble/host/services/gatt/include");
        // GATT
        add_c_files(builder, "../mynewt-nimble/nimble/host/services/gap/src");
        builder.include("../mynewt-nimble/nimble/host/services/gap/include");
    }

    // Target specific compilation flags
    let libc_path = set_target_flags(builder);

    // Compile
    builder.warnings(false).compile("nimble-controller");
    println!("cargo:rustc-link-lib=static=nimble-controller");

    // Note: some of the libc functions we replace ourselves, like __assert_func. See the `interop`
    // module in one of the port-layer files.
    println!("cargo:rustc-link-search={libc_path}");
    println!("cargo:rustc-link-lib=static=c");
}

fn main() {
    println!("cargo:rerun-if-changed=nrfx_include");

    // Note: we aren't using nimble_npl_os.h from the port layer crate. Since those types are
    // implemented in the apache-nimble-sys crate's lib.rs file, we generate C bindings for them,
    // then include them as part of the nimble build.

    let target_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut npl = target_dir.clone();
    npl.push("nimble");
    std::fs::create_dir_all(npl.clone()).unwrap();
    npl.push("nimble_npl_os.h");
    cbindgen::generate_with_config(
        PORT_LAYER_CRATE_DIR,
        cbindgen::Config::from_file("cbindgen.toml").unwrap(),
    )
    .expect("Unable to generate bindings")
    .write_to_file(npl.into_os_string());

    compile_nimble(target_dir);
}
