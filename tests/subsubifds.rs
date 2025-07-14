use tiff::{
    decoder::Decoder,
    tags::{IfdPointer, Tag},
};

// This file has the following IFD structure:
//
// ```
// IFD -> IFD -> IFD
//        |-> IFD
//        |-> IFD -> IFD
//        |          \-> IFD -> IFD
//        \-> IFD
// ```
//
// where the main chain of IFD has 3 images, the second of which has several sub-IFDs and some have
// more nested IFDs.
const TEST_IMAGE_SUBIFD: &str = "./tests/subsubifds.tif";
use std::fs::File;

#[test]
fn decode_seek_chain() {
    let file = File::open(TEST_IMAGE_SUBIFD).expect("Cannot open test image");
    let mut decoder = Decoder::new(file).expect("Invalid format to create decoder");

    // Remember the offsets in the first iteration.
    let offset0 = decoder.ifd_pointer().expect("First IFD pointer not found");
    decoder.next_image().unwrap();
    let offset1 = decoder.ifd_pointer().expect("Second IFD pointer not found");
    decoder.next_image().unwrap();
    let offset2 = decoder.ifd_pointer().expect("Third IFD pointer not found");
    assert!(!decoder.more_images());

    // Ensure we can seek, to do this again.
    decoder
        .seek_to_image(0)
        .expect("Failed to seek to first image");
    assert_eq!(Some(offset0), decoder.ifd_pointer());
    decoder.next_image().unwrap();
    assert_eq!(Some(offset1), decoder.ifd_pointer());
    decoder.next_image().unwrap();
    assert_eq!(Some(offset2), decoder.ifd_pointer());
    assert!(!decoder.more_images());
}

#[test]
fn decode_seek_recover() {
    let file = File::open(TEST_IMAGE_SUBIFD).expect("Cannot open test image");
    let mut decoder = Decoder::new(file).expect("Invalid format to create decoder");

    // Remember the offsets in the first iteration.
    let offset0 = decoder.ifd_pointer().expect("First IFD pointer not found");
    decoder.next_image().unwrap();
    let offset1 = decoder.ifd_pointer().expect("Second IFD pointer not found");
    decoder.next_image().unwrap();
    let offset2 = decoder.ifd_pointer().expect("Third IFD pointer not found");

    // oops!
    let fails = decoder.restart_at_image(IfdPointer(0xdead_beef));
    assert!(fails.is_err());

    // However we can recover by restarting our seek.
    decoder
        .restart_at_image(offset0)
        .expect("Failed to restart IFDs from first image");
    assert_eq!(Some(offset0), decoder.ifd_pointer());
    decoder.next_image().unwrap();
    assert_eq!(Some(offset1), decoder.ifd_pointer());
    decoder.next_image().unwrap();
    assert_eq!(Some(offset2), decoder.ifd_pointer());
}

#[test]
fn decode_seek_directory() {
    let file = File::open(TEST_IMAGE_SUBIFD).expect("Cannot open test image");
    let mut decoder = Decoder::new(file).expect("Invalid format to create decoder");

    let offset = decoder.ifd_pointer().expect("First IFD pointer not found");
    let img0 = decoder.read_image().unwrap();

    decoder.next_image().unwrap();
    let img1 = decoder.read_image().unwrap();
    assert_ne!(img0, img1);

    {
        // Due to the special test file structure.
        let subifd = decoder
            .get_tag(Tag::SubIfd)
            .unwrap()
            .into_ifd_vec()
            .unwrap()[0];

        // Let's try to manually interpret this as an image.
        decoder
            .restart_at_directory(subifd)
            .expect("Failed to restart IFDs at SubIfd");
        decoder
            .current_directory_as_image()
            .expect("Failed to read SubIfd image");

        assert_eq!(decoder.dimensions().unwrap(), (32, 32));
        let subimage = decoder.read_image().unwrap();
        // Verify we did not accidentally read another image.
        assert_ne!(img0, subimage);
        assert_ne!(img1, subimage);
    }

    // And finally, seek back to the first image.
    decoder
        .restart_at_image(offset)
        .expect("First IFD no longer readable");
    assert_eq!(Some(offset), decoder.ifd_pointer());

    let img0_try2 = decoder.read_image().unwrap();
    assert_eq!(img0, img0_try2);
}
