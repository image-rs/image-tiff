#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let cursor = std::io::Cursor::new(data);

    // Path 1: TiffHeader::parse (probe)
    {
        let _ = tiff::decoder::TiffHeader::parse(std::io::Cursor::new(data));
    }

    // Path 2: Full decode with strict limits
    let mut decoder = match tiff::decoder::Decoder::open(cursor) {
        Ok(d) => d,
        Err(_) => return,
    };

    let mut limits = tiff::decoder::Limits::default();
    limits.decoding_buffer_size = 512 * 1024;
    limits.ifd_value_size = 64 * 1024;
    limits.intermediate_buffer_size = 512 * 1024;

    decoder = decoder.with_limits(limits);

    // Probe metadata — dimensions() triggers image validation,
    // must succeed before get_chunk_type/chunk_dimensions (they unwrap image)
    let dims_ok = decoder.dimensions().is_ok();
    let _ = decoder.colortype();
    if dims_ok {
        let _ = decoder.get_chunk_type();
        let _ = decoder.chunk_dimensions();
    }
    let _ = decoder.more_images();

    // Read chunks individually if available
    let _ = decoder.read_chunk(0);

    // Read full image
    let _ = decoder.read_image();

    // Iterate through all IFDs
    while decoder.more_images() {
        if decoder.next_image().is_err() {
            break;
        }
        let _ = decoder.dimensions();
        let _ = decoder.colortype();
        let _ = decoder.read_image();
    }
});
