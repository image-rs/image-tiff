extern crate tiff;

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, ifd::Tag, ifd::Value};
use tiff::encoder::{TiffEncoder, Rational};

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
        let mut tiff = TiffEncoder::new_le(file);

        tiff.write_header().unwrap();

        tiff.new_ifd().unwrap();
        tiff.new_ifd_entry(Tag::ImageWidth, &[100u32][..]);
        tiff.new_ifd_entry(Tag::ImageLength, &[100u32][..]);
        tiff.new_ifd_entry(Tag::BitsPerSample, &[8u16,8,8][..]);
        tiff.new_ifd_entry(Tag::Compression, &[1u16][..]);
        tiff.new_ifd_entry(Tag::PhotometricInterpretation, &[2u16][..]);
        tiff.new_ifd_entry(Tag::StripOffsets, &[0u32][..]);
        tiff.new_ifd_entry(Tag::SamplesPerPixel, &[3u16][..]);
        tiff.new_ifd_entry(Tag::RowsPerStrip, &[100u32][..]);
        tiff.new_ifd_entry(Tag::StripByteCounts, &[0u32][..]);
        tiff.new_ifd_entry(Tag::XResolution, &[Rational {n: 1, d: 1}][..]);
        tiff.new_ifd_entry(Tag::YResolution, &[Rational {n: 1, d: 1}][..]);
        tiff.new_ifd_entry(Tag::ResolutionUnit, &[1u16][..]);
        tiff.finish_ifd().unwrap();

        tiff.write_strip(&image_data).unwrap();
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
