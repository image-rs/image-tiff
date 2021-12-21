extern crate tiff;

use std::io::Cursor;
use tiff::{
    decoder::{Decoder, DecodingResult},
    encoder::{colortype, compression::*, TiffEncoder},
    tags::Tag,
};

trait TestImage {
    const WIDTH: u32;
    const HEIGHT: u32;
    type PixelType;

    fn generate() -> Vec<Self::PixelType>;
}

struct TestImageColor;
impl TestImage for TestImageColor {
    const WIDTH: u32 = 1;
    const HEIGHT: u32 = 7;
    type PixelType = u16;

    fn generate() -> Vec<Self::PixelType> {
        let mut data: Vec<u16> = Vec::with_capacity((Self::WIDTH * Self::HEIGHT) as usize * 3);
        for x in 0..Self::WIDTH {
            for y in 0..Self::HEIGHT {
                let val = (x + y) % Self::PixelType::MAX as u32;
                data.extend(std::iter::repeat(val as Self::PixelType).take(3));
            }
        }
        assert_eq!(data.len() as u32, Self::WIDTH * Self::HEIGHT * 3);
        data
    }
}

struct TestImageGrayscale;
impl TestImage for TestImageGrayscale {
    const WIDTH: u32 = 21;
    const HEIGHT: u32 = 10;
    type PixelType = u8;

    fn generate() -> Vec<Self::PixelType> {
        let mut data: Vec<u8> = Vec::with_capacity((Self::WIDTH * Self::HEIGHT) as usize);
        for x in 0..Self::WIDTH {
            for y in 0..Self::HEIGHT {
                let val = (x + y) % Self::PixelType::MAX as u32;
                data.push(val as Self::PixelType);
            }
        }
        assert_eq!(data.len() as u32, Self::WIDTH * Self::HEIGHT);
        data
    }
}

fn encode_decode_with_compression<C: Compression + Clone>(compression: C) {
    let mut data = Cursor::new(Vec::new());

    let image_data_rgb = TestImageColor::generate();
    let image_data_gray = TestImageGrayscale::generate();

    // Encode tiff with compression
    {
        // Create a multipage image with 2 images
        let mut encoder = TiffEncoder::new(&mut data).unwrap();

        // Write first colored image
        let image_rgb = encoder
            .new_image_with_compression::<colortype::RGB16, C>(
                TestImageColor::WIDTH,
                TestImageColor::HEIGHT,
                compression.clone(),
            )
            .unwrap();
        image_rgb.write_data(&image_data_rgb).unwrap();

        // Write second grayscale image
        let image_gray = encoder
            .new_image_with_compression::<colortype::Gray8, C>(
                TestImageGrayscale::WIDTH,
                TestImageGrayscale::HEIGHT,
                compression,
            )
            .unwrap();
        image_gray.write_data(&image_data_gray).unwrap();
    }

    // Decode tiff
    data.set_position(0);
    {
        let mut decoder = Decoder::new(data).unwrap();

        // Check the RGB image
        assert_eq!(
            decoder
                .get_tag(Tag::ImageWidth)
                .unwrap()
                .into_u32()
                .unwrap(),
            TestImageColor::WIDTH
        );
        assert_eq!(
            match decoder.read_image() {
                Ok(DecodingResult::U16(image_data)) => image_data,
                unexpected => panic!("Descoding RGB failed: {:?}", unexpected),
            },
            image_data_rgb
        );

        // Check the grayscale image
        decoder.next_image().unwrap();
        assert_eq!(
            match decoder.read_image() {
                Ok(DecodingResult::U8(image_data)) => image_data,
                unexpected => panic!("Decoding grayscale failed: {:?}", unexpected),
            },
            image_data_gray
        );
    }
}

#[test]
fn encode_decode_without_compression() {
    encode_decode_with_compression(Uncompressed);
}

#[test]
fn encode_decode_with_lzw() {
    encode_decode_with_compression(Lzw::default());
}

#[test]
fn encode_decode_with_deflate() {
    encode_decode_with_compression(Deflate::with_level(DeflateLevel::Fast));
    encode_decode_with_compression(Deflate::with_level(DeflateLevel::Balanced));
    encode_decode_with_compression(Deflate::with_level(DeflateLevel::Best));
}
