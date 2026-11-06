// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! CRC-32 (the zip/PKF variant zlib uses) and Adler-32 (PNG's zlib trailer).
//!
//! Both are needed by the packagers the engine carries: stored-zip replay
//! packages (`zipstore.rs`) and PNG screenshots (`png.rs`). Implementations are
//! table-driven and checked against published vectors.

const CRC_TABLE: [u32; 256] = build_crc_table();

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 { 0xEDB8_8320 ^ (crc >> 1) } else { crc >> 1 };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// CRC-32 of `data`, as `zipfile` and zlib's `crc32` compute it.
pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(0, data)
}

/// CRC-32 continuing from `previous` (the chainable form zlib exposes).
pub fn crc32_update(previous: u32, data: &[u8]) -> u32 {
    let mut crc = !previous;
    for byte in data {
        crc = CRC_TABLE[((crc ^ *byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Adler-32, the PNG zlib stream's checksum.
pub fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    // 5552 is the largest block that cannot overflow before the modulo.
    for chunk in data.chunks(5552) {
        for byte in chunk {
            a += *byte as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_published_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn crc32_is_chainable() {
        let whole = crc32(b"voltlapse replay");
        let first = crc32_update(0, b"voltlapse ");
        assert_eq!(crc32_update(first, b"replay"), whole);
    }

    #[test]
    fn adler32_matches_published_vectors() {
        assert_eq!(adler32(b""), 1);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        // For n zero bytes `a` stays 1 and `b` is n modulo the prime, so this
        // also exercises the block reduction across more than one chunk.
        let zeros = adler32(&[0u8; 100_000]);
        assert_eq!(zeros, ((100_000 % 65_521) << 16) | 1);
    }
}
