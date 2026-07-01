extern crate tiff;

use tiff::decoder::{Decoder, DecodingSampleBuffer};
use tiff::encoder::{colortype, Predictor, TiffEncoder};
use tiff::ColorType;

use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

macro_rules! test_predict {
    ($name:ident, $buffer:ident, $buffer_ty:ty) => {
        fn $name<C: colortype::ColorType<Inner = $buffer_ty>>(
            file: &str,
            expected_type: ColorType,
        ) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::open(file).expect("Cannot create decoder!");

            decoder.next_image().unwrap();
            assert_eq!(decoder.colortype().unwrap(), expected_type);
            let image_data = match decoder.read_image().unwrap() {
                DecodingSampleBuffer::$buffer(res) => res,
                _ => panic!("Wrong data type"),
            };

            let mut predicted = Vec::with_capacity(image_data.len());
            C::horizontal_predict(&image_data, &mut predicted);

            let sample_size = C::SAMPLE_FORMAT.len();

            (0..sample_size).for_each(|i| {
                assert_eq!(predicted[i], image_data[i]);
            });

            (sample_size..image_data.len()).for_each(|i| {
                predicted[i] = predicted[i].wrapping_add(predicted[i - sample_size]);
                assert_eq!(predicted[i], image_data[i]);
            });
        }
    };
}

test_predict!(test_u8_predict, U8, u8);
test_predict!(test_i8_predict, I8, i8);
test_predict!(test_u16_predict, U16, u16);
test_predict!(test_i16_predict, I16, i16);
test_predict!(test_u32_predict, U32, u32);
test_predict!(test_u64_predict, U64, u64);

