extern crate tempfile;
extern crate tiff;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::encoder::{colortype, TiffEncoder};
use tiff::ColorType;

use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};

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
    let mut file = tempfile::tempfile().unwrap();
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


#[test]
fn test_gray_u8_roundtrip() {
    let img_file =
        File::open("./tests/images/minisblack-1c-8b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(8));

    let image_data = match decoder.read_image().unwrap() {
        DecodingResult::U8(res) => res,
        _ => panic!("Wrong data type"),
    };

    let mut file = tempfile::tempfile().unwrap();
    {
        let mut tiff = TiffEncoder::new(&mut file).unwrap();

        let (width, height) = decoder.dimensions().unwrap();
        tiff.write_image::<colortype::Gray8>(width, height, &image_data)
            .unwrap();
    }
    file.seek(SeekFrom::Start(0)).unwrap();
    {
        let mut decoder = Decoder::new(&mut file).unwrap();
        if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}

#[test]
fn test_rgb_u8_roundtrip() {
    let img_file = File::open("./tests/images/rgb-3c-8b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));

    let image_data = match decoder.read_image().unwrap() {
        DecodingResult::U8(res) => res,
        _ => panic!("Wrong data type"),
    };

    let mut file = tempfile::tempfile().unwrap();
    {
        let mut tiff = TiffEncoder::new(&mut file).unwrap();

        let (width, height) = decoder.dimensions().unwrap();
        tiff.write_image::<colortype::RGB8>(width, height, &image_data)
            .unwrap();
    }
    file.seek(SeekFrom::Start(0)).unwrap();
    {
        let mut decoder = Decoder::new(&mut file).unwrap();
        if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}

#[test]
fn test_gray_u16_roundtrip() {
    let img_file =
        File::open("./tests/images/minisblack-1c-16b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(16));

    let image_data = match decoder.read_image().unwrap() {
        DecodingResult::U16(res) => res,
        _ => panic!("Wrong data type"),
    };

    let mut file = tempfile::tempfile().unwrap();
    {
        let mut tiff = TiffEncoder::new(&mut file).unwrap();

        let (width, height) = decoder.dimensions().unwrap();
        tiff.write_image::<colortype::Gray16>(width, height, &image_data)
            .unwrap();
    }
    file.seek(SeekFrom::Start(0)).unwrap();
    {
        let mut decoder = Decoder::new(&mut file).unwrap();
        if let DecodingResult::U16(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}

#[test]
fn test_rgb_u16_roundtrip() {
    let img_file = File::open("./tests/images/rgb-3c-16b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(16));

    let image_data = match decoder.read_image().unwrap() {
        DecodingResult::U16(res) => res,
        _ => panic!("Wrong data type"),
    };

    let mut file = tempfile::tempfile().unwrap();
    {
        let mut tiff = TiffEncoder::new(&mut file).unwrap();

        let (width, height) = decoder.dimensions().unwrap();
        tiff.write_image::<colortype::RGB16>(width, height, &image_data)
            .unwrap();
    }
    file.seek(SeekFrom::Start(0)).unwrap();
    {
        let mut decoder = Decoder::new(&mut file).unwrap();
        if let DecodingResult::U16(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}
