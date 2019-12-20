extern crate tiff;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::encoder::{colortype, TiffEncoder, SRational};
use tiff::ColorType;
use tiff::tags::Tag;

use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::PathBuf;

#[test]
fn encode_decode() {
    let mut image_data = Vec::new();
    for x in 0..100 {
        for y in 0..100u8 {
            let val = x + y;
            image_data.push(val);
            image_data.push(val);
            image_data.push(val);
        }
    }
    let mut file = Cursor::new(Vec::new());
    {
        let mut tiff = TiffEncoder::new(&mut file).unwrap();

        tiff.write_image::<colortype::RGB8>(100, 100, &image_data)
            .unwrap();
    }
    {
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut decoder = Decoder::new(&mut file).unwrap();
        assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
        assert_eq!(decoder.dimensions().unwrap(), (100, 100));
        if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}

#[test]
/// Test that attempting to encode when the input buffer is undersized returns
/// an error rather than panicking.
/// See: https://github.com/PistonDevelopers/image-tiff/issues/35
fn test_encode_undersized_buffer() {
    let input_data = vec![1, 2, 3];
    let output = Vec::new();
    let mut output_stream = Cursor::new(output);
    if let Ok(mut tiff) = TiffEncoder::new(&mut output_stream) {
        let res = tiff.write_image::<colortype::RGB8>(50, 50, &input_data);
        assert!(res.is_err());
    }
}

const TEST_IMAGE_DIR: &str = "./tests/images/";

macro_rules! test_roundtrip {
    ($name:ident, $buffer:ident, $buffer_ty:ty) => {
        fn $name<C: colortype::ColorType<Inner=$buffer_ty>>(file: &str, expected_type: ColorType) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let img_file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
            assert_eq!(decoder.colortype().unwrap(), expected_type);

            let image_data = match decoder.read_image().unwrap() {
                DecodingResult::$buffer(res) => res,
                _ => panic!("Wrong data type"),
            };

            let mut file = Cursor::new(Vec::new());
            {
                let mut tiff = TiffEncoder::new(&mut file).unwrap();

                let (width, height) = decoder.dimensions().unwrap();
                tiff.write_image::<C>(width, height, &image_data)
                    .unwrap();
            }
            file.seek(SeekFrom::Start(0)).unwrap();
            {
                let mut decoder = Decoder::new(&mut file).unwrap();
                if let DecodingResult::$buffer(img_res) = decoder.read_image().unwrap() {
                    assert_eq!(image_data, img_res);
                } else {
                    panic!("Wrong data type");
                }
            }
        }
    };
}

test_roundtrip!(test_u8_roundtrip, U8, u8);
test_roundtrip!(test_u16_roundtrip, U16, u16);
test_roundtrip!(test_u32_roundtrip, U32, u32);
test_roundtrip!(test_u64_roundtrip, U64, u64);

#[test]
fn test_gray_u8_roundtrip() {
    test_u8_roundtrip::<colortype::Gray8>("minisblack-1c-8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_rgb_u8_roundtrip() {
    test_u8_roundtrip::<colortype::RGB8>("rgb-3c-8b.tiff", ColorType::RGB(8));
}

#[test]
fn test_cmyk_u8_roundtrip() {
    test_u8_roundtrip::<colortype::CMYK8>("cmyk-3c-8b.tiff", ColorType::CMYK(8));
}

#[test]
fn test_gray_u16_roundtrip() {
    test_u16_roundtrip::<colortype::Gray16>("minisblack-1c-16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_rgb_u16_roundtrip() {
    test_u16_roundtrip::<colortype::RGB16>("rgb-3c-16b.tiff", ColorType::RGB(16));
}

#[test]
fn test_cmyk_u16_roundtrip() {
    test_u16_roundtrip::<colortype::CMYK16>("cmyk-3c-16b.tiff", ColorType::CMYK(16));
}

#[test]
fn test_gray_u32_roundtrip() {
    test_u32_roundtrip::<colortype::Gray32>("gradient-1c-32b.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_u32_roundtrip() {
    test_u32_roundtrip::<colortype::RGB32>("gradient-3c-32b.tiff", ColorType::RGB(32));
}

#[test]
fn test_gray_u64_roundtrip() {
    test_u64_roundtrip::<colortype::Gray64>("gradient-1c-64b.tiff", ColorType::Gray(64));
}

#[test]
fn test_rgb_u64_roundtrip() {
    test_u64_roundtrip::<colortype::RGB64>("gradient-3c-64b.tiff", ColorType::RGB(64));
}

#[test]
fn test_multiple_byte() {
    let mut data = Cursor::new(Vec::new());

    {
        let mut tiff = TiffEncoder::new(&mut data).unwrap();
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(1, 1).unwrap();
        let encoder = image_encoder.encoder();

        encoder.write_tag(Tag::Unknown(65000), &[1_u8][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65001), &[1_u8, 2][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65002), &[1_u8, 2, 3][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65003), &[1_u8, 2, 3, 4][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65004), &[1_u8, 2, 3, 4, 5][..]).unwrap();
    }

    data.set_position(0);
    {
        let mut decoder = Decoder::new(&mut data).unwrap();

        assert_eq!(decoder.get_tag(Tag::Unknown(65000)).unwrap().into_u32_vec().unwrap(), [1]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65001)).unwrap().into_u32_vec().unwrap(), [1, 2]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65002)).unwrap().into_u32_vec().unwrap(), [1, 2, 3]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65003)).unwrap().into_u32_vec().unwrap(), [1, 2, 3, 4]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65004)).unwrap().into_u32_vec().unwrap(), [1, 2, 3, 4, 5]);
    }
}

