//! Regression test for <https://github.com/image-rs/image-tiff/issues/395>:
//! LZW-compressed strips/tiles whose encoder omitted the end-of-information
//! (EOI) code previously failed to decode with "no lzw end code found",
//! even though every real pixel byte was present in the stream and the
//! output size is already known in advance from the strip/tile geometry.
//! libtiff tolerates this; see the fix in `src/decoder/stream.rs`
//! (`LZWReader::read`).
//!
//! The test file below is built entirely in memory (classic, single-strip,
//! 8-bit grayscale TIFF) rather than vendored from a real-world sample, so
//! the LZW payload can be crafted to omit the EOI code deterministically.

extern crate tiff;
extern crate weezl;

use std::io::Cursor;
use tiff::decoder::{Decoder, DecodingSampleBuffer};

/// Fully LZW-encodes `pixels` (MSB-first bit packing, TIFF early code-size
/// switch, including the terminal end-of-information code) and returns the
/// resulting well-formed, EOI-terminated byte stream.
fn lzw_encode_with_eoi(pixels: &[u8]) -> Vec<u8> {
    weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
        .encode(pixels)
        .expect("encoding raw bytes cannot fail")
}

/// Attempts to decode `stream` (MSB-first, TIFF early code-size switch) into
/// exactly `expected.len()` bytes, tolerating a missing EOI code exactly the
/// way the fixed `LZWReader` does (stop once the known output size is
/// reached, or once input runs out with nothing left to decode). Returns
/// `Some(decoded_bytes)` on success, `None` if the stream doesn't decode to
/// the exact expected length.
fn try_decode_lzw_tolerant(stream: &[u8], expected_len: usize) -> Option<Vec<u8>> {
    let mut decoder = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
    let mut out = vec![0u8; expected_len];
    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    while out_pos < expected_len {
        let result = decoder.decode_bytes(&stream[in_pos..], &mut out[out_pos..]);
        in_pos += result.consumed_in;
        out_pos += result.consumed_out;
        match result.status {
            Ok(weezl::LzwStatus::Ok) => {}
            Ok(weezl::LzwStatus::Done) => break,
            Ok(weezl::LzwStatus::NoProgress) => {
                if in_pos >= stream.len() {
                    break;
                }
                return None;
            }
            Err(_) => return None,
        }
    }
    if out_pos == expected_len {
        Some(out)
    } else {
        None
    }
}

/// Builds an LZW stream for `pixels` that is missing its end-of-information
/// (EOI) code, reproducing what real-world encoders have been observed to
/// emit on the final strip/tile of an image (see
/// <https://github.com/image-rs/image-tiff/issues/395>): every pixel byte is
/// faithfully, correctly encoded, only the terminal EOI marker is absent.
///
/// This starts from a normal, complete, EOI-terminated encode (produced by
/// `weezl::encode::Encoder::finish`, which is also responsible for flushing
/// the last few bits of the final real code into a whole byte) and trims
/// trailing bytes one at a time -- re-verifying with a full tolerant decode
/// after each trim -- until removing one more byte would start eating into
/// real pixel data instead of just the EOI code and its byte-alignment
/// padding. This is more robust than hand-computing the EOI code's bit
/// offset, and self-verifying: the function never returns a stream that
/// fails to decode back to `pixels` exactly.
fn lzw_encode_without_eoi(pixels: &[u8]) -> Vec<u8> {
    let full_stream = lzw_encode_with_eoi(pixels);

    let mut best = full_stream.clone();
    for trimmed_len in (0..full_stream.len()).rev() {
        let candidate = &full_stream[..trimmed_len];
        match try_decode_lzw_tolerant(candidate, pixels.len()) {
            Some(decoded) if decoded == pixels => best = candidate.to_vec(),
            _ => break,
        }
    }

    assert!(
        best.len() < full_stream.len(),
        "could not find a shorter, still-fully-decodable prefix of the LZW stream; \
         the EOI code may not be trailing byte-aligned data for this input"
    );
    best
}

