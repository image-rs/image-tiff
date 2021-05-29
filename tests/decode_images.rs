extern crate tiff;

use tiff::decoder::{ifd, Decoder, DecodingResult};
use tiff::ColorType;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

macro_rules! test_image_sum {
    ($name:ident, $buffer:ident, $sum_ty:ty) => {
        fn $name(file: &str, expected_type: ColorType, expected_sum: $sum_ty) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let img_file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
            assert_eq!(decoder.colortype().unwrap(), expected_type);
            let img_res = decoder.read_image().unwrap();

            match img_res {
                DecodingResult::$buffer(res) => {
                    let sum: $sum_ty = res.into_iter().map(<$sum_ty>::from).sum();
                    assert_eq!(sum, expected_sum);
                }
                _ => panic!("Wrong bit depth"),
            }
        }
    };
}

test_image_sum!(test_image_sum_u8, U8, u64);
test_image_sum!(test_image_sum_i8, I8, i64);
test_image_sum!(test_image_sum_u16, U16, u64);
test_image_sum!(test_image_sum_i16, I16, i64);
test_image_sum!(test_image_sum_u32, U32, u64);
test_image_sum!(test_image_sum_u64, U64, u64);
test_image_sum!(test_image_sum_f32, F32, f32);
test_image_sum!(test_image_sum_f64, F64, f64);

/// Tests that a decoder can be constructed for an image and the color type
/// read from the IFD and is of the appropriate type, but the type is
/// unsupported.
fn test_image_color_type_unsupported(file: &str, expected_type: ColorType) {
    let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), expected_type);
    assert!(match decoder.read_image() {
        Err(tiff::TiffError::UnsupportedError(
            tiff::TiffUnsupportedError::UnsupportedColorType(_),
        )) => true,
        _ => false,
    });
}

#[test]
fn test_cmyk_u8() {
    test_image_sum_u8("cmyk-3c-8b.tiff", ColorType::CMYK(8), 8522658);
}

#[test]
fn test_cmyk_u16() {
    test_image_sum_u16("cmyk-3c-16b.tiff", ColorType::CMYK(16), 2181426827);
}

#[test]
fn test_cmyk_f32() {
    test_image_sum_f32("cmyk-3c-32b-float.tiff", ColorType::CMYK(32), 496.0405);
}

#[test]
fn test_gray_u8() {
    test_image_sum_u8("minisblack-1c-8b.tiff", ColorType::Gray(8), 2840893);
}

#[test]
fn test_gray_u12() {
    test_image_color_type_unsupported("12bit.cropped.tiff", ColorType::Gray(12));
}

#[test]
fn test_gray_u16() {
    test_image_sum_u16("minisblack-1c-16b.tiff", ColorType::Gray(16), 733126239);
}

#[test]
fn test_gray_u32() {
    test_image_sum_u32("gradient-1c-32b.tiff", ColorType::Gray(32), 549892913787);
}

#[test]
fn test_gray_u64() {
    test_image_sum_u64("gradient-1c-64b.tiff", ColorType::Gray(64), 549892913787);
}

#[test]
fn test_gray_f32() {
    test_image_sum_f32("gradient-1c-32b-float.tiff", ColorType::Gray(32), 128.03194);
}

#[test]
fn test_gray_f64() {
    test_image_sum_f64(
        "gradient-1c-64b-float.tiff",
        ColorType::Gray(64),
        128.0319210877642,
    );
}

#[test]
fn test_rgb_u8() {
    test_image_sum_u8("rgb-3c-8b.tiff", ColorType::RGB(8), 7842108);
}

#[test]
fn test_rgb_u12() {
    test_image_color_type_unsupported("12bit.cropped.rgb.tiff", ColorType::RGB(12));
}

#[test]
fn test_rgb_u16() {
    test_image_sum_u16("rgb-3c-16b.tiff", ColorType::RGB(16), 2024349944);
}

#[test]
fn test_rgb_u32() {
    test_image_sum_u32("gradient-3c-32b.tiff", ColorType::RGB(32), 2030834111716);
}

#[test]
fn test_rgb_u64() {
    test_image_sum_u64("gradient-3c-64b.tiff", ColorType::RGB(64), 2030834111716);
}

#[test]
fn test_rgb_f32() {
    test_image_sum_f32("gradient-3c-32b-float.tiff", ColorType::RGB(32), 472.8405);
}

