use std::collections::HashMap;
use std::{fs::File, path};

use tiff::decoder::{Decoder, DecodingSampleBuffer};
use tiff::tags::{ByteOrder, Type};
use tiff::ColorType;

use libtest_mimic::{Arguments, Failed, Trial};
use walkdir::WalkDir;

const REFERENCE_DIR: &str = "tests/libtiffpic";

// `find test/libtiffpic -type f | sort`
//
// Each entry maps a file path to its expected behavior:
//   None           = not a TIFF file, skip entirely
//   Some(0)        = TIFF with unsupported features, decode-only smoke test
//   Some(nonzero)  = TIFF with full CRC32 hash verification of decoded output
const FILES: &[(&str, Option<u32>)] = &[
    ("tests/libtiffpic/caspian.tif", Some(0x4ea8d4e4)),
    ("tests/libtiffpic/cramps.tif", Some(0xdfa2fba4)),
    ("tests/libtiffpic/cramps-tile.tif", Some(0xdfa2fba4)),
    ("tests/libtiffpic/CVS/Entries", None),
    ("tests/libtiffpic/CVS/Repository", None),
    ("tests/libtiffpic/CVS/Root", None),
    ("tests/libtiffpic/depth/CVS/Entries", None),
    ("tests/libtiffpic/depth/CVS/Repository", None),
    ("tests/libtiffpic/depth/CVS/Root", None),
    (
        "tests/libtiffpic/depth/flower-minisblack-02.tif",
        Some(0x1c90f515),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-04.tif",
        Some(0x0f4813cb),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-06.tif",
        Some(0xf0c0a901),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-08.tif",
        Some(0xe797b15b),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-10.tif",
        Some(0xdee442ec),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-12.tif",
        Some(0x4810c4e9),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-14.tif",
        Some(0x7e03cf03),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-16.tif",
        Some(0x8e0add32),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-24.tif",
        Some(0x5cf29313),
    ),
    (
        "tests/libtiffpic/depth/flower-minisblack-32.tif",
        Some(0x80025eee),
    ),
    (
        "tests/libtiffpic/depth/flower-palette-02.tif",
        Some(0xaeb21cff),
    ),
    (
        "tests/libtiffpic/depth/flower-palette-04.tif",
        Some(0x339d428b),
    ),
    (
        "tests/libtiffpic/depth/flower-palette-08.tif",
        Some(0xb6b203e9),
    ),
    (
        "tests/libtiffpic/depth/flower-palette-16.tif",
        Some(0x4e02308f),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-02.tif",
        Some(0x8f16cf35),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-04.tif",
        Some(0x95e97f94),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-08.tif",
        Some(0x98134e7c),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-10.tif",
        Some(0xa2ad8f4d),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-12.tif",
        Some(0xfad187fc),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-14.tif",
        Some(0x93fdc170),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-16.tif",
        Some(0x25e1066b),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-24.tif",
        Some(0x25cdf743),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-contig-32.tif",
        Some(0x195dde9c),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-02.tif",
        Some(0xcc0281ca),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-04.tif",
        Some(0x867134b2),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-08.tif",
        Some(0x2341af83),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-10.tif",
        Some(0x79211047),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-12.tif",
        Some(0x14a0ff72),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-14.tif",
        Some(0xc077389d),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-16.tif",
        Some(0xd0849282),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-24.tif",
        Some(0x433d333b),
    ),
    (
        "tests/libtiffpic/depth/flower-rgb-planar-32.tif",
        Some(0xd0780b23),
    ),
    (
        "tests/libtiffpic/depth/flower-separated-contig-08.tif",
        Some(0x97fb03a6),
    ),
    (
        "tests/libtiffpic/depth/flower-separated-contig-16.tif",
        Some(0xb6ee0135),
    ),
    (
        "tests/libtiffpic/depth/flower-separated-planar-08.tif",
        Some(0x502e93fa),
    ),
    (
        "tests/libtiffpic/depth/flower-separated-planar-16.tif",
        Some(0x50e580d9),
    ),
    ("tests/libtiffpic/depth/genimages", None),
    ("tests/libtiffpic/depth/README.txt", None),
    ("tests/libtiffpic/depth/summary.txt", None),
    ("tests/libtiffpic/dscf0013.tif", Some(0)), // unsupported YCbCr chroma subsampling
    ("tests/libtiffpic/fax2d.tif", Some(0xc2f30c6f)),
    // No support for direct CCITT Group 3 1D format
    ("tests/libtiffpic/g3test.g3", None),
    ("tests/libtiffpic/g3test.tif", Some(0x304b606b)),
    ("tests/libtiffpic/jello.tif", Some(0xb03dbda9)),
    ("tests/libtiffpic/jim___ah.tif", Some(0x81a27b7f)),
    ("tests/libtiffpic/jim___cg.tif", Some(0xfd31747c)),
    ("tests/libtiffpic/jim___dg.tif", Some(0x8c6a5e8f)),
    ("tests/libtiffpic/jim___gg.tif", Some(0x8c6a5e8f)),
    ("tests/libtiffpic/ladoga.tif", Some(0x9b118511)),
    ("tests/libtiffpic/off_l16.tif", Some(0)), // LogLuv decoder output varies across platforms
    ("tests/libtiffpic/off_luv24.tif", Some(0x2bee71a2)),
    ("tests/libtiffpic/off_luv32.tif", Some(0)), // LogLuv decoder output varies across platforms
    ("tests/libtiffpic/oxford.tif", Some(0x355d2e3e)),
    ("tests/libtiffpic/pc260001.tif", Some(0xdb56b753)),
    ("tests/libtiffpic/quad-jpeg.tif", Some(0xcba22475)),
    ("tests/libtiffpic/quad-lzw.tif", Some(0)), // invalid LZW stream
    ("tests/libtiffpic/quad-tile.tif", Some(0x6055f9ee)),
    ("tests/libtiffpic/README", None),
    ("tests/libtiffpic/smallliz.tif", Some(0)), // unsupported YCbCr chroma subsampling
    ("tests/libtiffpic/strike.tif", Some(0x016b0f6a)),
    ("tests/libtiffpic/text.tif", Some(0)), // unsupported compression
    ("tests/libtiffpic/ycbcr-cat.tif", Some(0)), // unsupported YCbCr chroma subsampling
    ("tests/libtiffpic/zackthecat.tif", Some(0)), // unsupported YCbCr chroma subsampling
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--print-hashes") {
        print_reference_hashes();
        return;
    }

    let mut candidates: HashMap<_, _> = FILES.iter().copied().collect();
    let mut trials = Vec::new();

    for entry in WalkDir::new(REFERENCE_DIR) {
        let entry = entry.unwrap();

        if !entry.file_type().is_file() {
            continue;
        }

        let Some(path) = entry.path().to_str() else {
            continue;
        };
        let path = path.replace(std::path::MAIN_SEPARATOR, "/"); // always use UNIX path separators

        // Mark this path as consumed in either case by removing it.
        let Some(expected_hash) = candidates.remove(path.as_str()) else {
            panic!("File {} not in reference list", path);
        };

        // None = non-image file, skip
        let Some(expected_hash) = expected_hash else {
            continue;
        };

        let path = path.to_string();
        trials.push(Trial::test(
            path.to_string(),
            move || -> Result<(), Failed> { decode_and_verify(&path, expected_hash) },
        ));
    }

    if !candidates.is_empty() {
        for (path, _) in candidates {
            eprintln!("Reference file {} not found in directory walk", path);
        }

        panic!("Some reference files were not found");
    }

    let args = Arguments::from_args();
    libtest_mimic::run(&args, trials).exit();
}

