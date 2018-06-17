extern crate tiff;

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult};

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
