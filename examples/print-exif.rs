extern crate exif;
extern crate tiff;

use tiff::{
    decoder::TiffDecoder,
    ifd::{process, ProcessedEntry},
    tags::{GpsTag, Tag},
};

use clap::Parser;
use std::fs::File;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the exposure folder containing the index.tse file
    #[arg(required = true)]
    path: PathBuf,
}

fn main() {
    let args = Cli::parse();

    let img_file = File::open(args.path).expect("Cannot find test image!");
    let mut decoder = TiffDecoder::new(img_file).expect("Cannot create decoder");
    let mut exif = decoder
        .get_exif_data()
        .expect("Unable to read Exif data")
        .into_iter()
        .map(|(id, be)| process(be).map(|e| (id, e)))
        .collect::<Result<Vec<(Tag, ProcessedEntry)>, _>>()
        .unwrap();

    exif.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    exif.into_iter()
        .for_each(|(id, entry)| println!("{id:?}:\t{entry}"));

    decoder
        .get_exif_ifd(Tag::ExifIfd)
        .expect("Unable to read Exif data")
        .into_iter()
        .map(|(id, be)| process(be).map(|e| (id, e)))
        .collect::<Result<std::collections::HashMap<Tag, ProcessedEntry>, _>>()
        .unwrap()
        .into_iter()
        .for_each(|(id, entry)| println!("{id:?}:\t{entry}"));

    let mut exif = decoder
        .get_gps_ifd()
        .expect("Unable to read Exif data")
        .into_iter()
        .map(|(id, be)| process(be).map(|e| (id, e)))
        .collect::<Result<Vec<(GpsTag, ProcessedEntry)>, _>>()
        .unwrap();

    exif.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    exif.into_iter()
        .for_each(|(id, entry)| println!("{id:?}:\t{entry}"));

    decoder
        .get_exif_ifd(Tag::InteropIfd)
        .expect("Unable to read Exif data")
        .into_iter()
        .map(|(id, be)| process(be).map(|e| (id, e)))
        .collect::<Result<std::collections::HashMap<Tag, ProcessedEntry>, _>>()
        .unwrap()
        .into_iter()
        .for_each(|(id, entry)| println!("{id:?}:\t{entry}"));
}