#[test]
fn test_gray_u8_predict() {
    test_u8_predict::<colortype::Gray8>("minisblack-1c-8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_gray_i8_predict() {
    test_i8_predict::<colortype::GrayI8>("minisblack-1c-i8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_rgb_u8_predict() {
    test_u8_predict::<colortype::RGB8>("rgb-3c-8b.tiff", ColorType::RGB(8));
}

#[test]
fn test_cmyk_u8_predict() {
    test_u8_predict::<colortype::CMYK8>("cmyk-3c-8b.tiff", ColorType::CMYK(8));
}

#[test]
fn test_gray_u16_predict() {
    test_u16_predict::<colortype::Gray16>("minisblack-1c-16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_gray_i16_predict() {
    test_i16_predict::<colortype::GrayI16>("minisblack-1c-i16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_rgb_u16_predict() {
    test_u16_predict::<colortype::RGB16>("rgb-3c-16b.tiff", ColorType::RGB(16));
}

#[test]
fn test_cmyk_u16_predict() {
    test_u16_predict::<colortype::CMYK16>("cmyk-3c-16b.tiff", ColorType::CMYK(16));
}

#[test]
fn test_gray_u32_predict() {
    test_u32_predict::<colortype::Gray32>("gradient-1c-32b.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_u32_predict() {
    test_u32_predict::<colortype::RGB32>("gradient-3c-32b.tiff", ColorType::RGB(32));
}

#[test]
fn test_gray_u64_predict() {
    test_u64_predict::<colortype::Gray64>("gradient-1c-64b.tiff", ColorType::Gray(64));
}

#[test]
fn test_rgb_u64_predict() {
    test_u64_predict::<colortype::RGB64>("gradient-3c-64b.tiff", ColorType::RGB(64));
}

#[test]
fn test_ycbcr_u8_predict() {
    test_u8_predict::<colortype::YCbCr8>("tiled-jpeg-ycbcr.tif", ColorType::YCbCr(8));
}

macro_rules! test_predict_roundtrip {
    ($name:ident, $buffer:ident, $buffer_ty:ty) => {
        fn $name<C: colortype::ColorType<Inner = $buffer_ty>>(
            file: &str,
            expected_type: ColorType,
        ) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let img_file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::open(img_file).expect("Cannot create decoder");

            decoder.next_image().unwrap();
            assert_eq!(decoder.colortype().unwrap(), expected_type);

            let image_data = match decoder.read_image().unwrap() {
                DecodingSampleBuffer::$buffer(res) => res,
                _ => panic!("Wrong data type"),
            };

            let mut file = Cursor::new(Vec::new());
            {
                let mut tiff = TiffEncoder::new(&mut file)
                    .unwrap()
                    .with_predictor(Predictor::Horizontal);

                let (width, height) = decoder.dimensions().unwrap();
                tiff.write_image::<C>(width, height, &image_data).unwrap();
            }
            file.seek(SeekFrom::Start(0)).unwrap();
            {
                let mut decoder = Decoder::open(&mut file).unwrap();
                decoder.next_image().unwrap();
                if let DecodingSampleBuffer::$buffer(img_res) =
                    decoder.read_image().expect("Decoding image failed")
                {
                    assert_eq!(image_data, img_res);
                } else {
                    panic!("Wrong data type");
                }
            }
        }
    };
}

test_predict_roundtrip!(test_u8_predict_roundtrip, U8, u8);
test_predict_roundtrip!(test_i8_predict_roundtrip, I8, i8);
test_predict_roundtrip!(test_u16_predict_roundtrip, U16, u16);
test_predict_roundtrip!(test_i16_predict_roundtrip, I16, i16);
test_predict_roundtrip!(test_u32_predict_roundtrip, U32, u32);
test_predict_roundtrip!(test_u64_predict_roundtrip, U64, u64);

#[test]
fn test_gray_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::Gray8>("minisblack-1c-8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_gray_i8_predict_roundtrip() {
    test_i8_predict_roundtrip::<colortype::GrayI8>("minisblack-1c-i8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_rgb_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::RGB8>("rgb-3c-8b.tiff", ColorType::RGB(8));
}

#[test]
fn test_cmyk_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::CMYK8>("cmyk-3c-8b.tiff", ColorType::CMYK(8));
}

#[test]
fn test_gray_u16_predict_roundtrip() {
    test_u16_predict_roundtrip::<colortype::Gray16>("minisblack-1c-16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_gray_i16_predict_roundtrip() {
    test_i16_predict_roundtrip::<colortype::GrayI16>(
        "minisblack-1c-i16b.tiff",
        ColorType::Gray(16),
    );
}

#[test]
fn test_rgb_u16_predict_roundtrip() {
    test_u16_predict_roundtrip::<colortype::RGB16>("rgb-3c-16b.tiff", ColorType::RGB(16));
}

#[test]
fn test_cmyk_u16_predict_roundtrip() {
    test_u16_predict_roundtrip::<colortype::CMYK16>("cmyk-3c-16b.tiff", ColorType::CMYK(16));
}

#[test]
fn test_gray_u32_predict_roundtrip() {
    test_u32_predict_roundtrip::<colortype::Gray32>("gradient-1c-32b.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_u32_predict_roundtrip() {
    test_u32_predict_roundtrip::<colortype::RGB32>("gradient-3c-32b.tiff", ColorType::RGB(32));
}

#[test]
fn test_gray_u64_predict_roundtrip() {
    test_u64_predict_roundtrip::<colortype::Gray64>("gradient-1c-64b.tiff", ColorType::Gray(64));
}

#[test]
fn test_rgb_u64_predict_roundtrip() {
    test_u64_predict_roundtrip::<colortype::RGB64>("gradient-3c-64b.tiff", ColorType::RGB(64));
}

#[test]
fn test_ycbcr_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::YCbCr8>("tiled-jpeg-ycbcr.tif", ColorType::YCbCr(8));
}

// ---------------------------------------------------------------------------
// Extended bit depths (9-15 promoted to u16, 17-31 promoted to u32).
//
// A horizontal-predicted n-bit sample stores its delta in exactly n bits,
// i.e. modulo 2^n, so reconstruction must also be modulo 2^n. The decoder
// accumulates in the promoted storage width and reduces back to n bits;
// these tests pin that reduction down with exact pixel values, including
// deltas that wrap modulo 2^n. The encoder does not support extended
// depths, so the TIFFs are hand-built.
// ---------------------------------------------------------------------------

/// Hand-build a minimal little-endian, single-strip, uncompressed grayscale
/// TIFF with `Predictor::Horizontal` and the given (possibly non-standard)
/// bit depth. `pixels` are the intended sample values; they are converted to
/// horizontal differences modulo `2^bits` and bit-packed MSB-first with each
/// row padded to a byte boundary — the only representation an n-bit field
/// can hold.
fn build_gray_hpredict_tiff(
    width: u32,
    height: u32,
    bits: u8,
    photometric: u16,
    pixels: &[u32],
) -> Vec<u8> {
    let max = ((1u64 << bits) - 1) as u32;
    let row_samples = width as usize;
    assert_eq!(pixels.len(), row_samples * height as usize);
    assert!(pixels.iter().all(|&v| v <= max));

    // Per-row horizontal differencing modulo 2^bits.
    let mut deltas = Vec::with_capacity(pixels.len());
    for row in pixels.chunks(row_samples) {
        for (i, &v) in row.iter().enumerate() {
            deltas.push(if i == 0 {
                v
            } else {
                v.wrapping_sub(row[i - 1]) & max
            });
        }
    }

    // Bit-pack MSB-first, each row padded to a byte boundary.
    let row_bytes = (row_samples * bits as usize).div_ceil(8);
    let mut data = vec![0u8; row_bytes * height as usize];
    for (r, row) in deltas.chunks(row_samples).enumerate() {
        for (i, &d) in row.iter().enumerate() {
            let start = r * row_bytes * 8 + i * bits as usize;
            for (off, b) in (start..).zip((0..bits).rev()) {
                if (d >> b) & 1 == 1 {
                    data[off / 8] |= 0x80 >> (off % 8);
                }
            }
        }
    }

    // Classic little-endian TIFF: header, strip data, then a single IFD.
    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    let data_offset = 8u32;
    let ifd_offset = data_offset + data.len() as u32;
    out.extend_from_slice(&ifd_offset.to_le_bytes());
    out.extend_from_slice(&data);

    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    let entries: &[(u16, u16, u32)] = &[
        (256, LONG, width),                   // ImageWidth
        (257, LONG, height),                  // ImageLength
        (258, SHORT, u32::from(bits)),        // BitsPerSample
        (259, SHORT, 1),                      // Compression: none
        (262, SHORT, u32::from(photometric)), // PhotometricInterpretation
        (273, LONG, data_offset),             // StripOffsets
        (277, SHORT, 1),                      // SamplesPerPixel
        (278, LONG, height),                  // RowsPerStrip
        (279, LONG, data.len() as u32),       // StripByteCounts
        (317, SHORT, 2),                      // Predictor: horizontal
        (339, SHORT, 1),                      // SampleFormat: unsigned integer
    ];
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for &(tag, ty, value) in entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        // A count-1 SHORT payload occupies the low bytes of the value field.
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    out
}

fn decode_gray_u16(tiff: Vec<u8>, bits: u8) -> Vec<u16> {
    let mut decoder = Decoder::open(Cursor::new(tiff)).expect("Cannot create decoder");
    decoder.next_image().unwrap();
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(bits));
    match decoder.read_image().unwrap() {
        DecodingSampleBuffer::U16(res) => res,
        _ => panic!("Wrong data type"),
    }
}

fn decode_gray_u32(tiff: Vec<u8>, bits: u8) -> Vec<u32> {
    let mut decoder = Decoder::open(Cursor::new(tiff)).expect("Cannot create decoder");
    decoder.next_image().unwrap();
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(bits));
    match decoder.read_image().unwrap() {
        DecodingSampleBuffer::U32(res) => res,
        _ => panic!("Wrong data type"),
    }
}

#[test]
fn test_gray12_hpredict_wraparound_exact() {
    // Pixel row [4095, 0, 1] encodes as deltas [4095, 1, 1] (mod 2^12).
    // u16 accumulation yields [4095, 4096, 4097]; reduction modulo 2^12
    // must restore [4095, 0, 1]. Saturating instead would corrupt the two
    // wrapped samples to 4095.
    let tiff = build_gray_hpredict_tiff(3, 1, 12, 1, &[4095, 0, 1]);
    assert_eq!(decode_gray_u16(tiff, 12), vec![4095, 0, 1]);
}

#[test]
fn test_gray12_hpredict_wraparound_white_is_zero_exact() {
    // Same stored samples with PhotometricInterpretation = WhiteIsZero:
    // after the mod-2^12 reduction the decoder inverts as (2^12 - 1) - v.
    let tiff = build_gray_hpredict_tiff(3, 1, 12, 0, &[4095, 0, 1]);
    assert_eq!(decode_gray_u16(tiff, 12), vec![0, 4095, 4094]);
}

#[test]
fn test_gray20_hpredict_wraparound_exact() {
    // The same wrap-around shape on the u32 storage path (17-31 bits).
    let tiff = build_gray_hpredict_tiff(3, 1, 20, 1, &[0xF_FFFF, 0, 1]);
    assert_eq!(decode_gray_u32(tiff, 20), vec![0xF_FFFF, 0, 1]);
}

#[test]
fn test_extended_depth_hpredict_random_roundtrip() {
    // Deterministic pseudo-random pixels across a spread of extended depths;
    // encode-then-decode must be the identity for every sample, including
    // every mod-2^n wrap-around the differencing produces. Three rows verify
    // that prediction restarts per row.
    let mut state = 0x243F_6A88_85A3_08D3u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    for &bits in &[9u8, 11, 12, 15] {
        let max = (1u32 << bits) - 1;
        let pixels: Vec<u32> = (0..16 * 3).map(|_| next() & max).collect();
        let tiff = build_gray_hpredict_tiff(16, 3, bits, 1, &pixels);
        let expected: Vec<u16> = pixels.iter().map(|&v| v as u16).collect();
        assert_eq!(decode_gray_u16(tiff, bits), expected, "bits={bits}");
    }
    for &bits in &[17u8, 20, 25, 31] {
        let max = ((1u64 << bits) - 1) as u32;
        let pixels: Vec<u32> = (0..16 * 3).map(|_| next() & max).collect();
        let tiff = build_gray_hpredict_tiff(16, 3, bits, 1, &pixels);
        assert_eq!(decode_gray_u32(tiff, bits), pixels, "bits={bits}");
    }
}
