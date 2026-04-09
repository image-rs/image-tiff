#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::{Cursor, Read, Seek};
use tiff::decoder::{Decoder, Limits};
use tiff::tags::Tag;

fuzz_target!(|data: &[u8]| {
    // Standalone header parse (separate code path from Decoder::open)
    let _ = tiff::decoder::TiffHeader::parse(Cursor::new(data));

    let mut decoder = match Decoder::open(Cursor::new(data)) {
        Ok(d) => d,
        Err(_) => return,
    };

    let mut limits = Limits::default();
    limits.decoding_buffer_size = 512 * 1024;
    limits.ifd_value_size = 64 * 1024;
    limits.intermediate_buffer_size = 512 * 1024;
    decoder = decoder.with_limits(limits);

    // Probe first IFD (opened but image not yet parsed)
    probe_directory(&mut decoder);

    // Linear IFD iteration via next_directory — tolerates invalid
    // intermediate IFDs that aren't decodable as images
    let mut ifd_count = 1u32;
    while decoder.more_images() && ifd_count < 8 {
        if decoder.next_directory().is_err() {
            break;
        }
        probe_directory(&mut decoder);
        ifd_count += 1;
    }

    // Random-access traversal via seek (different code path: find_nth_ifd
    // re-walks the IFD chain from the start each time)
    for i in 0..ifd_count.min(4) as usize {
        if decoder.seek_to_directory(i).is_ok() {
            probe_directory(&mut decoder);
        }
        // seek_to_image eagerly parses the directory as an Image
        let _ = decoder.seek_to_image(i);
    }
});

fn probe_directory<R: Read + Seek>(decoder: &mut Decoder<R>) {
    // Tag iteration — exercises value parsing for every tag in the IFD
    for result in decoder.current_ifd().tag_iter() {
        let _ = result;
    }

    // SubIFD / EXIF / GPS directory traversal — exercises read_directory
    // with potentially adversarial offsets
    if let Ok(Some(val)) = decoder.current_ifd().find_tag(Tag::SubIfd) {
        if let Ok(ptrs) = val.into_ifd_vec() {
            for ptr in ptrs.into_iter().take(4) {
                let _ = decoder.read_directory(ptr);
            }
        }
    }
    for tag in [Tag::ExifDirectory, Tag::GpsDirectory] {
        if let Ok(Some(val)) = decoder.current_ifd().find_tag(tag) {
            if let Ok(ptr) = val.into_ifd_pointer() {
                let _ = decoder.read_directory(ptr);
            }
        }
    }

    // Image-level probing — dimensions() triggers ensure_image,
    // which lazily parses the directory as an Image
    if decoder.dimensions().is_ok() {
        let _ = decoder.colortype();
        let _ = decoder.color_map();
        let _ = decoder.get_chunk_type();
        let _ = decoder.chunk_dimensions();
        let _ = decoder.strip_count();
        let _ = decoder.tile_count();
        let _ = decoder.image_buffer_layout();
        let _ = decoder.image_chunk_buffer_layout(0);

        // Chunk reads at boundaries — first and last exercise
        // different offset calculations
        let _ = decoder.read_chunk(0);
        let chunk_count = decoder
            .strip_count()
            .or_else(|_| decoder.tile_count())
            .unwrap_or(0);
        if chunk_count > 1 {
            let _ = decoder.read_chunk(chunk_count - 1);
        }

        // Full image decode
        let _ = decoder.read_image();
    }
}
