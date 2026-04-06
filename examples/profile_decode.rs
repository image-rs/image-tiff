//! Profiling helper: encodes and decodes several TIFF configurations.
//! Run under callgrind: `valgrind --tool=callgrind target/release/examples/profile_decode`
use std::io::Cursor;
use tiff::decoder::Decoder;
use tiff::encoder::colortype::{Gray8, RGB32Float, RGB16, RGB8};
use tiff::encoder::{Compression, Predictor, TiffEncoder};

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 2048;
const ITERS: usize = 3;

fn generate_rgb8(w: u32, h: u32) -> Vec<u8> {
    (0..w * h * 3).map(|i| (i % 256) as u8).collect()
}

fn generate_rgb16(w: u32, h: u32) -> Vec<u16> {
    (0..w * h * 3).map(|i| (i % 65536) as u16).collect()
}

fn generate_rgb_f32(w: u32, h: u32) -> Vec<f32> {
    (0..w * h * 3)
        .map(|i| (i as f32) / (w * h * 3) as f32)
        .collect()
}

fn generate_gray8(w: u32, h: u32) -> Vec<u8> {
    (0..w * h).map(|i| (i % 256) as u8).collect()
}

fn encode<C: tiff::encoder::colortype::ColorType>(
    data: &[C::Inner],
    w: u32,
    h: u32,
    compression: Compression,
    predictor: Predictor,
) -> Vec<u8>
where
    [C::Inner]: tiff::encoder::TiffValue,
{
    let mut buf = Cursor::new(Vec::new());
    TiffEncoder::new(&mut buf)
        .unwrap()
        .with_predictor(predictor)
        .with_compression(compression)
        .write_image::<C>(w, h, data)
        .unwrap();
    buf.into_inner()
}

fn decode(data: &[u8]) {
    let cursor = Cursor::new(data);
    let mut decoder = Decoder::open(cursor).unwrap();
    decoder.next_image().unwrap();
    let _ = decoder.read_image().unwrap();
}

fn main() {
    let rgb8 = generate_rgb8(WIDTH, HEIGHT);
    let rgb16 = generate_rgb16(WIDTH, HEIGHT);
    let rgb_f32 = generate_rgb_f32(WIDTH, HEIGHT);
    let gray8 = generate_gray8(WIDTH, HEIGHT);

    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "deflate-rgb8-hpredict",
            encode::<RGB8>(
                &rgb8,
                WIDTH,
                HEIGHT,
                Compression::Deflate(1),
                Predictor::Horizontal,
            ),
        ),
        (
            "deflate-gray8-hpredict",
            encode::<Gray8>(
                &gray8,
                WIDTH,
                HEIGHT,
                Compression::Deflate(1),
                Predictor::Horizontal,
            ),
        ),
        (
            "deflate-rgb16-hpredict",
            encode::<RGB16>(
                &rgb16,
                WIDTH,
                HEIGHT,
                Compression::Deflate(1),
                Predictor::Horizontal,
            ),
        ),
        (
            "deflate-rgb-f32-fpredict",
            encode::<RGB32Float>(
                &rgb_f32,
                WIDTH,
                HEIGHT,
                Compression::Deflate(1),
                Predictor::FloatingPoint,
            ),
        ),
        (
            "lzw-rgb8-no-pred",
            encode::<RGB8>(&rgb8, WIDTH, HEIGHT, Compression::Lzw, Predictor::None),
        ),
        (
            "lzw-rgb8-hpredict",
            encode::<RGB8>(
                &rgb8,
                WIDTH,
                HEIGHT,
                Compression::Lzw,
                Predictor::Horizontal,
            ),
        ),
    ];

    for (name, tiff_data) in &cases {
        eprintln!(
            "=== Profiling: {} ({} bytes encoded) ===",
            name,
            tiff_data.len()
        );
        for _ in 0..ITERS {
            decode(tiff_data);
        }
    }
}
