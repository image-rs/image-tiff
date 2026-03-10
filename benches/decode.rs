extern crate criterion;
extern crate tiff;

use criterion::{black_box, Criterion, Throughput};
use tiff::decoder::Decoder;

fn decode_image(data: &[u8]) {
    let image = std::io::Cursor::new(data);
    let mut reader = Decoder::open(black_box(image)).unwrap();
    reader.next_image().unwrap();
    reader.read_image().unwrap();
}

fn main() {
    let mut c = Criterion::default().configure_from_args();

    // Check if large bench images exist
    let bench_dir = std::path::Path::new("/tmp/bench-tiff");
    if !bench_dir.is_dir() {
        eprintln!("Skipping large image benchmarks: /tmp/bench-tiff/ not found");
        eprintln!("Generate with: cc -O2 -o /tmp/gen_bench_tiff /tmp/gen_bench_tiff.c -ltiff -lm && /tmp/gen_bench_tiff");
        return;
    }

    let mut group = c.benchmark_group("decode-1024x1024");
    group.sample_size(30);

    for depth in [3, 5, 7, 8, 10, 12, 14, 16, 24] {
        let path = bench_dir.join(format!("bench-1c-{depth}b.tiff"));
        if !path.exists() {
            continue;
        }
        let data = std::fs::read(&path).unwrap();
        group
            .throughput(Throughput::Bytes(data.len() as u64))
            .bench_function(format!("{depth}b"), |b| b.iter(|| decode_image(&data)));
    }

    group.finish();
}