/// Hand-assembles a minimal, valid, single-tile, classic (non-BigTIFF),
/// little-endian, 8-bit grayscale TIFF around a pre-encoded LZW tile.
///
/// `tile_width`/`tile_length` may be larger than `width`/`height`, in which
/// case the tile is expected to be padded on disk -- exactly the situation
/// where the decoder has to skip trailing padding bytes inside the
/// compressed stream.
fn build_single_tile_lzw_tiff(
    width: u32,
    height: u32,
    tile_width: u32,
    tile_length: u32,
    lzw_tile: &[u8],
) -> Vec<u8> {
    const HEADER_LEN: u32 = 8;
    let tile_offset = HEADER_LEN;
    let tile_len = lzw_tile.len() as u32;

    let mut buf = Vec::new();

    // -- Header --
    buf.extend_from_slice(b"II");
    buf.extend_from_slice(&42u16.to_le_bytes());
    let ifd_offset_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(buf.len() as u32, HEADER_LEN);

    // -- Tile data --
    buf.extend_from_slice(lzw_tile);
    if buf.len() % 2 == 1 {
        buf.push(0);
    }
    let ifd_offset = buf.len() as u32;
    buf[ifd_offset_pos..ifd_offset_pos + 4].copy_from_slice(&ifd_offset.to_le_bytes());

    // -- IFD --
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;
    let entries: &[(u16, u16, u32, u32)] = &[
        (256, TYPE_SHORT, 1, width),       // ImageWidth
        (257, TYPE_SHORT, 1, height),      // ImageLength
        (258, TYPE_SHORT, 1, 8),           // BitsPerSample
        (259, TYPE_SHORT, 1, 5),           // Compression = LZW
        (262, TYPE_SHORT, 1, 1),           // PhotometricInterpretation = BlackIsZero
        (277, TYPE_SHORT, 1, 1),           // SamplesPerPixel
        (322, TYPE_SHORT, 1, tile_width),  // TileWidth
        (323, TYPE_SHORT, 1, tile_length), // TileLength
        (324, TYPE_LONG, 1, tile_offset),  // TileOffsets
        (325, TYPE_LONG, 1, tile_len),     // TileByteCounts
    ];

    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for &(tag, ty, count, value) in entries {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        if ty == TYPE_SHORT {
            buf.extend_from_slice(&(value as u16).to_le_bytes());
            buf.extend_from_slice(&[0u8; 2]);
        } else {
            buf.extend_from_slice(&value.to_le_bytes());
        }
    }
    buf.extend_from_slice(&0u32.to_le_bytes()); // next IFD offset (none)

    buf
}

/// Hand-assembles a minimal, valid, single-strip, classic (non-BigTIFF),
/// little-endian, 8-bit grayscale TIFF around a pre-encoded LZW strip.
fn build_single_strip_lzw_tiff(width: u32, height: u32, lzw_strip: &[u8]) -> Vec<u8> {
    const HEADER_LEN: u32 = 8;
    let strip_offset = HEADER_LEN;
    let strip_len = lzw_strip.len() as u32;

    let mut buf = Vec::new();

    // -- Header --
    buf.extend_from_slice(b"II");
    buf.extend_from_slice(&42u16.to_le_bytes());
    // IFD offset patched in below, once we know the (possibly padded) length
    // of the strip data that precedes it.
    let ifd_offset_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(buf.len() as u32, HEADER_LEN);

    // -- Strip data --
    buf.extend_from_slice(lzw_strip);
    if buf.len() % 2 == 1 {
        // IFDs must start at a word-aligned offset.
        buf.push(0);
    }
    let ifd_offset = buf.len() as u32;
    buf[ifd_offset_pos..ifd_offset_pos + 4].copy_from_slice(&ifd_offset.to_le_bytes());

    // -- IFD --
    // (tag, type, count, value) -- every value here fits in the inline
    // 4-byte value field (SHORT count=1 or LONG count=1), so no separate
    // out-of-line value storage is needed.
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;
    let entries: &[(u16, u16, u32, u32)] = &[
        (256, TYPE_SHORT, 1, width),       // ImageWidth
        (257, TYPE_SHORT, 1, height),      // ImageLength
        (258, TYPE_SHORT, 1, 8),           // BitsPerSample
        (259, TYPE_SHORT, 1, 5),           // Compression = LZW
        (262, TYPE_SHORT, 1, 1),           // PhotometricInterpretation = BlackIsZero
        (273, TYPE_LONG, 1, strip_offset), // StripOffsets
        (277, TYPE_SHORT, 1, 1),           // SamplesPerPixel
        (278, TYPE_LONG, 1, height),       // RowsPerStrip (single strip covers the image)
        (279, TYPE_LONG, 1, strip_len),    // StripByteCounts
    ];

    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for &(tag, ty, count, value) in entries {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        if ty == TYPE_SHORT {
            // SHORT values are left-justified within the 4-byte value field.
            buf.extend_from_slice(&(value as u16).to_le_bytes());
            buf.extend_from_slice(&[0u8; 2]);
        } else {
            buf.extend_from_slice(&value.to_le_bytes());
        }
    }
    buf.extend_from_slice(&0u32.to_le_bytes()); // next IFD offset (none)

    buf
}

