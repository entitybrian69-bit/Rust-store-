// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 the Volt Lapse contributors.
// Feature design follows Replay Mod (Copyright (C) 2014-2026 Johni0702 et al.,
// LGPL-3.0); see NOTICE beside this bundle for the full attribution.

//! PNG screenshots, written without an image crate.
//!
//! Volt Lapse captures stills from a playback (`Screenshot` keybind) and can
//! dump every rendered frame of an offline render as a PNG sequence when no
//! encoder is installed. The encoder writes 8-bit RGBA or RGB images with
//! zlib's "stored" block type: no compression, but a valid PNG that every
//! viewer, editor and ffmpeg reads. Volunteer compression is the render
//! pipeline's job (a video encoder), not the screenshot path's.

use crate::crc::{adler32, crc32};

/// A tightly packed image: 4 bytes per pixel (RGBA) or 3 (RGB).
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `true` for RGBA, `false` for RGB.
    pub alpha: bool,
    /// Pixels, row-major, no padding.
    pub pixels: Vec<u8>,
}

impl Image {
    /// A transparent-free black image of the given size.
    pub fn new(width: u32, height: u32, alpha: bool) -> Self {
        let bytes = if alpha { 4 } else { 3 };
        Self { width, height, alpha, pixels: vec![0u8; width as usize * height as usize * bytes] }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        if self.alpha { 4 } else { 3 }
    }

    /// Set one pixel (RGBA or RGB depending on `alpha`).
    pub fn set(&mut self, x: u32, y: u32, pixel: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let bpp = self.bytes_per_pixel();
        let index = (y as usize * self.width as usize + x as usize) * bpp;
        let count = if self.alpha { 4 } else { 3 };
        self.pixels[index..index + count].copy_from_slice(&pixel[..count]);
    }

    /// Crop to the centre rectangle, for a render's letterbox or resolution
    /// override.
    pub fn cropped(&self, width: u32, height: u32) -> Image {
        let mut out = Image::new(width.min(self.width), height.min(self.height), self.alpha);
        let bpp = self.bytes_per_pixel();
        for y in 0..out.height {
            let src = (y as usize * self.width as usize) * bpp;
            let dst = (y as usize * out.width as usize) * bpp;
            let count = out.width as usize * bpp;
            out.pixels[dst..dst + count].copy_from_slice(&self.pixels[src..src + count]);
        }
        out
    }

    /// Encode to a `.png` byte vector.
    pub fn to_png(&self) -> Vec<u8> {
        let bpp = self.bytes_per_pixel() as u32;
        let stride = (self.width * bpp) as usize;
        // One filter byte (0 = None) before each scanline.
        let mut scanlines = Vec::with_capacity((stride + 1) * self.height as usize);
        for y in 0..self.height as usize {
            scanlines.push(0);
            scanlines.extend_from_slice(&self.pixels[y * stride..(y + 1) * stride]);
        }

        let mut png = Vec::new();
        png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        ihdr.push(8); // bit depth
        ihdr.push(if self.alpha { 6 } else { 2 }); // colour type: RGBA / RGB
        ihdr.extend_from_slice(&[0, 0, 0]); // deflate, adaptive filter, no interlace
        chunk(&mut png, b"IHDR", &ihdr);
        chunk(&mut png, b"IDAT", &zlib_stored(&scanlines));
        chunk(&mut png, b"IEND", &[]);
        png
    }
}

/// A PNG chunk: length, type, data, CRC over type and data.
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Wrap the scanline bytes in a zlib stream made of stored deflate blocks
/// (max 65 535 each). Stored blocks keep the writer dependency-free; the outer
/// replay container is what compresses.
pub fn zlib_stored(scanlines: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(scanlines.len() + 64);
    out.extend_from_slice(&[0x78, 0x01]); // CMF: deflate, 32K window; FLG: no dict, fastest
    if scanlines.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        let mut offset = 0;
        while offset < scanlines.len() {
            let len = (scanlines.len() - offset).min(65_535);
            let last = offset + len == scanlines.len();
            out.push(if last { 1 } else { 0 });
            out.extend_from_slice(&(len as u16).to_le_bytes());
            out.extend_from_slice(&(!(len as u16)).to_le_bytes());
            out.extend_from_slice(&scanlines[offset..offset + len]);
            offset += len;
        }
    }
    out.extend_from_slice(&adler32(scanlines).to_be_bytes());
    out
}

