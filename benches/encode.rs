extern crate criterion;
extern crate tiff;

use criterion::{Criterion, Throughput};
use std::hint::black_box;
use std::io::Cursor;
use tiff::encoder::colortype::RGB32Float;
use tiff::encoder::{Compression, Predictor, TiffEncoder};

const WIDTH: u32 = 500;
const HEIGHT: u32 = 500;

fn encode_image_f32(data: &[f32], width: u32, height: u32, compression: Compression) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::with_capacity(width as usize * height as usize * 4));
    TiffEncoder::new(&mut buf)
        .unwrap()
        .with_predictor(Predictor::FloatingPoint)
        .with_compression(compression)
        .write_image::<RGB32Float>(width, height, black_box(data))
        .unwrap();
    buf.into_inner()
}

fn bench_image(c: &mut Criterion, group_name: &str, data: &[f32], width: u32, height: u32) {
    let mut group = c.benchmark_group(group_name);
    group
        .sample_size(20)
        .throughput(Throughput::Bytes(std::mem::size_of_val(data) as u64));
    let algorithms: &[(&str, Compression)] = &[
        #[cfg(feature = "deflate")]
        ("f32-float-predictor-deflate-fast", Compression::Deflate(1)),
        #[cfg(feature = "zstd")]
        ("f32-float-predictor-zstd-1", Compression::Zstd(1)),
    ];
    for &(name, compression) in algorithms {
        let size = encode_image_f32(data, width, height, compression).len();
        println!("{group_name}/{name}: {size} bytes");
        group.bench_function(name, |b| {
            b.iter(|| encode_image_f32(data, width, height, compression))
        });
    }
    group.finish();
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
    bench_image(&mut c, "tiff-encode", &data, WIDTH, HEIGHT);

    if let Some(dir) = std::env::var_os("TIFF_BENCH_DIR") {
        let path = std::path::PathBuf::from(dir).join("photo-rgb-f32-deflate-fpred.tif");
        let mut limits = tiff::decoder::Limits::default();
        limits.decoding_buffer_size = 1024 * 1024 * 1024;
        let mut decoder = tiff::decoder::Decoder::open(std::fs::File::open(path).unwrap())
            .unwrap()
            .with_limits(limits);
        decoder.next_image().unwrap();
        let (width, height) = decoder.dimensions().unwrap();
        assert_eq!(decoder.colortype().unwrap(), tiff::ColorType::RGB(32));
        assert_eq!(
            decoder
                .current_ifd()
                .get_tag_u32(tiff::tags::Tag::PlanarConfiguration)
                .unwrap_or(1),
            1,
            "The photo corpus must contain contiguous RGB samples"
        );
        let tiff::decoder::DecodingSampleBuffer::F32(data) = decoder.read_image().unwrap() else {
            panic!("The photo corpus must contain RGB32Float samples");
        };
        bench_image(&mut c, "photo-tiff-encode", &data, width, height);
    }
}
