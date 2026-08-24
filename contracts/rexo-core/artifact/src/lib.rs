// Copyright (c) Subzero Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

pub const PROGRAM: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/",
    env!("RIALO_BUILD_ARTIFACT_FILE")
));
