// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! The replay stream's primitive encoding: LEB128 varints, zig-zag signed
//! integers, little-endian floats and length-prefixed strings.
//!
//! A recording is a stream of ticks, and most of a tick's bytes are small
//! numbers (an entity id, a delta of one block, a yaw in thousandths of a
//! degree). Fixed-width encoding would make a 20 tps recording of a busy server
//! several times larger than it needs to be, so everything that can be variable
//! is. The format is documented in `docs/VOLTLAPSE_FORMAT.md` and mirrored by
//! `tools/voltlapse_cli.py`, so a replay written by the mod can be inspected by
//! a tool that never links the engine.

/// Writes primitives into a byte buffer.
#[derive(Default, Debug)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { buf: Vec::with_capacity(capacity) }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub fn bool(&mut self, value: bool) {
        self.buf.push(value as u8);
    }

    /// Unsigned LEB128: seven bits per byte, high bit means "another byte".
    pub fn varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.buf.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.buf.push(value as u8);
    }

    pub fn varint_u32(&mut self, value: u32) {
        self.varint(value as u64);
    }

    /// Zig-zag then varint, so small negatives cost one byte too.
    pub fn zz(&mut self, value: i64) {
        self.varint(((value << 1) ^ (value >> 63)) as u64);
    }

    pub fn f32(&mut self, value: f32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f64(&mut self, value: f64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Bytes with a varint length prefix.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.varint(bytes.len() as u64);
        self.buf.extend_from_slice(bytes);
    }

    pub fn str(&mut self, text: &str) {
        self.bytes(text.as_bytes());
    }
}

/// Why a stream could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// The buffer ended in the middle of a value.
    Truncated,
    /// A varint was longer than 10 bytes, or a length exceeded the limit.
    Malformed,
    /// A string was not valid UTF-8.
    BadUtf8,
    /// A value was outside the range its field allows.
    OutOfRange,
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            WireError::Truncated => "stream ended mid-value",
            WireError::Malformed => "malformed value",
            WireError::BadUtf8 => "invalid UTF-8",
            WireError::OutOfRange => "value out of range",
        };
        f.write_str(text)
    }
}

impl std::error::Error for WireError {}

pub type Result<T> = std::result::Result<T, WireError>;

/// Longest string or byte blob a single field may carry (16 MiB, the same
/// per-entry ceiling the mod-file spec sets).
const MAX_BLOB: usize = 16 * 1024 * 1024;

/// Reads primitives from a byte slice.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn u8(&mut self) -> Result<u8> {
        let byte = *self.data.get(self.pos).ok_or(WireError::Truncated)?;
        self.pos += 1;
        Ok(byte)
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub fn varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.u8()?;
            if shift >= 64 {
                return Err(WireError::Malformed);
            }
            value |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    pub fn varint_u32(&mut self) -> Result<u32> {
        let value = self.varint()?;
        u32::try_from(value).map_err(|_| WireError::OutOfRange)
    }

    pub fn zz(&mut self) -> Result<i64> {
        let raw = self.varint()?;
        Ok(((raw >> 1) as i64) ^ -((raw & 1) as i64))
    }

    pub fn f32(&mut self) -> Result<f32> {
        let bytes = self.take(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn f64(&mut self) -> Result<f64> {
        let bytes = self.take(8)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(f64::from_le_bytes(array))
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            return Err(WireError::Truncated);
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.varint()?;
        let len = usize::try_from(len).map_err(|_| WireError::OutOfRange)?;
        if len > MAX_BLOB {
            return Err(WireError::Malformed);
        }
        self.take(len)
    }

    pub fn bytes_vec(&mut self) -> Result<Vec<u8>> {
        Ok(self.bytes()?.to_vec())
    }

    pub fn str(&mut self) -> Result<&'a str> {
        std::str::from_utf8(self.bytes()?).map_err(|_| WireError::BadUtf8)
    }

    pub fn string(&mut self) -> Result<String> {
        Ok(self.str()?.to_owned())
    }
}

