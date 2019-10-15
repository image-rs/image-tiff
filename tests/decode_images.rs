extern crate tiff;

use tiff::decoder::{ifd, Decoder, DecodingResult};
use tiff::ColorType;

use std::fs::File;

#[test]
fn test_gray_u8() {
    let img_file =
        File::open("./tests/images/minisblack-1c-8b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(8));
    let img_res = decoder.read_image().unwrap();
    if let DecodingResult::U8(res) = img_res {
        let mut res_sum: u64 = 0;
        for x in res {
            res_sum += x as u64;
        }

        assert_eq!(res_sum, 2840893);
    }
    else {
        panic!("Wrong bit depth")
    }
}

#[test]
fn test_rgb_u8() {
    let img_file = File::open("./tests/images/rgb-3c-8b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
    let img_res = decoder.read_image().unwrap();
    if let DecodingResult::U8(res) = img_res {
        let mut res_sum: u64 = 0;
        for x in res {
            res_sum += x as u64;
        }

        assert_eq!(res_sum, 7842108)
    }
    else {
        panic!("Wrong bit depth")
    }
}

#[test]
fn test_gray_u16() {
    let img_file =
        File::open("./tests/images/minisblack-1c-16b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(16));
    let img_res = decoder.read_image().unwrap();
    if let DecodingResult::U16(res) = img_res {
        let mut res_sum: u64 = 0;
        for x in res {
            res_sum += x as u64;
        }

        assert_eq!(res_sum, 733126239);
    }
    else {
        panic!("Wrong bit depth")
    }
}

#[test]
fn test_rgb_u16() {
    let img_file = File::open("./tests/images/rgb-3c-16b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(16));
    let img_res = decoder.read_image().unwrap();
    if let DecodingResult::U16(res) = img_res {
        let mut res_sum: u64 = 0;
        for x in res {
            res_sum += x as u64;
        }

        assert_eq!(res_sum, 2024349944);
    }
    else {
        panic!("Wrong bit depth")
    }
}

#[test]
fn test_string_tags() {
    // these files have null-terminated strings for their Software tag. One has extra bytes after
    // the null byte, so we check both to ensure that we're truncating properly
    let filenames = vec!["minisblack-1c-16b.tiff", "rgb-3c-16b.tiff"];
    for filename in filenames.iter() {
        let path = format!("./tests/images/{}", filename);
        let img_file = File::open(path).expect("can't open file");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        let software = decoder.get_tag(ifd::Tag::Software).unwrap();
        match software {
            ifd::Value::Ascii(s) => assert_eq!(
                s,
                String::from("GraphicsMagick 1.2 unreleased Q16 http://www.GraphicsMagick.org/")
            ),
            _ => assert!(false),
        };
    }
}

#[test]
fn test_decode_data() {
    let mut image_data = Vec::new();
    for x in 0..100 {
        for y in 0..100u8 {
            let val = x + y;
            image_data.push(val);
            image_data.push(val);
            image_data.push(val);
        }
    }
    let file = File::open("./tests/decodedata-rgb-3c-8b.tiff").unwrap();
    let mut decoder = Decoder::new(file).unwrap();
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
    assert_eq!(decoder.dimensions().unwrap(), (100, 100));
    if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
        assert_eq!(image_data, img_res);
    } else {
        panic!("Wrong data type");
    }
}

// TODO: GrayA support
//#[test]
//fn test_gray_alpha_u8()
//{
//let img_file = File::open("./tests/images/minisblack-2c-8b-alpha.tiff").expect("Cannot find test image!");
//let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
//assert_eq!(decoder.colortype().unwrap(), ColorType::GrayA(8));
//let img_res = decoder.read_image();
//assert!(img_res.is_ok());
//}
