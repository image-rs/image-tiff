extern crate tiff;

use std::io::{Cursor, Seek, Write};
use tiff::{
    decoder::{Decoder, DecodingSampleBuffer},
    encoder::{
        colortype::{self, ColorType},
        Compression, TiffEncoder, TiffValue,
    },
};

trait TestImage<const NUM_CHANNELS: usize>: From<Vec<<Self::Color as ColorType>::Inner>> {
    const WIDTH: u32;
    const HEIGHT: u32;
    type Color: ColorType;

    fn reference_data(&self) -> &[<Self::Color as ColorType>::Inner];
    fn generate_pixel(x: u32, y: u32) -> [<Self::Color as ColorType>::Inner; NUM_CHANNELS];

    fn compress<W: Write + Seek>(&self, encoder: &mut TiffEncoder<W>)
    where
        [<Self::Color as ColorType>::Inner]: TiffValue,
    {
        let image = encoder
            .new_image::<Self::Color>(Self::WIDTH, Self::HEIGHT)
            .unwrap();
        image.write_data(self.reference_data()).unwrap();
    }

    fn generate() -> Self {
        assert_eq!(
            Self::Color::BITS_PER_SAMPLE.len(),
            NUM_CHANNELS,
            "Incompatible color type"
        );

        let mut data = Vec::with_capacity((Self::WIDTH * Self::HEIGHT) as usize * NUM_CHANNELS);
        for x in 0..Self::WIDTH {
            for y in 0..Self::HEIGHT {
                data.extend(IntoIterator::into_iter(Self::generate_pixel(x, y)));
            }
        }
        Self::from(data)
    }
}

struct TestImageColor(Vec<u16>);

impl From<Vec<u16>> for TestImageColor {
    fn from(value: Vec<u16>) -> Self {
        Self(value)
    }
}

impl TestImage<3> for TestImageColor {
    const WIDTH: u32 = 1;
    const HEIGHT: u32 = 7;
    type Color = colortype::RGB16;

    fn reference_data(&self) -> &[u16] {
        &self.0
    }

    fn generate_pixel(x: u32, y: u32) -> [<Self::Color as ColorType>::Inner; 3] {
        let val = (x + y) % <Self::Color as ColorType>::Inner::MAX as u32;
        [val as <Self::Color as ColorType>::Inner; 3]
    }
}

struct TestImageGrayscale(Vec<u8>);

impl From<Vec<u8>> for TestImageGrayscale {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl TestImage<1> for TestImageGrayscale {
    const WIDTH: u32 = 21;
    const HEIGHT: u32 = 10;
    type Color = colortype::Gray8;

    fn reference_data(&self) -> &[u8] {
        &self.0
    }

