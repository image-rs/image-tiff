use std::collections::HashMap;
use std::{fs::File, path};

use tiff::decoder::{Decoder, DecodingResult};

use libtest_mimic::{Arguments, Failed, Trial};
use walkdir::WalkDir;

const REFERENCE_DIR: &str = "tests/libtiffpic";

// `find test/libtiffpic -type f | sort`
const FILES: &[(&str, bool)] = &[
    ("tests/libtiffpic/caspian.tif", true),
    ("tests/libtiffpic/cramps.tif", true),
    // FIXME: See <https://github.com/image-rs/image-tiff/issues/338>, this has tiles as
    // StripOffsets and has a RowsPerStrip.
    ("tests/libtiffpic/cramps-tile.tif", false),
    ("tests/libtiffpic/CVS/Entries", false),
    ("tests/libtiffpic/CVS/Repository", false),
    ("tests/libtiffpic/CVS/Root", false),
    ("tests/libtiffpic/depth/CVS/Entries", false),
    ("tests/libtiffpic/depth/CVS/Repository", false),
    ("tests/libtiffpic/depth/CVS/Root", false),
    ("tests/libtiffpic/depth/flower-minisblack-02.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-04.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-06.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-08.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-10.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-12.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-14.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-16.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-24.tif", true),
    ("tests/libtiffpic/depth/flower-minisblack-32.tif", true),
    ("tests/libtiffpic/depth/flower-palette-02.tif", true),
    ("tests/libtiffpic/depth/flower-palette-04.tif", true),
    ("tests/libtiffpic/depth/flower-palette-08.tif", true),
    ("tests/libtiffpic/depth/flower-palette-16.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-02.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-04.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-08.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-10.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-12.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-14.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-16.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-24.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-contig-32.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-02.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-04.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-08.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-10.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-12.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-14.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-16.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-24.tif", true),
    ("tests/libtiffpic/depth/flower-rgb-planar-32.tif", true),
    (
        "tests/libtiffpic/depth/flower-separated-contig-08.tif",
        true,
    ),
    (
        "tests/libtiffpic/depth/flower-separated-contig-16.tif",
        true,
    ),
    (
        "tests/libtiffpic/depth/flower-separated-planar-08.tif",
        true,
    ),
    (
        "tests/libtiffpic/depth/flower-separated-planar-16.tif",
        true,
    ),
    ("tests/libtiffpic/depth/genimages", false),
    ("tests/libtiffpic/depth/README.txt", false),
    ("tests/libtiffpic/depth/summary.txt", false),
    ("tests/libtiffpic/dscf0013.tif", true),
    ("tests/libtiffpic/fax2d.tif", true),
    // No support for direct CCITT Group 3 1D format
    ("tests/libtiffpic/g3test.g3", false),
    ("tests/libtiffpic/g3test.tif", true),
    ("tests/libtiffpic/jello.tif", true),
    ("tests/libtiffpic/jim___ah.tif", true),
    ("tests/libtiffpic/jim___cg.tif", true),
    ("tests/libtiffpic/jim___dg.tif", true),
    ("tests/libtiffpic/jim___gg.tif", true),
    ("tests/libtiffpic/ladoga.tif", true),
    ("tests/libtiffpic/off_l16.tif", true),
    ("tests/libtiffpic/off_luv24.tif", true),
    ("tests/libtiffpic/off_luv32.tif", true),
    ("tests/libtiffpic/oxford.tif", true),
    ("tests/libtiffpic/pc260001.tif", true),
    ("tests/libtiffpic/quad-jpeg.tif", true),
    ("tests/libtiffpic/quad-lzw.tif", true),
    // FIXME: See <https://github.com/image-rs/image-tiff/issues/338>, this has tiles as
    // StripOffsets and has a RowsPerStrip.
    ("tests/libtiffpic/quad-tile.tif", false),
    ("tests/libtiffpic/README", false),
    ("tests/libtiffpic/smallliz.tif", true),
    ("tests/libtiffpic/strike.tif", true),
    ("tests/libtiffpic/text.tif", true),
    ("tests/libtiffpic/ycbcr-cat.tif", true),
    ("tests/libtiffpic/zackthecat.tif", true),
];

fn main() {
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

        // Mark this path as consumed in either case by removing it.
        let Some(should_decode) = candidates.remove(path) else {
            panic!("File {} not in reference list", path);
        };

        // Check if we should turn it into a test case.
        if !should_decode {
            continue;
        }

        let path = path.to_string();
        trials.push(Trial::test(
            path.to_string(),
            move || -> Result<(), Failed> {
                decode_libtiffpic(&path)
                    .map_err(|e| Failed::from(format_args!("Decoding failed for {}: {}", path, e)))
            },
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

fn decode_libtiffpic(path: &str) -> Result<(), tiff::TiffError> {
    let path = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);

    let img_file = File::open(path)?;
    let mut decoder = Decoder::new(img_file)?;
    let mut buffer = DecodingResult::U8(vec![]);

    loop {
        let _layout = decoder.read_image_to_buffer(&mut buffer);

        if !decoder.more_images() {
            break;
        }

        decoder.next_image()?;
    }

    Ok(())
}
