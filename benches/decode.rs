extern crate criterion;
extern crate tiff;

use criterion::{black_box, Criterion, Throughput};
use std::io::Cursor;
use tiff::decoder::Decoder;
use tiff::encoder::colortype::{Gray16, Gray8, RGB32Float, RGB16, RGB8, RGBA8};
use tiff::encoder::{Compression, Predictor, TiffEncoder};

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 2048;

// === Corpus paths (real photo-based TIFFs) ===
// Set TIFF_BENCH_DIR / TIFF_GEOTIFF_DIR env vars to a directory containing the corpus files.
// If the files are not found, the corresponding benchmarks are silently skipped.
// The corpus can be generated with vips/imagemagick from any large photo; see the benchmark
// names for the expected filenames and compression settings.
const CORPUS_DIR: &str = "tests/benches/corpus";

fn bench_dir() -> String {
    std::env::var("TIFF_BENCH_DIR").unwrap_or_else(|_| CORPUS_DIR.to_string())
}

fn geotiff_dir() -> String {
    std::env::var("TIFF_GEOTIFF_DIR").unwrap_or_else(|_| CORPUS_DIR.to_string())
}

/// Load a file from the corpus directory, returning None if not found.
fn load_corpus(dir: &str, name: &str) -> Option<Vec<u8>> {
    let path = format!("{}/{}", dir, name);
    std::fs::read(&path).ok()
}

fn decode_tiff_bytes(data: &[u8]) {
    let cursor = Cursor::new(data);
    let mut decoder = Decoder::open(black_box(cursor)).unwrap();
    decoder.next_image().unwrap();
    decoder.read_image().unwrap();
}

// === Synthetic image generators ===

fn generate_rgb8(w: u32, h: u32) -> Vec<u8> {
    (0..w * h * 3)
        .map(|i| ((i * 7 + i / w * 3) & 0xFF) as u8)
        .collect()
}

fn generate_rgba8(w: u32, h: u32) -> Vec<u8> {
    (0..w * h * 4)
        .map(|i| ((i * 11 + i / w * 5) & 0xFF) as u8)
        .collect()
}

fn generate_gray8(w: u32, h: u32) -> Vec<u8> {
    (0..w * h)
        .map(|i| ((i * 7 + i / w * 3) & 0xFF) as u8)
        .collect()
}

fn generate_rgb16(w: u32, h: u32) -> Vec<u16> {
    (0..w * h * 3)
        .map(|i| ((i * 257 + i / w * 131) & 0xFFFF) as u16)
        .collect()
}

fn generate_gray16(w: u32, h: u32) -> Vec<u16> {
    (0..w * h)
        .map(|i| ((i * 257 + i / w * 131) & 0xFFFF) as u16)
        .collect()
}

fn generate_rgb_f32(w: u32, h: u32) -> Vec<f32> {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            data.push(fx * 0.5 + fy * 0.3);
            data.push(fy * 0.7 + fx * 0.2);
            data.push((fx + fy) * 0.25);
        }
    }
    data
}

// === Synthetic encoders (strip-only, encoder doesn't support tiles) ===

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

