extern crate tiff;

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, ifd::Tag, ifd::Value};

use std::fs::File;

#[test]
fn test_gray_u8()
{
    let img_file = File::open("./tests/images/minisblack-1c-8b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(8));
    let img_res = decoder.read_image();
    assert!(img_res.is_ok());
}

#[test]
fn test_rgb_u8()
{
    let img_file = File::open("./tests/images/rgb-3c-8b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
    let img_res = decoder.read_image();
    assert!(img_res.is_ok());
}

#[test]
fn test_gray_u16()
{
    let img_file = File::open("./tests/images/minisblack-1c-16b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(16));
    let img_res = decoder.read_image();
    assert!(img_res.is_ok());
}

#[test]
fn test_rgb_u16()
{
    let img_file = File::open("./tests/images/rgb-3c-16b.tiff").expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(16));
    let img_res = decoder.read_image();
    assert!(img_res.is_ok());
}

#[test]
fn test_string_tags()
{
    // these files have null-terminated strings for their Software tag. One has extra bytes after
    // the null byte, so we check both to ensure that we're truncating properly
    let filenames = vec!["minisblack-1c-16b.tiff", "rgb-3c-16b.tiff"];
    for filename in filenames.iter() {
        let path = format!("./tests/images/{}", filename);
        let img_file = File::open(path).expect("can't open file");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        let software= decoder.get_tag(Tag::Software).unwrap();
        match software {
            Value::Ascii(s) => assert_eq!(s, String::from("GraphicsMagick 1.2 unreleased Q16 http://www.GraphicsMagick.org/")),
            _ => assert!(false)
        };
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