/// A value the replay stream can write and read back.
pub trait Wire: Sized {
    fn write(&self, out: &mut Writer);
    fn read(input: &mut Reader<'_>) -> Result<Self>;
}

impl Wire for u8 {
    fn write(&self, out: &mut Writer) {
        out.u8(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.u8()
    }
}

impl Wire for bool {
    fn write(&self, out: &mut Writer) {
        out.bool(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.bool()
    }
}

impl Wire for u32 {
    fn write(&self, out: &mut Writer) {
        out.varint_u32(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.varint_u32()
    }
}

impl Wire for u64 {
    fn write(&self, out: &mut Writer) {
        out.varint(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.varint()
    }
}

impl Wire for i64 {
    fn write(&self, out: &mut Writer) {
        out.zz(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.zz()
    }
}

impl Wire for f32 {
    fn write(&self, out: &mut Writer) {
        out.f32(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.f32()
    }
}

impl Wire for f64 {
    fn write(&self, out: &mut Writer) {
        out.f64(*self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.f64()
    }
}

impl Wire for String {
    fn write(&self, out: &mut Writer) {
        out.str(self);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        input.string()
    }
}

impl<T: Wire> Wire for Option<T> {
    fn write(&self, out: &mut Writer) {
        match self {
            Some(value) => {
                out.bool(true);
                value.write(out);
            }
            None => out.bool(false),
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        if input.bool()? {
            Ok(Some(T::read(input)?))
        } else {
            Ok(None)
        }
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn write(&self, out: &mut Writer) {
        out.varint(self.len() as u64);
        for item in self {
            item.write(out);
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        let len = input.varint()?;
        let len = usize::try_from(len).map_err(|_| WireError::OutOfRange)?;
        // Every element is at least one byte, so a length beyond the remaining
        // input is malformed rather than an allocation to attempt.
        if len > input.remaining() {
            return Err(WireError::Malformed);
        }
        let mut items = Vec::with_capacity(len);
        for _ in 0..len {
            items.push(T::read(input)?);
        }
        Ok(items)
    }
}

/// A position in the world, in double precision.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn sub(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub fn scale(self, factor: f64) -> Vec3 {
        Vec3::new(self.x * factor, self.y * factor, self.z * factor)
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Vec3 {
        let len = self.length();
        if len <= 1e-12 {
            Vec3::ZERO
        } else {
            self.scale(1.0 / len)
        }
    }

    pub fn distance(self, other: Vec3) -> f64 {
        self.sub(other).length()
    }

    pub fn lerp(self, other: Vec3, t: f64) -> Vec3 {
        self.scale(1.0 - t).add(other.scale(t))
    }

    /// Finite check used wherever a value comes from a file or a packet.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Wire for Vec3 {
    fn write(&self, out: &mut Writer) {
        out.f64(self.x);
        out.f64(self.y);
        out.f64(self.z);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Vec3::new(input.f64()?, input.f64()?, input.f64()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip_at_every_width_boundary() {
        for value in [0u64, 1, 127, 128, 255, 16_383, 16_384, u32::MAX as u64, u64::MAX] {
            let mut w = Writer::new();
            w.varint(value);
            let bytes = w.into_bytes();
            let mut r = Reader::new(&bytes);
            assert_eq!(r.varint().expect("reads"), value);
            assert!(r.is_empty());
        }
    }

    #[test]
    fn small_values_take_one_byte() {
        let mut w = Writer::new();
        w.varint(127);
        assert_eq!(w.len(), 1);
        let mut w = Writer::new();
        w.varint(128);
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn zig_zag_keeps_small_negatives_small() {
        let mut w = Writer::new();
        w.zz(-1);
        w.zz(-64);
        w.zz(i64::MIN);
        assert_eq!(w.len(), 1 + 1 + 10);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.zz().unwrap(), -1);
        assert_eq!(r.zz().unwrap(), -64);
        assert_eq!(r.zz().unwrap(), i64::MIN);
    }

    #[test]
    fn strings_and_blobs_round_trip() {
        let mut w = Writer::new();
        w.str("hallo 🌍");
        w.bytes(&[1, 2, 3]);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.str().unwrap(), "hallo 🌍");
        assert_eq!(r.bytes().unwrap(), &[1, 2, 3]);
        assert!(r.is_empty());
    }

    #[test]
    fn truncation_is_reported_not_panicked() {
        let mut r = Reader::new(&[0xFF]);
        assert_eq!(r.varint(), Err(WireError::Truncated));
        let mut r = Reader::new(&[4, 65]);
        assert_eq!(r.str(), Err(WireError::Truncated));
        let mut r = Reader::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
        assert_eq!(r.varint(), Err(WireError::Malformed));
    }

    #[test]
    fn vecs_refuse_absurd_lengths_instead_of_allocating() {
        let mut r = Reader::new(&[0xFA, 0xFF, 0xFF, 0xFF, 0x0F]);
        assert!(matches!(Vec::<u8>::read(&mut r), Err(WireError::Malformed)));
    }

    #[test]
    fn generic_round_trip_of_a_record() {
        let record: (u32, Option<String>, Vec<f32>, bool) = (7, Some("x".into()), vec![1.5, -2.25], true);
        let mut w = Writer::new();
        record.0.write(&mut w);
        record.1.write(&mut w);
        record.2.write(&mut w);
        record.3.write(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        let back = (
            u32::read(&mut r).unwrap(),
            Option::<String>::read(&mut r).unwrap(),
            Vec::<f32>::read(&mut r).unwrap(),
            bool::read(&mut r).unwrap(),
        );
        assert_eq!(back, record);
    }

    #[test]
    fn vec3_math_is_stable_near_zero() {
        assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
        let unit = Vec3::new(0.0, 3.0, 4.0).normalized();
        assert!((unit.length() - 1.0).abs() < 1e-12);
        assert!((Vec3::new(0.0, 0.0, 0.0).lerp(Vec3::new(2.0, 4.0, 6.0), 0.5) - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-12);
        assert!(!Vec3::new(f64::NAN, 0.0, 0.0).is_finite());
    }
}