#[test]
fn test_int8() {
    test_image_sum_i8("int8.tif", ColorType::Gray(8), 3111)
}

#[test]
fn test_int8_rgb() {
    test_image_sum_i8("int8_rgb.tif", ColorType::RGB(8), -10344)
}

#[test]
fn test_int16() {
    test_image_sum_i16("int16.tif", ColorType::Gray(16), 354396);
}

#[test]
fn test_int16_rgb() {
    test_image_sum_i16("int16_rgb.tif", ColorType::RGB(16), 1063188);
}

#[test]
fn test_string_tags() {
    // these files have null-terminated strings for their Software tag. One has extra bytes after
    // the null byte, so we check both to ensure that we're truncating properly
    let filenames = ["minisblack-1c-16b.tiff", "rgb-3c-16b.tiff"];
    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        let software = decoder.get_tag(tiff::tags::Tag::Software).unwrap();
        match software {
            ifd::Value::Ascii(s) => assert_eq!(
                &s,
                "GraphicsMagick 1.2 unreleased Q16 http://www.GraphicsMagick.org/"
            ),
            _ => assert!(false),
        };
    }
}

#[test]
fn test_decode_data() {
    let mut image_data = Vec::new();
    for x in 0..100 {
        for y in 0..100u8 {
            let val = x + y;
            image_data.push(val);
            image_data.push(val);
            image_data.push(val);
        }
    }
    let file = File::open("./tests/decodedata-rgb-3c-8b.tiff").unwrap();
    let mut decoder = Decoder::new(file).unwrap();
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
    assert_eq!(decoder.dimensions().unwrap(), (100, 100));
    if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
        assert_eq!(image_data, img_res);
    } else {
        panic!("Wrong data type");
    }
}

#[test]
fn issue_69() {
    test_image_sum_u16("issue_69_lzw.tiff", ColorType::Gray(16), 1015486);
    test_image_sum_u16("issue_69_packbits.tiff", ColorType::Gray(16), 1015486);
}

// TODO: GrayA support
//#[test]
//fn test_gray_alpha_u8()
//{
//let img_file = File::open("./tests/images/minisblack-2c-8b-alpha.tiff").expect("Cannot find test image!");
//let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
//assert_eq!(decoder.colortype().unwrap(), ColorType::GrayA(8));
//let img_res = decoder.read_image();
//assert!(img_res.is_ok());
//}

#[test]
fn test_tiled_rgb_u8() {
    test_image_sum_u8("tiled-rgb-u8.tif", ColorType::RGB(8), 39528948);
}

#[test]
fn test_tiled_rect_rgb_u8() {
    test_image_sum_u8("tiled-rect-rgb-u8.tif", ColorType::RGB(8), 62081032);
}

/* #[test]
fn test_tiled_jpeg_rgb_u8() {
    test_image_sum_u8("tiled-jpeg-rgb-u8.tif", ColorType::RGB(8), 93031606);
} */

#[test]
fn test_tiled_oversize_gray_i8() {
    test_image_sum_i8("tiled-oversize-gray-i8.tif", ColorType::Gray(8), 1214996);
}

#[test]
fn test_tiled_cmyk_i8() {
    test_image_sum_i8("tiled-cmyk-i8.tif", ColorType::CMYK(8), 1759101);
}