#[test]
/// Test writing signed tags from TIFF 6.0
fn test_signed() {
    let mut data = Cursor::new(Vec::new());

    {
        let mut tiff = TiffEncoder::new(&mut data).unwrap();
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(1, 1).unwrap();
        let encoder = image_encoder.encoder();

        //Use the "reusable" tags section as per the TIFF6 spec
        encoder.write_tag(Tag::Unknown(65000), -1_i8).unwrap();
        encoder.write_tag(Tag::Unknown(65001), &[-1_i8][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65002), &[-1_i8, 2][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65003), &[-1_i8, 2, -3][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65004), &[-1_i8, 2, -3, 4][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65005), &[-1_i8, 2, -3, 4, -5][..]).unwrap();

        encoder.write_tag(Tag::Unknown(65010), -1_i16).unwrap();
        encoder.write_tag(Tag::Unknown(65011), -1_i16).unwrap();
        encoder.write_tag(Tag::Unknown(65012), &[-1_i16, 2][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65013), &[-1_i16, 2, -3][..]).unwrap();

        encoder.write_tag(Tag::Unknown(65020), -1_i32).unwrap();
        encoder.write_tag(Tag::Unknown(65021), &[-1_i32][..]).unwrap();
        encoder.write_tag(Tag::Unknown(65022), &[-1_i32, 2][..]).unwrap();

        encoder.write_tag(Tag::Unknown(65030), SRational { n: -1, d: 100 }).unwrap();
        encoder.write_tag(Tag::Unknown(65031), &[SRational { n: -1, d: 100 }, SRational { n: 2, d: 100 }][..]).unwrap();
    }

    //Rewind the cursor for reading
    data.set_position(0);
    {
        let mut decoder = Decoder::new(&mut data).unwrap();

        assert_eq!(decoder.get_tag(Tag::Unknown(65000)).unwrap().into_i32().unwrap(), -1, );
        assert_eq!(decoder.get_tag(Tag::Unknown(65001)).unwrap().into_i32_vec().unwrap(), [-1]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65002)).unwrap().into_i32_vec().unwrap(), [-1, 2]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65003)).unwrap().into_i32_vec().unwrap(), [-1, 2, -3]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65004)).unwrap().into_i32_vec().unwrap(), [-1, 2, -3, 4]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65005)).unwrap().into_i32_vec().unwrap(), [-1, 2, -3, 4, -5], );

        assert_eq!(decoder.get_tag(Tag::Unknown(65010)).unwrap().into_i32().unwrap(), -1);
        assert_eq!(decoder.get_tag(Tag::Unknown(65011)).unwrap().into_i32_vec().unwrap(), [-1]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65012)).unwrap().into_i32_vec().unwrap(), [-1, 2]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65013)).unwrap().into_i32_vec().unwrap(), [-1, 2, -3]);

        assert_eq!(decoder.get_tag(Tag::Unknown(65020)).unwrap().into_i32().unwrap(), -1);
        assert_eq!(decoder.get_tag(Tag::Unknown(65021)).unwrap().into_i32_vec().unwrap(), [-1]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65022)).unwrap().into_i32_vec().unwrap(), [-1, 2]);

        assert_eq!(decoder.get_tag(Tag::Unknown(65030)).unwrap().into_i32_vec().unwrap(), [-1, 100]);
        assert_eq!(decoder.get_tag(Tag::Unknown(65031)).unwrap().into_i32_vec().unwrap(), [-1_i32, 100, 2, 100]);
    }
}

#[test]
/// check multipage image handling
fn test_multipage_image() {
    let mut img_file = Cursor::new(Vec::new());

    {
        // first create a multipage image with 2 images
        let mut img_encoder = TiffEncoder::new(&mut img_file).unwrap();

        // write first grayscale image (2x2 16-bit)
        let img1: Vec<u16> = [1, 2, 3, 4].to_vec();
        img_encoder.write_image::<colortype::Gray16>(2, 2, &img1[..]).unwrap();
        // write second grayscale image (3x3 8-bit)
        let img2: Vec<u8> = [9, 8, 7, 6, 5, 4, 3, 2, 1].to_vec();
        img_encoder.write_image::<colortype::Gray8>(3, 3, &img2[..]).unwrap();
    }

    // seek to the beginning of the file, so that it can be decoded
    img_file.seek(SeekFrom::Start(0)).unwrap();

    {
        let mut img_decoder = Decoder::new(&mut img_file).unwrap();

        // check the dimensions of the image in the first page
        assert_eq!(img_decoder.dimensions().unwrap(), (2, 2));
        img_decoder.next_image().unwrap();
        // check the dimensions of the image in the second page
        assert_eq!(img_decoder.dimensions().unwrap(), (3, 3));
    }
}
