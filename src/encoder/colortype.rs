use super::Value;
use tags::PhotometricInterpretation;

pub struct ColorType {
    pub value: Value,
    pub tiff_value: PhotometricInterpretation,
    pub bit_per_sample: &'static [u16],
}

pub const GRAY8: ColorType = ColorType {
    value: Value::U8(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[8],
};
pub const GRAYI8: ColorType = ColorType {
    value: Value::I8(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[8],
};
pub const GRAY16: ColorType = ColorType {
    value: Value::U16(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[16],
};
pub const GRAYI16: ColorType = ColorType {
    value: Value::I16(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[16],
};
pub const GRAY32: ColorType = ColorType {
    value: Value::U32(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[32],
};
pub const GRAYI32: ColorType = ColorType {
    value: Value::I32(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[32],
};
pub const GRAYF32: ColorType = ColorType {
    value: Value::F32(0_f32),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[32],
};
pub const GRAY64: ColorType = ColorType {
    value: Value::U64(0),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[64],
};
pub const GRAYF64: ColorType = ColorType {
    value: Value::F64(0_f64),
    tiff_value: PhotometricInterpretation::BlackIsZero,
    bit_per_sample: &[64],
};

pub const RGB8: ColorType = ColorType {
    value: Value::U8(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[8, 8, 8],
};
pub const RGB16: ColorType = ColorType {
    value: Value::U16(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[16, 16, 16],
};
pub const RGB32: ColorType = ColorType {
    value: Value::U32(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[32, 32, 32],
};
pub const RGB64: ColorType = ColorType {
    value: Value::U64(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[64, 64, 64],
};

pub const RGBA8: ColorType = ColorType {
    value: Value::U8(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[8, 8, 8, 8],
};
pub const RGBA16: ColorType = ColorType {
    value: Value::U16(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[16, 16, 16, 16],
};
pub const RGBA32: ColorType = ColorType {
    value: Value::U32(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[32, 32, 32, 32],
};
pub const RGBA64: ColorType = ColorType {
    value: Value::U64(0),
    tiff_value: PhotometricInterpretation::RGB,
    bit_per_sample: &[64, 64, 64, 64],
};

pub const CMYK8: ColorType = ColorType {
    value: Value::U8(0),
    tiff_value: PhotometricInterpretation::CMYK,
    bit_per_sample: &[8, 8, 8, 8],
};
pub const CMYK16: ColorType = ColorType {
    value: Value::U16(0),
    tiff_value: PhotometricInterpretation::CMYK,
    bit_per_sample: &[16, 16, 16, 16],
};
pub const CMYK32: ColorType = ColorType {
    value: Value::U32(0),
    tiff_value: PhotometricInterpretation::CMYK,
    bit_per_sample: &[32, 32, 32, 32],
};
pub const CMYK64: ColorType = ColorType {
    value: Value::U64(0),
    tiff_value: PhotometricInterpretation::CMYK,
    bit_per_sample: &[64, 64, 64, 64],
};