fn main() {
    let mut c = Criterion::default().configure_from_args();
    let bd = bench_dir();
    let gd = geotiff_dir();

    // ========================================================================
    // PART 1: Real photo corpus (5616x3744, ~21MP)
    // ========================================================================
    // Raw pixel sizes for throughput calculation:
    //   RGB8:  5616*3744*3 = 63,078,912
    //   RGB16: 5616*3744*6 = 126,157,824
    //   F32x3: 5616*3744*12 = 252,315,648
    //   Gray8: 5616*3744*1 = 21,026,304
    //   GrayF32: 5616*3744*4 = 84,105,216
    //   Palette8: 5616*3744*1 = 21,026,304 (output)

    struct CorpusBench {
        file: &'static str,
        name: &'static str,
        raw_bytes: u64,
    }

    let photo_benches = [
        // Strip-based
        CorpusBench {
            file: "photo-rgb8-none.tif",
            name: "strip-rgb8-none",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb8-packbits.tif",
            name: "strip-rgb8-packbits",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb8-lzw.tif",
            name: "strip-rgb8-lzw",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb8-lzw-hpred.tif",
            name: "strip-rgb8-lzw-hpred",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb8-deflate-hpred.tif",
            name: "strip-rgb8-deflate-hpred",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb16-deflate-hpred.tif",
            name: "strip-rgb16-deflate-hpred",
            raw_bytes: 126_157_824,
        },
        CorpusBench {
            file: "photo-gray8-lzw.tif",
            name: "strip-gray8-lzw",
            raw_bytes: 21_026_304,
        },
        CorpusBench {
            file: "photo-gray8-deflate-hpred.tif",
            name: "strip-gray8-deflate-hpred",
            raw_bytes: 21_026_304,
        },
        CorpusBench {
            file: "photo-rgb-f32-deflate-fpred.tif",
            name: "strip-rgb-f32-deflate-fpred",
            raw_bytes: 252_315_648,
        },
        CorpusBench {
            file: "photo-gray-f32-deflate-fpred.tif",
            name: "strip-gray-f32-deflate-fpred",
            raw_bytes: 84_105_216,
        },
        CorpusBench {
            file: "photo-palette8-lzw.tif",
            name: "strip-palette8-lzw",
            raw_bytes: 21_026_304,
        },
        CorpusBench {
            file: "scan-bilevel-fax4.tif",
            name: "strip-bilevel-fax4",
            raw_bytes: 5616 * 3744 / 8,
        },
        // Tiled
        CorpusBench {
            file: "photo-rgb8-lzw-hpred-tiled256.tif",
            name: "tiled256-rgb8-lzw-hpred",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb8-deflate-hpred-tiled256.tif",
            name: "tiled256-rgb8-deflate-hpred",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-rgb8-jpeg-tiled256.tif",
            name: "tiled256-rgb8-jpeg",
            raw_bytes: 63_078_912,
        },
        CorpusBench {
            file: "photo-gray-f32-deflate-fpred-tiled512.tif",
            name: "tiled512-gray-f32-deflate-fpred",
            raw_bytes: 84_105_216,
        },
    ];

    {
        let mut group = c.benchmark_group("photo-21mp");
        group.sample_size(10);

        for bench in &photo_benches {
            if let Some(data) = load_corpus(&bd, bench.file) {
                group.throughput(Throughput::Bytes(bench.raw_bytes));
                let data_ref = &data;
                group.bench_function(bench.name, |b| b.iter(|| decode_tiff_bytes(data_ref)));
            }
        }

        group.finish();
    }

    // ========================================================================
    // PART 2: GeoTIFF (real tiled DEM data)
    // ========================================================================

    let geotiff_benches = [
        // Copernicus DEM: 3600x3600, tiled 1024x1024, Gray f32, deflate + fpredict
        CorpusBench {
            file: "Copernicus_DSM_COG_10_N40_00_W075_00_DEM.tif",
            name: "copernicus-dem-f32-deflate-fpred-1024tile",
            raw_bytes: 3600 * 3600 * 4,
        },
        // USGS DEM: 3612x3612, tiled 512x512, Gray f32, LZW + fpredict
        CorpusBench {
            file: "USGS_1_n40w075.tif",
            name: "usgs-dem-f32-lzw-fpred-512tile",
            raw_bytes: 3612 * 3612 * 4,
        },
    ];

    {
        let mut group = c.benchmark_group("geotiff");
        group.sample_size(10);

        for bench in &geotiff_benches {
            if let Some(data) = load_corpus(&gd, bench.file) {
                group.throughput(Throughput::Bytes(bench.raw_bytes));
                let data_ref = &data;
                group.bench_function(bench.name, |b| b.iter(|| decode_tiff_bytes(data_ref)));
            }
        }

        group.finish();
    }

    // ========================================================================
    // PART 3: Synthetic (2048x2048, controlled comparisons)
    // ========================================================================

    let rgb8 = generate_rgb8(WIDTH, HEIGHT);
    let rgba8 = generate_rgba8(WIDTH, HEIGHT);
    let gray8 = generate_gray8(WIDTH, HEIGHT);
    let rgb16 = generate_rgb16(WIDTH, HEIGHT);
    let gray16 = generate_gray16(WIDTH, HEIGHT);
    let rgb_f32 = generate_rgb_f32(WIDTH, HEIGHT);

    let raw_rgb8 = (WIDTH * HEIGHT * 3) as u64;
    let raw_rgba8 = (WIDTH * HEIGHT * 4) as u64;
    let raw_gray8 = (WIDTH * HEIGHT) as u64;
    let raw_rgb16 = (WIDTH * HEIGHT * 6) as u64;
    let raw_gray16 = (WIDTH * HEIGHT * 2) as u64;
    let raw_f32x3 = (WIDTH * HEIGHT * 12) as u64;

    // --- Uncompressed baseline ---
    {
        let mut group = c.benchmark_group("synth-uncompressed");
        group.sample_size(10);

        let t = encode::<RGB8>(
            &rgb8,
            WIDTH,
            HEIGHT,
            Compression::Uncompressed,
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGBA8>(
            &rgba8,
            WIDTH,
            HEIGHT,
            Compression::Uncompressed,
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_rgba8))
            .bench_function("rgba8", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<Gray8>(
            &gray8,
            WIDTH,
            HEIGHT,
            Compression::Uncompressed,
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_gray8))
            .bench_function("gray8", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB16>(
            &rgb16,
            WIDTH,
            HEIGHT,
            Compression::Uncompressed,
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb16))
            .bench_function("rgb16", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<Gray16>(
            &gray16,
            WIDTH,
            HEIGHT,
            Compression::Uncompressed,
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_gray16))
            .bench_function("gray16", |b| b.iter(|| decode_tiff_bytes(&t)));

        group.finish();
    }

    // --- Predictor overhead isolation (deflate-1, with vs without predictor) ---
    {
        let mut group = c.benchmark_group("synth-predictor");
        group.sample_size(10);

        let t = encode::<RGB8>(
            &rgb8,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8-deflate-none", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB8>(
            &rgb8,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::Horizontal,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8-deflate-hpred", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB16>(
            &rgb16,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb16))
            .bench_function("rgb16-deflate-none", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB16>(
            &rgb16,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::Horizontal,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb16))
            .bench_function("rgb16-deflate-hpred", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB32Float>(
            &rgb_f32,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::None,
        );
        group
            .throughput(Throughput::Bytes(raw_f32x3))
            .bench_function("rgb-f32-deflate-none", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB32Float>(
            &rgb_f32,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::FloatingPoint,
        );
        group
            .throughput(Throughput::Bytes(raw_f32x3))
            .bench_function("rgb-f32-deflate-fpred", |b| {
                b.iter(|| decode_tiff_bytes(&t))
            });

        group.finish();
    }

    // --- LZW vs Deflate comparison ---
    {
        let mut group = c.benchmark_group("synth-compression");
        group.sample_size(10);

        let t = encode::<RGB8>(
            &rgb8,
            WIDTH,
            HEIGHT,
            Compression::Deflate(1),
            Predictor::Horizontal,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8-deflate-hpred", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB8>(
            &rgb8,
            WIDTH,
            HEIGHT,
            Compression::Lzw,
            Predictor::Horizontal,
        );
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8-lzw-hpred", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB8>(&rgb8, WIDTH, HEIGHT, Compression::Lzw, Predictor::None);
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8-lzw-none", |b| b.iter(|| decode_tiff_bytes(&t)));

        let t = encode::<RGB8>(&rgb8, WIDTH, HEIGHT, Compression::Packbits, Predictor::None);
        group
            .throughput(Throughput::Bytes(raw_rgb8))
            .bench_function("rgb8-packbits", |b| b.iter(|| decode_tiff_bytes(&t)));

        group.finish();
    }
}