    fn generate_pixel(x: u32, y: u32) -> [<Self::Color as ColorType>::Inner; 1] {
        let val = (x + y) % <Self::Color as ColorType>::Inner::MAX as u32;
        [val as <Self::Color as ColorType>::Inner]
    }
}

fn encode_decode_with_compression(compression: Compression) {
    let mut data = Cursor::new(Vec::new());

    let image_rgb = TestImageColor::generate();
    let image_grayscale = TestImageGrayscale::generate();

    // Encode tiff with compression
    {
        // Create a multipage image with 2 images
        let mut encoder = TiffEncoder::new(&mut data)
            .unwrap()
            .with_compression(compression);
        image_rgb.compress(&mut encoder);
        image_grayscale.compress(&mut encoder);
    }

    // Decode tiff
    data.set_position(0);
    {
        let mut decoder = Decoder::open(data).unwrap();
        decoder.next_image().unwrap();

        // Check the RGB image
        assert_eq!(
            match decoder.read_image() {
                Ok(DecodingSampleBuffer::U16(image_data)) => image_data,
                unexpected => panic!("Descoding RGB failed: {unexpected:?}"),
            },
            image_rgb.reference_data()
        );

        // Check the grayscale image
        decoder.next_image().unwrap();
        assert_eq!(
            match decoder.read_image() {
                Ok(DecodingSampleBuffer::U8(image_data)) => image_data,
                unexpected => panic!("Decoding grayscale failed: {unexpected:?}"),
            },
            image_grayscale.reference_data()
        );
    }
}

#[test]
fn encode_decode_without_compression() {
    encode_decode_with_compression(Compression::Uncompressed);
}

#[test]
#[cfg(feature = "lzw")]
fn encode_decode_with_lzw() {
    encode_decode_with_compression(Compression::Lzw);
}

#[test]
#[cfg(feature = "deflate")]
fn encode_decode_with_deflate() {
    encode_decode_with_compression(Compression::Deflate(1));
    encode_decode_with_compression(Compression::Deflate(6));
    encode_decode_with_compression(Compression::Deflate(9));
}

#[test]
fn encode_decode_with_packbits() {
    encode_decode_with_compression(Compression::Packbits);
}

#[test]
fn rgba_extra_samples() {
    use tiff::tags::Tag;

    fn check<C: ColorType>(data: &[C::Inner], extras: Option<Vec<u16>>)
    where
        [C::Inner]: TiffValue,
    {
        let mut output = Cursor::new(Vec::new());
        TiffEncoder::new(&mut output)
            .unwrap()
            .write_image::<C>(1, 1, data)
            .unwrap();
        output.set_position(0);
        let mut decoder = Decoder::open(output).unwrap();
        decoder.next_image().unwrap();
        assert_eq!(
            decoder
                .current_ifd()
                .find_tag_unsigned_vec::<u16>(Tag::ExtraSamples)
                .unwrap(),
            extras
        );
        assert_eq!(
            decoder.read_image().unwrap().as_buffer(0).as_bytes(),
            data.data().as_ref()
        );
    }
    check::<colortype::RGBA8>(&[200, 100, 25, 64], Some(vec![2]));
    check::<colortype::RGBA16>(&[200, 100, 25, 64], Some(vec![2]));
    check::<colortype::RGBA32>(&[200, 100, 25, 64], Some(vec![2]));
    check::<colortype::RGBA64>(&[200, 100, 25, 64], Some(vec![2]));
    check::<colortype::RGBA32Float>(&[1.2, 0.5, 0.75, 0.25], Some(vec![2]));
    check::<colortype::RGBA64Float>(&[1.2, 0.5, 0.75, 0.25], Some(vec![2]));
    check::<colortype::RGB8>(&[200, 100, 25], None);
    check::<colortype::CMYK8>(&[200, 100, 25, 64], None);
}

#[test]
fn rgba_extra_samples_configuration() {
    use tiff::tags::{ExtraSamples, Tag};

    for append in [false, true] {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut output).unwrap();
            let mut image = encoder.new_image::<colortype::RGBA8>(1, 1).unwrap();
            if append {
                image.extra_samples(&[ExtraSamples::Unspecified]).unwrap();
            } else {
                image
                    .encoder()
                    .write_tag(Tag::ExtraSamples, &[1u16][..])
                    .unwrap();
            }
            image
                .write_data(&[25, 30, 35, 64, 9][..4 + usize::from(append)])
                .unwrap();
        }
        output.set_position(0);
        let mut decoder = Decoder::open(output).unwrap();
        decoder.next_image().unwrap();
        let mut ifd = decoder.current_ifd();
        assert_eq!(
            ifd.get_tag_u16_vec(Tag::ExtraSamples).unwrap(),
            if append { vec![2, 0] } else { vec![1] }
        );
        assert_eq!(
            ifd.get_tag_u32(Tag::SamplesPerPixel).unwrap(),
            if append { 5 } else { 4 }
        );
        assert!(
            matches!(decoder.read_image().unwrap(), DecodingSampleBuffer::U8(data) if data == [25, 30, 35, 64])
        );
    }
}

#[test]
fn encoded_metadata_is_aligned() {
    use tiff::encoder::{TiffKind, TiffKindBig, TiffKindStandard};
    use tiff::tags::Tag;

    fn check<K: TiffKind>(big: bool) {
        let mut output = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::<_, K>::new_generic(&mut output).unwrap();
            let mut image = encoder.new_image::<colortype::Gray8>(1, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::Artist, "first extended value")
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ImageDescription, "second extended value")
                .unwrap();
            image.write_data(&[7]).unwrap();
        }
        let bytes = output.into_inner();
        let offset = |data: &[u8]| {
            if big {
                u64::from_ne_bytes(data[..8].try_into().unwrap()) as usize
            } else {
                u32::from_ne_bytes(data[..4].try_into().unwrap()) as usize
            }
        };
        let alignment = if big { 8 } else { 2 };
        let ifd = offset(&bytes[if big { 8 } else { 4 }..]);
        assert_eq!(ifd % alignment, 0, "unaligned IFD");
        let count = if big {
            offset(&bytes[ifd..])
        } else {
            u16::from_ne_bytes(bytes[ifd..ifd + 2].try_into().unwrap()) as usize
        };
        let start = ifd + if big { 8 } else { 2 };
        for entry in bytes[start..]
            .chunks_exact(if big { 20 } else { 12 })
            .take(count)
        {
            let tag = u16::from_ne_bytes(entry[..2].try_into().unwrap());
            if tag == Tag::Artist.to_u16() || tag == Tag::ImageDescription.to_u16() {
                assert_eq!(
                    offset(&entry[if big { 12 } else { 8 }..]) % alignment,
                    0,
                    "unaligned tag value"
                );
            }
        }
    }
    check::<TiffKindStandard>(false);
    check::<TiffKindBig>(true);
}

