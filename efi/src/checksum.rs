// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026 Alexandru-Nicolas Negrișan

use sha2::{Digest, Sha256};

pub fn verify_sha256(data: &[u8], expected_hex: &str) -> uefi::Result {
    if expected_hex.len() != 64 {
        return Err(uefi::Status::INVALID_PARAMETER.into());
    }

    let actual = Sha256::digest(data);
    let matches = actual
        .iter()
        .enumerate()
        .all(|(i, &byte)| u8::from_str_radix(&expected_hex[2 * i..2 * i + 2], 16) == Ok(byte));

    if matches {
        Ok(())
    } else {
        Err(uefi::Status::SECURITY_VIOLATION.into())
    }
}
