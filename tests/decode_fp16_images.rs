extern crate tiff;

use tiff::decoder::{ifd, Decoder, DecodingResult};
use tiff::tags::Tag;
use tiff::ColorType;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

#[test]
fn test_ieee_fp16() {
    let filenames = ["tile_1845_3473_13.tif"];
    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (513, 513)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img_res) = decoder.read_image().unwrap() {
            eprintln!("Num Pixels: {}", img_res.len());
            eprintln!("Pixel 0: {}", img_res[0]);
            //assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}