/// Map DecodingResult variant to a TIFF Type for byte-order normalization.
fn sample_type_of(result: &DecodingSampleBuffer) -> Type {
    match result {
        DecodingSampleBuffer::U8(_) => Type::BYTE,
        DecodingSampleBuffer::I8(_) => Type::SBYTE,
        DecodingSampleBuffer::U16(_) => Type::SHORT,
        DecodingSampleBuffer::I16(_) => Type::SSHORT,
        DecodingSampleBuffer::U32(_) => Type::LONG,
        DecodingSampleBuffer::I32(_) => Type::SLONG,
        DecodingSampleBuffer::F32(_) => Type::FLOAT,
        DecodingSampleBuffer::U64(_) => Type::LONG8,
        DecodingSampleBuffer::I64(_) => Type::SLONG8,
        DecodingSampleBuffer::F64(_) => Type::DOUBLE,
        DecodingSampleBuffer::F16(_) => Type::SHORT, // f16 is 2 bytes like SHORT
    }
}

/// Hash color type into the hasher (discriminant + fields).
fn hash_color_type(hasher: &mut crc32fast::Hasher, color_type: ColorType) {
    match color_type {
        ColorType::Gray(n) => hasher.update(&[0, n]),
        ColorType::RGB(n) => hasher.update(&[1, n]),
        ColorType::Palette(n) => hasher.update(&[2, n]),
        ColorType::GrayA(n) => hasher.update(&[3, n]),
        ColorType::RGBA(n) => hasher.update(&[4, n]),
        ColorType::CMYK(n) => hasher.update(&[5, n]),
        ColorType::CMYKA(n) => hasher.update(&[6, n]),
        ColorType::YCbCr(n) => hasher.update(&[7, n]),
        ColorType::Lab(n) => hasher.update(&[8, n]),
        ColorType::Multiband {
            bit_depth,
            num_samples,
        } => {
            hasher.update(&[9, bit_depth]);
            hasher.update(&num_samples.to_le_bytes());
        }
        other => panic!("compute_tiff_hash: unhandled ColorType {other:?}"),
    }
}

