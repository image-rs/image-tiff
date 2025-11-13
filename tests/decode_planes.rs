use tiff::decoder::Decoder;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

#[test]
fn read_all_planes() {
    let path = PathBuf::from(TEST_IMAGE_DIR).join("planar-rgb-u8.tif");
    let file = File::open(path).unwrap();
    let io = std::io::BufReader::new(file);

    let mut tif = Decoder::new(io).unwrap();
    let layout = tif.image_buffer_layout().unwrap();
    assert_eq!(layout.planes, 3);
    let plane_stride = layout.plane_stride.map_or(0, |x| x.get());

    let mut buffer = vec![0u8; layout.planes * plane_stride];
    tif.read_image_bytes(&mut buffer).unwrap();

    // Mainly you can see: these are different and non-zero. Otherwise these magic constants just
    // depend on the file itself that is being used.
    const PLANE_EXCPECTED_SUMS: [u32; 3] = [15417630, 13007788, 11103530];

    for (plane, expected) in (0..layout.planes).zip(PLANE_EXCPECTED_SUMS) {
        let plane_data = &buffer[plane_stride * plane..][..layout.len];
        let sum: u32 = plane_data.iter().copied().map(u32::from).sum();
        assert_eq!(sum, expected);
    }
}
