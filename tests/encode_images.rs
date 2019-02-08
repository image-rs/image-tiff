extern crate tiff;

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, ifd::Tag, ifd::Value};
use tiff::encoder::{TiffEncoder, RGB8};

use std::fs::File;


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
    {
        let file = std::fs::File::create("test.tiff").unwrap();
        let mut tiff = TiffEncoder::new(file).unwrap();

        let mut image = tiff.new_image::<RGB8>(100, 100).unwrap();
    
        let mut idx = 0;
        while image.next_strip_sample_count() > 0 {
            let sample_count = image.next_strip_sample_count() as usize;
            image.write_strip(&image_data[idx..idx+sample_count]).unwrap();
            idx += sample_count;
        }
        image.finish().unwrap();
    }
    {
        let file = File::open("test.tiff").unwrap();
        let mut decoder = Decoder::new(file).unwrap();
        assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
        assert_eq!(decoder.dimensions().unwrap(), (100, 100));
        if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        }
        else {
            panic!("Wrong data type");
        }
    }
    
    std::fs::remove_file("test.tiff").unwrap();
}
