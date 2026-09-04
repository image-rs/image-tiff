extern crate tiff;

#[cfg(feature = "fax")]
use std::fs::File;
#[cfg(feature = "fax")]
use std::io::BufReader;
use std::io::Cursor;
use tiff::decoder::{Decoder, DecodingSampleBuffer};
use tiff::encoder::colortype;
use tiff::encoder::{Compression, Predictor, TiffEncoder};

fn roundtrip(compression: Compression, width: u32, height: u32, data: &[u8]) -> Vec<u8> {
    let mut tiff_data = Cursor::new(Vec::new());
    {
        let mut tiff = TiffEncoder::new(&mut tiff_data)
            .unwrap()
            .with_compression(compression);
        tiff.write_image::<colortype::Gray1>(width, height, data)
            .unwrap();
    }

    tiff_data.set_position(0);
    let mut decoder = Decoder::open(tiff_data).unwrap();
    decoder.next_image().unwrap();
    assert_eq!(decoder.dimensions().unwrap(), (width, height));
    assert_eq!(decoder.colortype().unwrap(), tiff::ColorType::Gray(1));

    let mut buffer = DecodingSampleBuffer::U8(vec![]);
    decoder.read_image_to_buffer(&mut buffer).unwrap();
    match buffer {
        DecodingSampleBuffer::U8(samples) => samples,
        _ => panic!("expected 8-bit samples"),
    }
}

#[cfg(feature = "fax")]
#[test]
fn gray1_huffman_roundtrip() {
    // Multiple of 8: 16x2 image with alternating and inverted rows
    let data = [0xAA, 0xAA, 0x55, 0x55];
    assert_eq!(roundtrip(Compression::Huffman, 16, 2, &data), data.to_vec());

    // Non-multiple of 8: 5x3 image; only the width pixels round-trip, the
    // row padding bits decode as zeros (0xF0 -> 11110___, 0x0F -> 00001___,
    // 0xFF -> 11111___)
    assert_eq!(
        roundtrip(Compression::Huffman, 5, 3, &[0xF0, 0x0F, 0xFF]),
        vec![0xF0, 0x08, 0xF8]
    );
}

#[test]
fn gray1_other_compressions_roundtrip() {
    let data = [0xA5, 0x5A, 0xC3, 0x3C, 0x00, 0xFF, 0xF0, 0x0F];
    for compression in [
        Compression::Uncompressed,
        Compression::Packbits,
        #[cfg(feature = "lzw")]
        Compression::Lzw,
        #[cfg(feature = "deflate")]
        Compression::Deflate(6),
        #[cfg(feature = "fax")]
        Compression::Huffman,
    ] {
        assert_eq!(
            roundtrip(compression, 16, 4, &data),
            data.to_vec(),
            "failed for {compression:?}"
        );
    }
}

#[test]
fn gray1_rejects_predictor() {
    let mut tiff_data = Cursor::new(Vec::new());
    let mut tiff = TiffEncoder::new(&mut tiff_data)
        .unwrap()
        .with_predictor(Predictor::Horizontal);
    let result = tiff.write_image::<colortype::Gray1>(16, 2, &[0xAA; 4]);
    assert!(result.is_err());
}

#[test]
fn gray1_rejects_undersized_data() {
    let mut tiff_data = Cursor::new(Vec::new());
    let mut tiff = TiffEncoder::new(&mut tiff_data).unwrap();
    // 16x2 needs 2 packed bytes per row = 4 bytes total; 3 is undersized
    let result = tiff.write_image::<colortype::Gray1>(16, 2, &[0xAA, 0xAA, 0xAA]);
    assert!(result.is_err());
}

