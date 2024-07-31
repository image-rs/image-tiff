extern crate tiff;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::ColorType;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

/// Test a basic all white image
#[test]
fn test_white_ieee_fp16() {
    let filenames = ["white-fp16.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            for p in img {
                assert!(p == half::f16::from_f32_const(1.0));
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test a single black pixel, to make sure scaling is ok
#[test]
fn test_one_black_pixel_ieee_fp16() {
    let filenames = ["single-black-fp16.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            for (i, p) in img.iter().enumerate() {
                if i == 0 {
                    assert!(p < &half::f16::from_f32_const(0.001));
                } else {
                    assert!(p == &half::f16::from_f32_const(1.0));
                }
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test white with horizontal differencing predictor
#[test]
fn test_horizontal_differencing_ieee_fp16() {
    let filenames = ["white-fp16-pred2.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            // 0, 2, 5, 8, 12, 16, 255 are black
            let black = [0, 2, 5, 8, 12, 16, 255];
            for (i, p) in img.iter().enumerate() {
                if black.contains(&i) {
                    assert!(p < &half::f16::from_f32_const(0.001));
                } else {
                    assert!(p == &half::f16::from_f32_const(1.0));
                }
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test white with floating point predictor
#[test]
fn test_predictor_ieee_fp16() {
    let filenames = ["white-fp16-pred3.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            // 0, 2, 5, 8, 12, 16, 255 are black
            let black = [0, 2, 5, 8, 12, 16, 255];
            for (i, p) in img.iter().enumerate() {
                if black.contains(&i) {
                    assert!(p < &half::f16::from_f32_const(0.001));
                } else {
                    assert!(p == &half::f16::from_f32_const(1.0));
                }
            }
        } else {
            panic!("Wrong data type");
        }
    }
}