#[test]
fn encode_decode_strips_with_compression() {
    use tiff::tags::Tag;

    let data: Vec<u8> = (0..30).collect();
    for compression in [
        Compression::Uncompressed,
        Compression::Packbits,
        #[cfg(feature = "lzw")]
        Compression::Lzw,
        #[cfg(feature = "deflate")]
        Compression::Deflate(6),
    ] {
        let mut file = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut file)
                .unwrap()
                .with_compression(compression);
            let mut image = encoder.new_image::<colortype::Gray8>(10, 3).unwrap();
            assert!(image.rows_per_strip(0).is_err());
            image.rows_per_strip(1).unwrap();
            for row in data.as_chunks::<10>().0 {
                image.write_strip(row).unwrap();
                image
                    .encoder()
                    .write_tag(Tag::Artist, "Separate strips")
                    .unwrap();
            }
            assert!(image.write_strip(&[]).is_err());
            image.finish().unwrap();
        }
        file.set_position(0);
        let mut decoder = Decoder::open(file).unwrap();
        decoder.next_image().unwrap();
        assert_eq!(
            decoder
                .current_ifd()
                .get_tag_ascii_string(Tag::Artist)
                .unwrap(),
            "Separate strips"
        );
        assert!(
            matches!(decoder.read_image().unwrap(), DecodingSampleBuffer::U8(actual) if actual == data)
        );
    }
}

#[test]
#[cfg(feature = "zstd")]
fn encode_decode_with_zstd() {
    for level in [-3, 0, 6] {
        encode_decode_with_compression(Compression::Zstd(level));
    }
}

#[test]
#[cfg(feature = "zstd")]
fn zstd_strips_predictors_and_bigtiff() {
    use tiff::encoder::{Predictor, TiffKind, TiffKindBig, TiffKindStandard};
    use tiff::tags::Tag;

    fn check<K: TiffKind>(manual: bool) {
        let rgba: Vec<u8> = (0..7 * 5 * 4).map(|i| (i * 37) as u8).collect();
        let rgb: Vec<u16> = (0..7 * 5 * 3).map(|i| (i * 117) as u16).collect();
        let gray: Vec<f32> = (0..7 * 5).map(|i| i as f32 * 0.25 - 4.0).collect();
        let mut file = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::<_, K>::new_generic(&mut file)
                .unwrap()
                .with_compression(Compression::Zstd(0));
            macro_rules! image {
                ($color:ty, $data:expr, $predictor:expr) => {
                    encoder = encoder.with_predictor($predictor);
                    let mut image = encoder.new_image::<$color>(7, 5).unwrap();
                    image.rows_per_strip(2).unwrap();
                    image.encoder().write_tag(Tag::Artist, "Zstd").unwrap();
                    if manual {
                        let mut remaining = $data.as_slice();
                        while !remaining.is_empty() {
                            let count = image.next_strip_sample_count() as usize;
                            image.write_strip(&remaining[..count]).unwrap();
                            remaining = &remaining[count..];
                            image.encoder().write_tag(Tag::Artist, "Zstd").unwrap();
                        }
                        image.finish().unwrap();
                    } else {
                        image.write_data($data).unwrap();
                    }
                };
            }
            image!(colortype::RGBA8, &rgba, Predictor::Horizontal);
            image!(colortype::RGB16, &rgb, Predictor::Horizontal);
            image!(colortype::Gray32Float, &gray, Predictor::FloatingPoint);
        }
        let bytes = file.into_inner();
        let mut decoder = Decoder::open(Cursor::new(&bytes)).unwrap();
        for mut expected in [
            DecodingSampleBuffer::U8(rgba),
            DecodingSampleBuffer::U16(rgb),
            DecodingSampleBuffer::F32(gray),
        ] {
            decoder.next_image().unwrap();
            assert_eq!(decoder.dimensions().unwrap(), (7, 5));
            let mut ifd = decoder.current_ifd();
            assert_eq!(ifd.get_tag_u32(Tag::Compression).unwrap(), 50000);
            assert_eq!(ifd.get_tag_ascii_string(Tag::Artist).unwrap(), "Zstd");
            let offsets = ifd.get_tag_u64_vec(Tag::StripOffsets).unwrap();
            let counts = ifd.get_tag_u64_vec(Tag::StripByteCounts).unwrap();
            assert_eq!((offsets.len(), counts.len()), (3, 3));
            for (offset, count) in offsets.into_iter().zip(counts) {
                let frame = &bytes[offset as usize..(offset + count) as usize];
                assert_eq!(
                    zstd::zstd_safe::find_frame_compressed_size(frame).unwrap(),
                    frame.len()
                );
            }
            let mut actual = decoder.read_image().unwrap();
            assert_eq!(
                actual.as_buffer(0).as_bytes(),
                expected.as_buffer(0).as_bytes()
            );
        }
        assert!(!decoder.more_images());
    }
    for manual in [false, true] {
        check::<TiffKindStandard>(manual);
        check::<TiffKindBig>(manual);
    }
}
