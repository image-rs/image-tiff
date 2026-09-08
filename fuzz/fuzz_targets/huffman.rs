#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;
use tiff::decoder::{Decoder, DecodingSampleBuffer, Limits};
use tiff::encoder::{colortype::Gray1, Compression, TiffEncoder};
use tiff::tags::Tag;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 || data.len() > 65_536 {
        return;
    }
    let width = u32::from(u16::from_le_bytes([data[0], data[1]])).max(1);
    let height = u32::from(data[2] % 8) + 1;
    let fill_order = u16::from(data[3] & 1) + 1;
    let payload = &data[4..];

    // Keep the IFD valid so mutations reach the Huffman decoder itself.
    // Multiple rows per strip, both FillOrders and both grayscale polarities.
    let mut file = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut file).unwrap();
        let mut directory = encoder.new_directory().unwrap();
        let offset = directory.write_data(payload).unwrap();
        directory.write_tag(Tag::ImageWidth, width).unwrap();
        directory.write_tag(Tag::ImageLength, height).unwrap();
        directory.write_tag(Tag::BitsPerSample, 1u16).unwrap();
        directory.write_tag(Tag::SamplesPerPixel, 1u16).unwrap();
        directory
            .write_tag(Tag::PhotometricInterpretation, u16::from(data[3] >> 1 & 1))
            .unwrap();
        directory.write_tag(Tag::Compression, 2u16).unwrap();
        directory.write_tag(Tag::FillOrder, fill_order).unwrap();
        directory.write_tag(Tag::RowsPerStrip, height).unwrap();
        directory
            .write_tag(Tag::StripOffsets, offset as u32)
            .unwrap();
        directory
            .write_tag(Tag::StripByteCounts, payload.len() as u32)
            .unwrap();
        directory.finish().unwrap();
    }
    file.set_position(0);
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 256 * 1024;
    limits.intermediate_buffer_size = 256 * 1024;
    let mut decoder = Decoder::open(file).unwrap().with_limits(limits);
    decoder.next_image().unwrap();
    let _ = decoder.read_image(); // Malformed codes must return errors, not panic.

    // Valid encoding must preserve every image bit, including partial rows.
    let stride = width.div_ceil(8) as usize;
    let mut pixels = vec![0u8; stride * height as usize];
    for (dst, src) in pixels.iter_mut().zip(payload.iter().cycle()) {
        *dst = *src;
    }
    let mut encoded = Cursor::new(Vec::new());
    TiffEncoder::new(&mut encoded)
        .unwrap()
        .with_compression(Compression::Huffman)
        .write_image::<Gray1>(width, height, &pixels)
        .unwrap();
    encoded.set_position(0);
    let mut decoder = Decoder::open(encoded).unwrap();
    decoder.next_image().unwrap();
    if width % 8 != 0 {
        for row in pixels.chunks_exact_mut(stride) {
            row[stride - 1] &= 0xFF << (8 - width % 8);
        }
    }
    match decoder.read_image().unwrap() {
        DecodingSampleBuffer::U8(decoded) => assert_eq!(decoded, pixels),
        _ => unreachable!(),
    }
});
