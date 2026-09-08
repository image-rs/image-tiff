use criterion::{black_box, Criterion, Throughput};
use std::{io::Cursor, time::Duration};
use tiff::{
    decoder::{Decoder, DecodingSampleBuffer},
    encoder::{colortype::Gray1, Compression, TiffEncoder},
};

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 2048;

fn encode(data: &[u8]) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    TiffEncoder::new(&mut output)
        .unwrap()
        .with_compression(Compression::Huffman)
        .write_image::<Gray1>(WIDTH, HEIGHT, data)
        .unwrap();
    output.into_inner()
}

fn decode(data: &[u8]) -> Vec<u8> {
    let mut decoder = Decoder::open(Cursor::new(data)).unwrap();
    decoder.next_image().unwrap();
    let mut output = DecodingSampleBuffer::U8(Vec::new());
    decoder.read_image_to_buffer(&mut output).unwrap();
    match output {
        DecodingSampleBuffer::U8(bytes) => bytes,
        _ => unreachable!(),
    }
}

fn main() {
    let mut c = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .configure_from_args();
    let size = (WIDTH as usize / 8) * HEIGHT as usize;
    let patterns = [
        ("zero", vec![0; size]),
        ("one", vec![0xFF; size]),
        (
            "runs-512px",
            (0..size)
                .map(|i| if i / 64 % 2 == 0 { 0 } else { 0xFF })
                .collect(),
        ),
        ("alternating", vec![0xAA; size]),
    ];
    let mut group = c.benchmark_group("huffman-2048x2048");
    group.throughput(Throughput::Elements(u64::from(WIDTH) * u64::from(HEIGHT)));
    for (name, pixels) in patterns {
        let encoded = encode(&pixels);
        assert_eq!(decode(&encoded), pixels);
        group.bench_function(format!("decode/{name}"), |b| {
            b.iter(|| black_box(decode(black_box(&encoded))))
        });
        group.bench_function(format!("encode/{name}"), |b| {
            b.iter(|| black_box(encode(black_box(&pixels))))
        });
    }
    group.finish();
    let fixture = include_bytes!("../tests/images/ccitt1d.tiff");
    c.bench_function("huffman-fixture/decode", |b| {
        b.iter(|| black_box(decode(black_box(fixture))))
    });
    c.final_summary();
}
