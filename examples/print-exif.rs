extern crate exif;
extern crate tiff;

use tiff::{decoder::TiffDecoder, tags::Tag};

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
    let exif = decoder.get_exif_data().expect("Unable to read Exif data");

    println!("Base: {exif:#?}");

    let exif = decoder
        .get_exif_ifd(Tag::ExifIfd)
        .expect("Unable to read Exif data");

    println!("Extra: {exif:#?}");

    let exif = decoder
        .get_exif_ifd(Tag::GpsIfd)
        .expect("Unable to read Exif data");

    println!("GPS: {exif:#?}");

    let exif = decoder
        .get_exif_ifd(Tag::InteropIfd)
        .expect("Unable to read Exif data");

    println!("Interop: {exif:#?}");
}
