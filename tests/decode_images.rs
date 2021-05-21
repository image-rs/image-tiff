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
fn test_predictor_3_rgb_f32() {
    test_image_sum_f32("predictor-3-rgb-f32.tif", ColorType::RGB(32), 54004.33);
}

#[test]
fn test_predictor_3_gray_f32() {
    test_image_sum_f32("predictor-3-gray-f32.tif", ColorType::Gray(32), 20008.275);
}
