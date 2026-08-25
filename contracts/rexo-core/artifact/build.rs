// Copyright (c) Subzero Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    println!("cargo:rerun-if-changed=../src");
    println!("cargo:rerun-if-changed=../wit");

    // Build the Rialo VM (RISC-V/PolkaVM) program
    if let Err(e) = rialo_build_lib::build_script::setup_polkavm_artifact_build()
        .program_path("..")
        .run()
    {
        panic!("PolkaVM artifact build failed: {:?}", e);
    }

    // Compile rex WASM components (if any rex blocks are present)
    if let Err(e) = rialo_venus_build_helper::compile_rex_components("..") {
        println!("cargo:warning=Rex component compilation notice: {:?}", e);
    }
}