/// Undo [`zlib_stored`]; only the form this module writes, which is what the
/// tests and the frame-pipeline self-check need. Returns `Err` for a compressed
/// stream, so a caller never mistakes unvalidated output for a decode.
pub fn unzlib_stored(stream: &[u8]) -> std::result::Result<Vec<u8>, &'static str> {
    if stream.len() < 6 {
        return Err("short zlib stream");
    }
    if stream[0] & 0x0F != 8 {
        return Err("not deflate");
    }
    if (stream[0] as u16 * 256 + stream[1] as u16) % 31 != 0 {
        return Err("bad zlib header check");
    }
    let mut out = Vec::new();
    let mut pos = 2;
    loop {
        let header = *stream.get(pos).ok_or("truncated block header")?;
        pos += 1;
        if header & 0x06 != 0 {
            return Err("compressed blocks are not read here");
        }
        let len = u16::from_le_bytes([stream[pos], stream[pos + 1]]) as usize;
        let nlen = u16::from_le_bytes([stream[pos + 2], stream[pos + 3]]) as usize;
        if len != (!nlen & 0xFFFF) {
            return Err("length check failed");
        }
        pos += 4;
        out.extend_from_slice(stream.get(pos..pos + len).ok_or("truncated stored block")?);
        pos += len;
        if header & 1 == 1 {
            break;
        }
    }
    let expected = adler32(&out);
    let found = u32::from_be_bytes([
        *stream.get(pos).ok_or("missing adler32")?,
        *stream.get(pos + 1).ok_or("missing adler32")?,
        *stream.get(pos + 2).ok_or("missing adler32")?,
        *stream.get(pos + 3).ok_or("missing adler32")?,
    ]);
    if expected != found {
        return Err("adler32 mismatch");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_iend(png: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut chunks = Vec::new();
        let mut pos = 8;
        while pos + 8 <= png.len() {
            let len = u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
            let kind = String::from_utf8(png[pos + 4..pos + 8].to_vec()).unwrap();
            let data = png[pos + 8..pos + 8 + len].to_vec();
            let stored = u32::from_be_bytes([
                png[pos + 8 + len],
                png[pos + 9 + len],
                png[pos + 10 + len],
                png[pos + 11 + len],
            ]);
            assert_eq!(stored, crc32(&png[pos + 4..pos + 8 + len]), "chunk CRC");
            chunks.push((kind, data));
            pos += 12 + len;
        }
        assert_eq!(pos, png.len(), "no trailing bytes");
        chunks
    }

    #[test]
    fn writes_a_readable_rgba_png() {
        let mut image = Image::new(3, 2, true);
        image.set(0, 0, [255, 0, 0, 255]);
        image.set(2, 1, [0, 0, 255, 128]);
        let png = image.to_png();
        let chunks = decode_iend(&png);
        assert_eq!(chunks[0].0, "IHDR");
        assert_eq!(chunks[0].1[..8], [0, 0, 0, 3, 0, 0, 0, 2]);
        assert_eq!(chunks[0].1[9], 6);
        assert_eq!(chunks.last().unwrap().0, "IEND");

        let inflated = unzlib_stored(&chunks[1].1).expect("inflates");
        assert_eq!(inflated.len(), 2 * (1 + 3 * 4));
        assert_eq!(&inflated[1..5], &[255, 0, 0, 255]);
        assert_eq!(&inflated[(1 + 3 * 4) + 1 + 2 * 4..(1 + 3 * 4) + 1 + 2 * 4 + 4], &[0, 0, 255, 128]);
    }

    #[test]
    fn opaque_images_use_colour_type_2() {
        let image = Image::new(1, 1, false);
        let png = image.to_png();
        let chunks = decode_iend(&png);
        assert_eq!(chunks[0].1[9], 2);
        assert_eq!(unzlib_stored(&chunks[1].1).unwrap(), vec![0, 0, 0, 0]);
    }

    #[test]
    fn large_images_span_several_stored_blocks() {
        let image = Image::new(300, 300, true); // 360 000 bytes > 65 535
        let png = image.to_png();
        let chunks = decode_iend(&png);
        let inflated = unzlib_stored(&chunks[1].1).unwrap();
        assert_eq!(inflated.len(), 300 * (1 + 300 * 4));
    }

    #[test]
    fn a_bad_checksum_is_reported() {
        let mut stream = zlib_stored(b"voltlapse");
        let len = stream.len();
        stream[len - 1] ^= 0xFF;
        assert_eq!(unzlib_stored(&stream), Err("adler32 mismatch"));
    }

    #[test]
    fn crop_keeps_the_top_left_window() {
        let mut image = Image::new(4, 4, false);
        image.set(1, 1, [9, 9, 9, 0]);
        let crop = image.cropped(2, 2);
        assert_eq!(crop.width, 2);
        assert_eq!(&crop.pixels[3..6], &[9, 9, 9]);
    }
}
