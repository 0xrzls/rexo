// Copyright (c) Subzero Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    // Build the Rialo VM (RISC-V) program
    rialo_build_lib::build_script::setup_polkavm_artifact_build()
        .program_path("..")
        .run()
        .unwrap();

    // Compile rex WASM components (if any rex blocks are present)
    rialo_venus_build_helper::compile_rex_components("..")
        .expect("Failed to compile rex WASM components");
}