#[test]
fn test_tiled_incremental() {
    let file = "tiled-rgb-u8.tif";
    let expected_type = ColorType::RGB(8);
    let sums = [
        188760, 195639, 108148, 81986, 665088, 366140, 705317, 423366, 172033, 324455, 244102,
        81853, 181258, 247971, 129486, 55600, 565625, 422102, 730888, 379271, 232142, 292549,
        244045, 86866, 188141, 115036, 150785, 84389, 353170, 459325, 719619, 329594, 278663,
        220474, 243048, 113563, 189152, 109684, 179391, 122188, 279651, 622093, 724682, 302459,
        268428, 204499, 224255, 124674, 170668, 121868, 192768, 183367, 378029, 585651, 657712,
        296790, 241444, 197083, 198429, 134869, 182318, 86034, 203655, 182338, 297255, 601284,
        633813, 242531, 228578, 206441, 193552, 125412, 181527, 165439, 202531, 159538, 268388,
        565790, 611382, 272967, 236497, 215154, 158881, 90806, 106114, 182342, 191824, 186138,
        215174, 393193, 701228, 198866, 227944, 193830, 166330, 49008, 55719, 122820, 197316,
        161969, 203152, 170986, 624427, 188605, 186187, 111064, 115192, 39538, 48626, 163929,
        144682, 135796, 194141, 154198, 584125, 180255, 153524, 121433, 132641, 35743, 47798,
        152343, 162874, 167664, 160175, 133038, 659882, 138339, 166470, 124173, 118929, 51317,
        45267, 155776, 161331, 161006, 130052, 137618, 337291, 106481, 161999, 127343, 87724,
        59540, 63907, 155677, 140668, 141523, 108061, 168657, 186482, 98599, 147614, 139963, 90444,
        56602, 92547, 125644, 134212, 126569, 144153, 179800, 174516, 133969, 129399, 117681,
        83305, 55075, 110737, 115108, 128572, 128911, 130922, 179986, 143288, 145884, 155856,
        96683, 94057, 56238, 79649, 71651, 70182, 75010, 77009, 98855, 78979, 74341, 83482, 53403,
        59842, 30305,
    ];

    let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), expected_type);

    let tiles = decoder.tile_count().unwrap();
    assert_eq!(tiles as usize, sums.len());

    for tile in 0..tiles {
        match decoder.read_tile().unwrap() {
            DecodingResult::U8(res) => {
                let sum: u64 = res.into_iter().map(<u64>::from).sum();
                assert_eq!(sum, sums[tile as usize]);
            }
            _ => panic!("Wrong bit depth"),
        }
    }
}

