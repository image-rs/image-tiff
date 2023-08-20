use std::{fs, env};
use std::io::{Cursor, BufWriter};
use fax::{decoder, decoder::pels, Color};
use image::{ImageError, DynamicImage, GrayImage};
use image::io::Reader as ImageReader;

fn resize_dyn_img_to_png_bytes(dyn_image: DynamicImage) -> Result<Vec<u8>, ImageError> {
    let mut image_data = BufWriter::new(Cursor::new(Vec::new()));
    dyn_image.write_to(&mut image_data, image::ImageOutputFormat::Png).unwrap();
    return Ok(image_data.get_ref().get_ref().to_owned());
}

fn image_preprocess_fax(bytes:Vec<u8>) -> Result<GrayImage, Box<dyn std::error::Error>> {
    use tiff::decoder::Decoder;
    use tiff::tags::Tag;
    let mut decoder = Decoder::new(Cursor::new(bytes.as_slice())).unwrap();
    let width = decoder.get_tag_u32(Tag::ImageWidth).unwrap();
    let height = decoder.get_tag_u32(Tag::ImageLength).unwrap();
    assert_eq!(decoder.get_tag_u32(Tag::Compression).unwrap(), 4);
    assert_eq!(decoder.get_tag_u32(Tag::BitsPerSample).unwrap(), 1);
    
    let strip_offsets = decoder.get_tag_u32_vec(Tag::StripOffsets).unwrap();
    let strip_lengths = decoder.get_tag_u32_vec(Tag::StripByteCounts).unwrap();

    let mut image = GrayImage::new(width, height);

    let mut rows = image.rows_mut();
    let mut cols_read = 0;
    for (&off, &len) in strip_offsets.iter().zip(strip_lengths.iter()) {
        decoder.goto_offset(off).unwrap();
        let bytes = std::iter::from_fn(|| decoder.read_byte().ok()).take(len as usize);

        decoder::decode_g4(bytes, width as u16, None,  |transitions| {
            let row = rows.next().unwrap();
            for (c, px) in pels(transitions, width as u16).zip(row) {
                let byte = match c {
                    Color::Black => 0,
                    Color::White => 255
                };
                px.0[0] = byte;
            }

            cols_read += 1;
        });
    }
    
    Ok(image)
}

pub fn read_bytes_to_png_bytes(bytes:Vec<u8>,) -> Result<Vec<u8>, ImageError>{
    if bytes.len() > 3 && bytes[0] == 73 && bytes[1] == 73 // big endian, fine for POC
        && bytes[2] == 42{
            let image = image_preprocess_fax(bytes).unwrap();
            return resize_dyn_img_to_png_bytes(image.into());
        }

    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;

    match reader.decode(){
        Ok(img) => {
          return resize_dyn_img_to_png_bytes(img);
        }
        Err(e) => {
            eprint!("Error decoding image: {}", e);
            return Err(e);
        }
    };
}

pub fn read_image_to_png_bytes(path: &String) -> Result<Vec<u8>, ImageError>{
    let bytes = fs::read(path).unwrap();
    return read_bytes_to_png_bytes(bytes);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1{
        let path = &args[1];
        let image_bytes = read_image_to_png_bytes(path).unwrap();
        std::fs::write("out.png", &image_bytes).unwrap();
        println!("Image converted to out.png, {} bytes in final PNG representation", image_bytes.len());
    }
    else{
        println!("Usage - this_executable <path/to/input-image.any>");
    }
}
