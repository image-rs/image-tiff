pub trait ColorType {
    type Inner: super::TiffValue;
    const TIFF_VALUE: u8;
    fn bits_per_sample() -> Vec<u16>;
}

pub struct Grey8;
impl ColorType for Grey8 {
    type Inner = u8;
    const TIFF_VALUE: u8 = 1;
    fn bits_per_sample() -> Vec<u16> {
        vec![8]
    }
}

pub struct Grey16;
impl ColorType for Grey16 {
    type Inner = u16;
    const TIFF_VALUE: u8 = 1;
    fn bits_per_sample() -> Vec<u16> {
        vec![16]
    }
}

pub struct RGB8;
impl ColorType for RGB8 {
    type Inner = u8;
    const TIFF_VALUE: u8 = 2;
    fn bits_per_sample() -> Vec<u16> {
        vec![8,8,8]
    }
}

pub struct RGB16;
impl ColorType for RGB16 {
    type Inner = u16;
    const TIFF_VALUE: u8 = 2;
    fn bits_per_sample() -> Vec<u16> {
        vec![16,16,16]
    }
}

pub struct RGBA8;
impl ColorType for RGBA8 {
    type Inner = u8;
    const TIFF_VALUE: u8 = 2;
    fn bits_per_sample() -> Vec<u16> {
        vec![8,8,8,8]
    }
}

pub struct RGBA16;
impl ColorType for RGBA16 {
    type Inner = u16;
    const TIFF_VALUE: u8 = 2;
    fn bits_per_sample() -> Vec<u16> {
        vec![16,16,16,16]
    }
}

pub struct CMYK8;
impl ColorType for CMYK8 {
    type Inner = u8;
    const TIFF_VALUE: u8 = 5;
    fn bits_per_sample() -> Vec<u16> {
        vec![8,8,8,8]
    }
}