/// Fixture provenance: `tests/images/ccitt1d.tiff` was converted from the
/// Group 4 image `fax4.tiff` with this crate (decode G4, re-encode the packed
/// bilevel rows as Gray1 + CCITT 1D Huffman).
#[cfg(feature = "fax")]
#[test]
fn gray1_ccitt1d_fixture_matches_source() {
    fn decode_packed(path: &str) -> ((u32, u32), Vec<u8>) {
        let file = File::open(path).unwrap();
        let mut decoder = Decoder::open(BufReader::new(file)).unwrap();
        decoder.next_image().unwrap();
        let dims = decoder.dimensions().unwrap();
        assert_eq!(decoder.colortype().unwrap(), tiff::ColorType::Gray(1));
        let mut buffer = DecodingSampleBuffer::U8(vec![]);
        decoder.read_image_to_buffer(&mut buffer).unwrap();
        match buffer {
            DecodingSampleBuffer::U8(samples) => (dims, samples),
            _ => panic!("expected 8-bit samples"),
        }
    }

    let (source_dims, source) = decode_packed("./tests/images/fax4.tiff");
    let (fixture_dims, converted) = decode_packed("./tests/images/ccitt1d.tiff");
    assert_eq!(source_dims, fixture_dims);
    assert_eq!(source, converted);
}

#[cfg(feature = "fax")]
#[test]
fn huffman_encodes_only_image_pixels() {
    for input in [0xF8, 0xFF] {
        let mut output = Cursor::new(Vec::new());
        TiffEncoder::new(&mut output)
            .unwrap()
            .with_compression(Compression::Huffman)
            .write_image::<colortype::Gray1>(5, 1, &[input])
            .unwrap();
        let bytes = output.into_inner();
        let mut decoder = Decoder::open(Cursor::new(&bytes)).unwrap();
        decoder.next_image().unwrap();
        let offset = decoder
            .current_ifd()
            .get_tag_u64(tiff::tags::Tag::StripOffsets)
            .unwrap() as usize;
        let len = decoder
            .current_ifd()
            .get_tag_u64(tiff::tags::Tag::StripByteCounts)
            .unwrap() as usize;
        // White(0) = 00110101, black(5) = 0011, then four padding bits.
        assert_eq!(&bytes[offset..offset + len], &[0x35, 0x30]);
    }
}

#[cfg(feature = "fax")]
#[test]
fn huffman_rejects_multirow_strips() {
    let mut output = Cursor::new(Vec::new());
    let mut encoder = TiffEncoder::new(&mut output)
        .unwrap()
        .with_compression(Compression::Huffman);
    let mut image = encoder.new_image::<colortype::Gray1>(8, 2).unwrap();
    assert!(image.rows_per_strip(2).is_err());
    assert!(image.rows_per_strip(0).is_err());
    assert_eq!(image.next_strip_sample_count(), 1);
    image.write_data(&[0xAA, 0x55]).unwrap();
}

#[cfg(feature = "fax")]
#[test]
fn huffman_rejects_non_bilevel_colors() {
    let mut output = Cursor::new(Vec::new());
    let mut encoder = TiffEncoder::new(&mut output)
        .unwrap()
        .with_compression(Compression::Huffman);
    assert!(encoder
        .write_image::<colortype::Gray8>(8, 1, &[128; 8])
        .is_err());
    assert!(encoder
        .write_image::<colortype::RGB8>(8, 1, &[128; 24])
        .is_err());
}

#[cfg(feature = "fax")]
#[test]
fn huffman_manual_strips_roundtrip() {
    let mut output = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut output)
            .unwrap()
            .with_compression(Compression::Huffman);
        let mut image = encoder.new_image::<colortype::Gray1>(5, 2).unwrap();
        image.write_strip(&[0xFF]).unwrap();
        image.write_strip(&[0xAA]).unwrap();
        image.finish().unwrap();
    }
    output.set_position(0);
    let mut decoder = Decoder::open(output).unwrap();
    decoder.next_image().unwrap();
    let mut buffer = DecodingSampleBuffer::U8(vec![]);
    decoder.read_image_to_buffer(&mut buffer).unwrap();
    match buffer {
        DecodingSampleBuffer::U8(data) => assert_eq!(data, [0xF8, 0xA8]),
        _ => panic!("expected packed bytes"),
    }
}
