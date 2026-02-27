extern crate criterion;
extern crate tiff;

use criterion::{black_box, Criterion, Throughput};
use std::io::Cursor;
use tiff::encoder::colortype::RGB32Float;
use tiff::encoder::{Compression, DeflateLevel, Predictor, TiffEncoder};

const WIDTH: u32 = 500;
const HEIGHT: u32 = 500;

fn encode_image_f32_float_predictor_deflate(data: &[f32]) {
    let mut buf = Cursor::new(Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4));
    TiffEncoder::new(&mut buf)
        .unwrap()
        .with_predictor(Predictor::FloatingPoint)
        .with_compression(Compression::Deflate(DeflateLevel::Fast))
        .write_image::<RGB32Float>(WIDTH, HEIGHT, black_box(data))
        .unwrap();
}

fn main() {
    // Generate synthetic f32 image data with a gradient pattern
    let data: Vec<f32> = (0..WIDTH as usize * HEIGHT as usize * 3)
        .map(|i| {
            let pixel = i / 3;
            let x = (pixel % WIDTH as usize) as f32 / WIDTH as f32;
            let y = (pixel / WIDTH as usize) as f32 / HEIGHT as f32;
            x * 0.5 + y * 0.5
        })
        .collect();

    let mut c = Criterion::default().configure_from_args();
    let mut group = c.benchmark_group("tiff-encode");

    group
        .sample_size(20)
        .throughput(Throughput::Bytes(
            (WIDTH as u64) * (HEIGHT as u64) * 3 * std::mem::size_of::<f32>() as u64,
        ))
        .bench_function("f32-float-predictor-deflate-fast", |b| {
            b.iter(|| encode_image_f32_float_predictor_deflate(&data))
        });
}
