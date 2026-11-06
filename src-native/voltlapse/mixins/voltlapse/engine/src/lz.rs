// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The recording stream's compression.
//!
//! Packet data is repetitive (the same entity moving every tick, the same block
//! palette), so a small LZ77 pass over the tick stream is worth more than a
//! general-purpose deflate implementation would cost in dependencies. The
//! codec is deliberately simple and fully deterministic: a match is a
//! 32 KiB-window back-reference found through a hash chain, literals are
//! stored in runs.
//!
//! [`pack`] keeps whichever is smaller and marks its choice with a four-byte
//! magic, so a reader never has to be told whether a segment is compressed.

/// Magic of a compressed segment: `VLC1`.
pub const MAGIC: [u8; 4] = *b"VLC1";

const MIN_MATCH: usize = 4;
const MAX_MATCH: usize = 4096;
const WINDOW: usize = 32 * 1024;
const HASH_BITS: u32 = 15;
const MAX_CHAIN: usize = 48;

fn hash4(bytes: &[u8], at: usize) -> usize {
    let value = u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (value.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

/// Compress `data`, returning the raw input when compression does not help.
pub fn pack(data: &[u8]) -> Vec<u8> {
    let compressed = compress(data);
    if compressed.len() + MAGIC.len() < data.len() {
        let mut out = Vec::with_capacity(compressed.len() + MAGIC.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&compressed);
        out
    } else {
        data.to_vec()
    }
}

/// Reverse [`pack`].
pub fn unpack(data: &[u8]) -> std::result::Result<Vec<u8>, &'static str> {
    if data.len() >= MAGIC.len() && data[..MAGIC.len()] == MAGIC {
        decompress(&data[MAGIC.len()..])
    } else {
        Ok(data.to_vec())
    }
}

/// LZ77 compression: a sequence of literal runs (tag 0) and matches (tag 1).
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = crate::wire::Writer::with_capacity(data.len() / 2 + 16);
    out.varint(data.len() as u64);

    let mut chains: Vec<Vec<usize>> = vec![Vec::new(); 1 << HASH_BITS];
    let mut literals_start = 0usize;
    let mut at = 0usize;

    while at < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if at + MIN_MATCH <= data.len() {
            let hash = hash4(data, at);
            let chain = &chains[hash];
            let mut checked = 0;
            for &candidate in chain.iter().rev() {
                if checked >= MAX_CHAIN {
                    break;
                }
                checked += 1;
                let distance = at - candidate;
                if distance > WINDOW {
                    break;
                }
                let limit = (data.len() - at).min(MAX_MATCH);
                let mut len = 0;
                while len < limit && data[candidate + len] == data[at + len] {
                    len += 1;
                }
                if len > best_len {
                    best_len = len;
                    best_dist = distance;
                    if len >= 512 {
                        break;
                    }
                }
            }
        }

        if best_len >= MIN_MATCH {
            if literals_start < at {
                out.u8(0);
                out.varint((at - literals_start) as u64);
                out.bytes(&data[literals_start..at]);
            }
            out.u8(1);
            out.varint((best_len - MIN_MATCH) as u64);
            out.varint((best_dist - 1) as u64);
            // Index every position the match covers, so later matches find it.
            for pos in at..(at + best_len).min(data.len().saturating_sub(MIN_MATCH - 1)) {
                let hash = hash4(data, pos);
                let chain = &mut chains[hash];
                chain.push(pos);
                if chain.len() > MAX_CHAIN * 2 {
                    chain.remove(0);
                }
            }
            at += best_len;
            literals_start = at;
        } else {
            if at + MIN_MATCH <= data.len() {
                let hash = hash4(data, at);
                let chain = &mut chains[hash];
                chain.push(at);
                if chain.len() > MAX_CHAIN * 2 {
                    chain.remove(0);
                }
            }
            at += 1;
        }
    }

    if literals_start < data.len() {
        out.u8(0);
        out.varint((data.len() - literals_start) as u64);
        out.bytes(&data[literals_start..]);
    }
    out.into_bytes()
}

/// Reverse [`compress`]; refuses a stream that does not add up.
pub fn decompress(data: &[u8]) -> std::result::Result<Vec<u8>, &'static str> {
    let mut input = crate::wire::Reader::new(data);
    let total = input.varint().map_err(|_| "bad length")?;
    let total = usize::try_from(total).map_err(|_| "bad length")?;
    if total > 512 * 1024 * 1024 {
        return Err("declared size too large");
    }
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        match input.u8().map_err(|_| "truncated token")? {
            0 => {
                let run = input.bytes().map_err(|_| "truncated literal run")?;
                out.extend_from_slice(run);
            }
            1 => {
                let len = input.varint().map_err(|_| "truncated match length")? as usize + MIN_MATCH;
                let distance = input.varint().map_err(|_| "truncated distance")? as usize + 1;
                if distance > out.len() || distance > WINDOW {
                    return Err("match points outside the window");
                }
                let start = out.len() - distance;
                for i in 0..len {
                    let byte = out[start + i];
                    out.push(byte);
                }
            }
            _ => return Err("unknown token"),
        }
        if out.len() > total {
            return Err("stream longer than declared");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                (state >> 33) as u8
            })
            .collect()
    }

    #[test]
    fn round_trips_empty_short_and_long_inputs() {
        for data in [
            vec![],
            b"a".to_vec(),
            b"voltlapse".to_vec(),
            vec![0u8; 100_000],
            pseudo_random(50_000, 7),
        ] {
            let packed = pack(&data);
            assert_eq!(unpack(&packed).expect("unpacks"), data, "len {}", data.len());
        }
    }

    #[test]
    fn repetitive_data_shrinks_a_lot() {
        let data: Vec<u8> = b"minecraft:overworld;pos=1.0,2.0,3.0;".repeat(500);
        let packed = pack(&data);
        assert!(packed.len() * 8 < data.len(), "packed {} of {}", packed.len(), data.len());
        assert_eq!(unpack(&packed).unwrap(), data);
    }

    #[test]
    fn incompressible_data_is_stored_raw() {
        let data = pseudo_random(4096, 99);
        assert_eq!(pack(&data), data, "random bytes must not grow");
    }

    #[test]
    fn a_match_pointing_before_the_start_is_refused() {
        let mut w = crate::wire::Writer::new();
        w.varint(10);
        w.u8(1); // match
        w.varint(4); // len 8
        w.varint(99); // distance 100, but nothing was output yet
        assert_eq!(decompress(w.as_slice()), Err("match points outside the window"));
    }

    #[test]
    fn overlapping_matches_expand_like_run_length_encoding() {
        // "abc" then a match of length 9 at distance 3 spells "abcabcabcabc".
        let mut w = crate::wire::Writer::new();
        w.varint(12);
        w.u8(0);
        w.bytes(b"abc");
        w.u8(1);
        w.varint(9 - MIN_MATCH as u64);
        w.varint(2);
        assert_eq!(decompress(w.as_slice()).unwrap(), b"abcabcabcabc");
    }
}