/// Compute a CRC32 hash of the full decoded TIFF output using read_image_to_buffer.
///
/// Hashes per page: page index, dimensions, color type, TIFF sample type tag,
/// and decoded sample data normalized to little-endian byte order.
fn compute_hash(path: &std::path::Path) -> Result<u32, tiff::TiffError> {
    let img_file = File::open(path)?;
    let mut decoder = Decoder::open(img_file)?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = DecodingSampleBuffer::U8(vec![]);

    let mut page = 0u32;
    while decoder.more_images() {
        decoder.next_image()?;
        let (width, height) = decoder.dimensions()?;
        let color_type = decoder.colortype()?;
        let _layout = decoder.read_image_to_buffer(&mut buffer)?;

        hasher.update(&page.to_le_bytes());
        hasher.update(&width.to_le_bytes());
        hasher.update(&height.to_le_bytes());
        hash_color_type(&mut hasher, color_type);

        let sample_type = sample_type_of(&buffer);
        hasher.update(&sample_type.to_u16().to_le_bytes());

        let mut buf = buffer.as_buffer(0);
        let bytes = buf.as_bytes_mut();
        ByteOrder::native().convert(sample_type, bytes, ByteOrder::LittleEndian);
        hasher.update(bytes);

        page += 1;
    }

    hasher.update(&page.to_le_bytes());
    Ok(hasher.finalize())
}

fn decode_and_verify(path: &str, expected_hash: u32) -> Result<(), Failed> {
    let full_path = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);

    if expected_hash == 0 {
        // Smoke test: try to decode, tolerate errors (unsupported formats)
        let _ = compute_hash(&full_path);
        return Ok(());
    }

    let actual_hash = compute_hash(&full_path)
        .map_err(|e| Failed::from(format!("Decoding failed for {path}: {e}")))?;

    if actual_hash != expected_hash {
        return Err(Failed::from(format!(
            "{path}: expected hash 0x{expected_hash:08x} but got 0x{actual_hash:08x}"
        )));
    }

    Ok(())
}

/// Print reference hashes for all decodable files (use --print-hashes).
fn print_reference_hashes() {
    for &(path, expected) in FILES {
        if expected.is_none() {
            continue;
        }
        let full_path = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
        match compute_hash(&full_path) {
            Ok(hash) => eprintln!("(\"{path}\", Some(0x{hash:08x})),"),
            Err(e) => eprintln!("// {path}: FAILED: {e}"),
        }
    }
}
