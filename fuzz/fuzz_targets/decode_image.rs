#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decoder = if let Ok(d) = tiff::decoder::Decoder::new(std::io::Cursor::new(data)) {
        d
    } else {
        return;
    };

    let mut limits = tiff::decoder::Limits::default();
    limits.decoding_buffer_size = 1_000_000;
    limits.ifd_value_size = 1_000_000;
    limits.intermediate_buffer_size = 1_000_000;

    decoder = decoder.with_limits(limits);

    let _ = decoder.read_image();
});
