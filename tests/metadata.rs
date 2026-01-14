use tiff::{
    decoder::Decoder,
    encoder::{colortype::Gray8, EncodeMetadataOptions, TiffEncoder},
    tags::Tag,
};

#[test]
fn copies_exif_data() {
    let src = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/images/predictor-3-gray-f32.tif"
    ));

    let src = std::fs::File::open(src).unwrap();
    let mut src = Decoder::new(src).unwrap();

    let mut data = std::io::Cursor::new(vec![]);
    {
        let mut dst = TiffEncoder::new(&mut data).unwrap();
        let mut img = dst.new_image::<Gray8>(1, 1).unwrap();

        let report = img
            .write_metadata_copy_from(&mut src, EncodeMetadataOptions::default())
            .unwrap();

        assert_eq!(report.copied, 8);
        assert_eq!(report.filtered, 0, "There are no tags we do not recognize");
        img.write_data(&[0x80]).unwrap();
    }

    data.set_position(0);

    {
        let mut roundtrip = Decoder::new(data).unwrap();
        assert!(roundtrip.find_tag(Tag::DateTime).unwrap().is_some());
    }
}
