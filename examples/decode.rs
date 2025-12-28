use tiff::decoder::{BufferLayoutPreference, Decoder, DecodingBuffer, DecodingResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(image) = std::env::args_os().nth(1) else {
        eprintln!("Usage: decode FILE");
        return Ok(());
    };

    let file = std::fs::File::open(image)?;
    let io = std::io::BufReader::new(file);
    let mut reader = Decoder::new(io)?;

    let mut data = DecodingResult::I8(vec![]);

    for i in 0u32.. {
        let colortype = reader.colortype()?;
        let dimensions = reader.dimensions()?;
        let layout = reader.read_image_to_buffer(&mut data)?;

        debug_planes(i, &mut data, &layout, &colortype, dimensions);

        if !reader.more_images() {
            break;
        }

        reader.next_image()?
    }

    Ok(())
}

fn debug_planes(
    index: u32,
    data: &mut DecodingResult,
    layout: &BufferLayoutPreference,
    colortype: &tiff::ColorType,
    (width, height): (u32, u32),
) {
    let (depth, mut tupltype) = match colortype {
        tiff::ColorType::Gray(_) => (1, "GRAYSCALE"),
        tiff::ColorType::RGB(_) => (3, "RGB"),
        tiff::ColorType::RGBA(_) => (4, "RGB_ALPHA"),
        tiff::ColorType::Palette(_) => (1, "PALETTE"),
        tiff::ColorType::CMYK(_) => (4, "CMYK"),
        tiff::ColorType::Multiband { num_samples: 2, .. } => (2, "GRAYSCALE_ALPHA"),
        _ => {
            eprintln!("Unsupported color type for PAM output: {:?}", colortype);
            return;
        }
    };

    let maxval = match data.as_buffer(0) {
        DecodingBuffer::U8(_) => u64::from(u8::MAX),
        DecodingBuffer::U16(_) => u64::from(u16::MAX),
        DecodingBuffer::U32(_) => u64::from(u32::MAX),
        DecodingBuffer::U64(_) => u64::MAX,
        // Eh, this is a good effort I guess.
        DecodingBuffer::I8(items) => {
            items.iter_mut().for_each(|v| *v = 0.max(*v));
            u64::from(i8::MAX as u8)
        }
        DecodingBuffer::I16(items) => {
            items.iter_mut().for_each(|v| *v = 0.max(*v));
            u64::from(i16::MAX as u16)
        }
        DecodingBuffer::I32(items) => {
            items.iter_mut().for_each(|v| *v = 0.max(*v));
            u64::from(i32::MAX as u32)
        }
        DecodingBuffer::I64(items) => {
            items.iter_mut().for_each(|v| *v = 0.max(*v));
            i64::MAX as u64
        }
        // Not supported by PAM.
        DecodingBuffer::F16(_) | DecodingBuffer::F32(_) | DecodingBuffer::F64(_) => return,
    };

    let byte_len = if layout.planes > 1 {
        tupltype = "GRAYSCALE";
        layout.plane_stride.unwrap().get()
    } else {
        data.as_buffer(0).byte_len()
    };

    // Write out this image's planes to PAM files.
    let header = format!("P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH {depth}\nMAXVAL {maxval}\nTUPLTYPE {tupltype}\nENDHDR\n");

    for (plane, plane_data) in data
        .as_buffer(0)
        .as_bytes_mut()
        .chunks_exact(byte_len)
        .enumerate()
    {
        let filename = format!("image_{index:03}_plane_{plane:03}.pam");
        let mut file = std::fs::File::create(filename).expect("Failed to create output file");

        use std::io::Write;
        file.write_all(header.as_bytes())
            .expect("Failed to write PAM header");
        file.write_all(plane_data)
            .expect("Failed to write PAM data");
    }
}
