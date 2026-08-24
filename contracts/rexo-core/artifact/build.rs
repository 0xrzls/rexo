// Copyright (c) Subzero Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../wit");

    // Build the Rialo VM (RISC-V/PolkaVM) program if the Rialo compiler toolchain is active
    let polkavm_res = std::panic::catch_unwind(|| {
        rialo_build_lib::build_script::setup_polkavm_artifact_build()
            .program_path("..")
            .run()
    });

    match polkavm_res {
        Ok(Ok(_)) => println!("cargo:warning=PolkaVM artifact build succeeded"),
        Ok(Err(e)) => println!("cargo:warning=PolkaVM build skipped or non-fatal: {e:?}"),
        Err(_) => println!("cargo:warning=PolkaVM builder not found in PATH, continuing with standalone WASM artifacts"),
    }

    // Compile rex WASM components (if any rex blocks are present)
    let rex_res = std::panic::catch_unwind(|| {
        rialo_venus_build_helper::compile_rex_components("..")
    });

    match rex_res {
        Ok(Ok(_)) => println!("cargo:warning=Rex WASM components compiled"),
        Ok(Err(e)) => println!("cargo:warning=Rex component compilation non-fatal: {e:?}"),
        Err(_) => println!("cargo:warning=Rex compiler skipped"),
    }
}