#[test]
fn test_div_zero() {
    use tiff::tags;
    use tiff::{TiffError, TiffFormatError};

    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 40, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 39, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 158, 0, 0, 251, 67, 1, 3, 0,
        1, 0, 0, 0, 40, 0, 0, 0, 66, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 178, 178, 178, 178,
        178, 178, 178,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        TiffError::FormatError(TiffFormatError::InvalidTagValueType(tags::Tag::TileWidth)) => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn test_too_many_value_bytes() {
    let image = [
        73, 73, 43, 0, 8, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 8, 0, 0, 0,
        23, 0, 12, 0, 0, 65, 4, 0, 1, 6, 0, 0, 1, 16, 0, 1, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0,
        0, 0, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 59, 73, 84, 186, 202, 83, 240, 66, 1, 53, 22, 56, 47,
        0, 0, 0, 0, 0, 0, 1, 222, 4, 0, 58, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 4, 0, 0, 100, 0,
        0, 89, 89, 89, 89, 89, 89, 89, 89, 96, 1, 20, 89, 89, 89, 89, 18,
    ];

    let error = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();

    match error {
        tiff::TiffError::LimitsExceeded => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn test_bad_strip_count() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 100, 0, 0, 0, 1, 1, 4, 0, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0, 0, 0, 0, 0, 246, 16, 0, 0, 22, 1, 4, 0, 1,
        0, 0, 0, 40, 0, 251, 255, 23, 1, 4, 0, 1, 0, 0, 0, 48, 178, 178, 178, 178, 178, 178, 178,
        178, 178, 178,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        tiff::TiffError::IntSizeError => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn test_too_big_buffer_size() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 99, 255, 255, 254, 1, 1, 4, 0, 1,
        0, 0, 0, 158, 0, 0, 251, 3, 1, 3, 255, 254, 255, 255, 0, 1, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0,
        0, 0, 0, 0, 0, 17, 1, 4, 0, 9, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0, 2, 0, 0, 0, 63, 0, 0, 0,
        22, 1, 4, 0, 1, 0, 0, 0, 44, 0, 0, 0, 23, 1, 4, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 1, 0, 178,
        178,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        tiff::TiffError::LimitsExceeded => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn test_stripped_image_overflow() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 100, 0, 0, 148, 1, 1, 4, 0, 1, 0,
        0, 0, 158, 0, 0, 251, 3, 1, 3, 255, 254, 255, 255, 0, 1, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0,
        0, 0, 0, 0, 17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0, 2, 0, 0, 0, 63, 0, 0, 0, 22,
        1, 4, 0, 1, 0, 0, 0, 44, 0, 248, 255, 23, 1, 4, 0, 1, 0, 0, 0, 178, 178, 178, 0, 1, 178,
        178, 178,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        tiff::TiffError::LimitsExceeded => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn oom() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 40, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 7, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 2, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 3, 77, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 3, 128, 0, 0, 22, 1, 4, 0, 1,
        0, 0, 0, 40, 0, 0, 0, 23, 1, 4, 0, 1, 0, 0, 0, 178, 48, 178, 178, 178, 178, 162, 178,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        tiff::TiffError::LimitsExceeded => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn oom_2() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 40, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 5, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 3, 128, 0, 0, 22, 1, 4, 0, 1,
        0, 0, 0, 40, 0, 0, 0, 23, 1, 4, 0, 1, 0, 0, 0, 48, 178, 178, 178, 0, 1, 0, 13, 13,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        tiff::TiffError::LimitsExceeded => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn invalid_jpeg_tag() {
    use tiff::tags;
    use tiff::{TiffError, TiffFormatError};

    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 15, 0, 0, 254, 44, 1, 0, 0, 0, 0, 0, 32, 0, 0, 0, 1, 4, 0, 1, 0,
        0, 0, 0, 1, 0, 0, 91, 1, 1, 0, 0, 0, 0, 0, 242, 4, 0, 0, 0, 22, 0, 56, 77, 0, 77, 1, 0, 0,
        73, 42, 0, 1, 4, 0, 1, 0, 0, 0, 4, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 158, 0, 0, 251, 3, 1,
        3, 0, 1, 0, 0, 0, 7, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 2, 0, 0, 0, 17, 1, 4, 0, 1, 0, 0, 0,
        0, 0, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 4, 61, 1, 18, 0, 1, 0, 0, 0, 202, 0, 0, 0, 17,
        1, 100, 0, 129, 0, 0, 0, 0, 0, 0, 0, 232, 254, 252, 255, 254, 255, 255, 255, 1, 29, 0, 0,
        22, 1, 3, 0, 1, 0, 0, 0, 16, 0, 0, 0, 23, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 123, 73, 254, 0,
        73,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        TiffError::FormatError(TiffFormatError::InvalidTagValueType(tags::Tag::JPEGTables)) => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn invalid_jpeg_tag_2() {
    use tiff::tags;
    use tiff::{TiffError, TiffFormatError};

    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 16, 0, 254, 0, 4, 0, 1, 0, 0, 0, 0, 0, 0, 242, 0, 1, 4, 0, 1, 0,
        0, 0, 0, 129, 16, 0, 1, 1, 4, 0, 1, 0, 0, 0, 214, 0, 0, 248, 253, 1, 3, 0, 1, 0, 0, 0, 64,
        0, 0, 0, 3, 1, 3, 0, 1, 0, 0, 0, 7, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 64, 14, 1, 0,
        2, 0, 0, 148, 0, 206, 0, 0, 0, 17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
        1, 0, 0, 0, 22, 1, 4, 0, 17, 0, 0, 201, 1, 0, 0, 0, 23, 1, 2, 0, 20, 0, 0, 0, 194, 0, 0, 0,
        91, 1, 7, 0, 5, 0, 0, 0, 64, 0, 0, 0, 237, 254, 65, 255, 255, 255, 255, 255, 1, 0, 0, 0,
        22, 1, 4, 0, 1, 0, 0, 0, 42, 0, 0, 0, 23, 1, 255, 255, 255, 255, 255, 36, 36, 0, 0, 0, 0,
        0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 36, 73, 73, 0, 42, 36, 36, 36, 36, 0, 0, 8, 0,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let err = decoder.read_image().unwrap_err();

    match err {
        TiffError::FormatError(TiffFormatError::InvalidTagValueType(tags::Tag::JPEGTables)) => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}

#[test]
fn multiple_images_strip_height() {
    use tiff::TiffError;

    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 2, 0, 0, 0, 61, 1, 9, 0, 46, 22,
        128, 0, 0, 0, 0, 1, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0, 17, 1, 4, 0, 27, 0, 0, 0, 0, 0, 0,
        0, 1, 1, 3, 0, 1, 0, 0, 0, 17, 1, 0, 231, 22, 1, 1, 0, 1, 0, 0, 0, 130, 0, 0, 0, 23, 1, 4,
        0, 14, 0, 0, 0, 0, 0, 0, 0, 133, 133, 133, 77, 77, 77, 0, 0, 22, 128, 0, 255, 255, 255,
        255, 255,
    ];

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap();

    let _ = decoder.read_image().unwrap();
    let err = decoder.read_image().unwrap_err();

    match err {
        TiffError::IntSizeError => {}
        unexpected => panic!("Unexpected error {}", unexpected),
    }
}