#[test]
fn lzw_strip_missing_eoi_code_still_decodes() {
    let width = 16u32;
    let height = 16u32;
    // A pattern with some repetition (so LZW actually builds up its
    // dictionary across multiple codes, like a real image) but not so
    // uniform that a single code could encode the entire strip.
    let pixels: Vec<u8> = (0..width * height)
        .map(|i| {
            let x = i % width;
            let y = i / width;
            ((x * 7 + y * 3) % 251) as u8
        })
        .collect();

    let lzw_strip = lzw_encode_without_eoi(&pixels);

    // Sanity check on the test setup itself: the crafted stream must really
    // be shorter than a normal, EOI-terminated encode of the same pixels,
    // otherwise this test would not exercise the missing-EOI path at all.
    let lzw_strip_with_eoi = lzw_encode_with_eoi(&pixels);
    assert!(
        lzw_strip.len() < lzw_strip_with_eoi.len(),
        "test setup bug: the no-EOI stream ({} bytes) should be strictly \
         shorter than the EOI-terminated stream ({} bytes)",
        lzw_strip.len(),
        lzw_strip_with_eoi.len()
    );

    let tiff_bytes = build_single_strip_lzw_tiff(width, height, &lzw_strip);

    let mut decoder =
        Decoder::open(Cursor::new(tiff_bytes)).expect("failed to open synthetic TIFF");
    decoder
        .next_image()
        .expect("failed to read synthetic TIFF IFD");
    let result = decoder.read_image().expect(
        "an LZW strip missing only its end-of-information code must still decode \
         (libtiff-compatible tolerance for issue #395)",
    );

    match result {
        DecodingSampleBuffer::U8(decoded) => {
            assert_eq!(
                decoded, pixels,
                "decoded pixels must match the original data exactly"
            );
        }
        other => panic!("unexpected buffer kind: {other:?}"),
    }
}

/// The tiled variant of the missing-EOI scenario, matching the shape of the
/// report in issue #395 (tiled LZW). This one goes further than the strip
/// test above: the image width (20) is smaller than the tile width (32), so
/// every tile row carries 12 trailing padding bytes that the decoder must
/// read-and-discard from the compressed stream. The crafted stream omits
/// both the final row's trailing padding *and* the EOI code -- exactly the
/// kind of shortcut encoders take on the last coding unit of an image. All
/// real pixel bytes are present.
///
/// Unlike the strip test, this exercises the fixed code path directly: while
/// skipping the (absent) final padding the reader hits end-of-input without
/// an EOI code, which previously produced a hard "no lzw end code found"
/// error even though every requested pixel byte had already been decoded.
#[test]
fn tiled_lzw_missing_final_padding_and_eoi_still_decodes() {
    let width = 20u32;
    let height = 32u32;
    let tile_width = 32u32;
    let tile_length = 32u32;

    // The pixel data that should come out of the decoder (width x height).
    let pixels: Vec<u8> = (0..width * height)
        .map(|i| {
            let x = i % width;
            let y = i / width;
            ((x * 7 + y * 3) % 251) as u8
        })
        .collect();

    // The padded on-disk tile content (tile_width x tile_length), zero-filled
    // on the right where the image doesn't reach.
    let mut tile = vec![0u8; (tile_width * tile_length) as usize];
    for y in 0..height {
        for x in 0..width {
            tile[(y * tile_width + x) as usize] = pixels[(y * width + x) as usize];
        }
    }

    // Encode the padded tile, but stop before the final row's trailing
    // padding bytes -- then additionally strip the EOI code and its
    // byte-alignment padding, self-verified the same way as above.
    let final_padding = (tile_width - width) as usize;
    let encoded_content = &tile[..tile.len() - final_padding];
    let lzw_tile = lzw_encode_without_eoi(encoded_content);

    let tiff_bytes = build_single_tile_lzw_tiff(width, height, tile_width, tile_length, &lzw_tile);

    let mut decoder =
        Decoder::open(Cursor::new(tiff_bytes)).expect("failed to open synthetic TIFF");
    decoder
        .next_image()
        .expect("failed to read synthetic TIFF IFD");
    let result = decoder.read_image().expect(
        "a tiled LZW stream that contains every pixel byte but omits the final \
         tile-padding bytes and the end-of-information code must still decode \
         (libtiff-compatible tolerance for issue #395)",
    );

    match result {
        DecodingSampleBuffer::U8(decoded) => {
            assert_eq!(
                decoded, pixels,
                "decoded pixels must match the original data exactly"
            );
        }
        other => panic!("unexpected buffer kind: {other:?}"),
    }
}

/// A stream that runs out of input before producing the *full* expected
/// output (i.e. genuinely truncated/corrupt, not just missing the EOI code)
/// must still fail. The tolerance added for issue #395 is scoped to "ran out
/// of input right as the last real byte was produced", not "ran out of input
/// early".
#[test]
fn lzw_strip_truncated_mid_stream_still_errors() {
    let width = 16u32;
    let height = 16u32;
    let pixels: Vec<u8> = (0..width * height)
        .map(|i| {
            let x = i % width;
            let y = i / width;
            ((x * 7 + y * 3) % 251) as u8
        })
        .collect();

    let mut lzw_strip = lzw_encode_without_eoi(&pixels);
    // Chop off roughly the last quarter of the (already EOI-less) stream, so
    // even the real pixel data is incomplete.
    lzw_strip.truncate(lzw_strip.len() * 3 / 4);

    let tiff_bytes = build_single_strip_lzw_tiff(width, height, &lzw_strip);

    let mut decoder =
        Decoder::open(Cursor::new(tiff_bytes)).expect("failed to open synthetic TIFF");
    decoder
        .next_image()
        .expect("failed to read synthetic TIFF IFD");
    let result = decoder.read_image();
    assert!(
        result.is_err(),
        "a stream truncated before all real pixel data was produced must still fail, \
         not silently return partial/zero-padded data"
    );
}
