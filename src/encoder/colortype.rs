use tags::PhotometricInterpretation;

/// Trait for different colortypes that can be encoded.
pub trait ColorType {
    /// The type of each sample of this colortype
    type Inner: super::TiffValue;
    /// The value of the tiff tag `PhotometricInterpretation`
    const TIFF_VALUE: PhotometricInterpretation;
    /// The value of the tiff tag `BitsPerSample`
    const BITS_PER_SAMPLE: &'static [u16];
}

pub struct Gray8;
impl ColorType for Gray8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[8];
}

pub struct Gray16;
impl ColorType for Gray16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[16];
}

pub struct Gray32;
impl ColorType for Gray32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[32];
}

pub struct Gray64;
impl ColorType for Gray64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[64];
}

pub struct RGB8;
impl ColorType for RGB8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8];
}

pub struct RGB16;
impl ColorType for RGB16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[16, 16, 16];
}

pub struct RGB32;
impl ColorType for RGB32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32];
}

pub struct RGB64;
impl ColorType for RGB64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64];
}

pub struct RGBA8;
impl ColorType for RGBA8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8, 8];
}

pub struct RGBA16;
impl ColorType for RGBA16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[16, 16, 16, 16];
}

pub struct RGBA32;
impl ColorType for RGBA32 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32, 32];
}

pub struct RGBA64;
impl ColorType for RGBA64 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64, 64];
}

pub struct CMYK8;
impl ColorType for CMYK8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8, 8];
}

pub struct CMYK16;
impl ColorType for CMYK16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[16, 16, 16, 16];
}

pub struct CMYK32;
impl ColorType for CMYK32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32, 32];
}

pub struct CMYK64;
impl ColorType for CMYK64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64, 64];
}
