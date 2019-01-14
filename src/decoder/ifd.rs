//! Function for reading TIFF tags

use std::io::{self, Read, Seek};
use std::collections::{HashMap};

use super::stream::{ByteOrder, SmartReader, EndianReader};
use ::{TiffError, TiffFormatError, TiffUnsupportedError, TiffResult};

use self::Value::{Unsigned, List, Rational, Ascii};

macro_rules! tags {
    {$(
        $tag:ident
        $val:expr;
    )*} => {

        /// TIFF tag
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        pub enum Tag {
            $($tag,)*
            Unknown(u16)
        }
        impl Tag {
            pub fn from_u16(n: u16) -> Tag {
                $(if n == $val { Tag::$tag } else)* {
                    Tag::Unknown(n)
                }
            }
        }
    }
}

// Note: These tags appear in the order they are mentioned in the TIFF reference
tags!{
    // Baseline tags:
    Artist 315;
    // grayscale images PhotometricInterpretation 1 or 3
    BitsPerSample 258;
    CellLength 265; // TODO add support
    CellWidth 264; // TODO add support
    // palette-color images (PhotometricInterpretation 3)
    ColorMap 320; // TODO add support
    Compression 259; // TODO add support for 2 and 32773
    Copyright 33_432;
    DateTime 306;
    ExtraSamples 338; // TODO add support
    FillOrder 266; // TODO add support
    FreeByteCounts 289; // TODO add support
    FreeOffsets 288; // TODO add support
    GrayResponseCurve 291; // TODO add support
    GrayResponseUnit 290; // TODO add support
    HostComputer 316;
    ImageDescription 270;
    ImageLength 257;
    ImageWidth 256;
    Make 271;
    MaxSampleValue 281; // TODO add support
    MinSampleValue 280; // TODO add support
    Model 272;
    NewSubfileType 254; // TODO add support
    Orientation 274; // TODO add support
    PhotometricInterpretation 262;
    PlanarConfiguration 284;
    ResolutionUnit 296; // TODO add support
    RowsPerStrip 278;
    SamplesPerPixel 277;
    Software 305;
    StripByteCounts 279;
    StripOffsets 273;
    SubfileType 255; // TODO add support
    Threshholding 263; // TODO add support
    XResolution 282;
    YResolution 283;
    // Advanced tags
    Predictor 317;
}

#[derive(Clone, Copy, Debug, FromPrimitive)]
pub enum Type {
    BYTE = 1,
    ASCII = 2,
    SHORT = 3,
    LONG = 4,
    RATIONAL = 5,
}


#[allow(unused_qualifications)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    //Signed(i32),
    Unsigned(u32),
    List(Vec<Value>),
    Rational(u32, u32),
    Ascii(String)
}

impl Value {
    pub fn into_u32(self) -> TiffResult<u32> {
        match self {
            Unsigned(val) => Ok(val),
            val => Err(TiffError::FormatError(TiffFormatError::UnsignedIntegerExpected(val))),
        }
    }
    pub fn into_u32_vec(self) -> TiffResult<Vec<u32>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    new_vec.push(v.into_u32()?)
                }
                Ok(new_vec)
            },
            Unsigned(val) => Ok(vec![val]),
            Rational(numerator, denominator) => Ok(vec![numerator, denominator]),
            Ascii(val) => Ok(val.chars().map(|x| x as u32).collect())
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    type_: Type,
    count: u32,
    offset: [u8; 4],
}

impl ::std::fmt::Debug for Entry {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.write_str(&format!("Entry {{ type_: {:?}, count: {:?}, offset: {:?} }}",
            self.type_,
            self.count,
            &self.offset
        ))
    }
}

impl Entry {
    pub fn new(type_: Type, count: u32, offset: [u8; 4]) -> Entry {
        Entry { type_, count, offset }
    }

    /// Returns a mem_reader for the offset/value field
    fn r(&self, byte_order: ByteOrder) -> SmartReader<io::Cursor<Vec<u8>>> {
        SmartReader::wrap(
            io::Cursor::new(self.offset.to_vec()),
            byte_order
        )
    }

    pub fn val<R: Read + Seek>(&self, decoder: &mut super::Decoder<R>)
    -> TiffResult<Value> {
        let bo = decoder.byte_order();
        match (self.type_, self.count) {
            // TODO check if this could give wrong results
            // at a different endianess of file/computer.
            (Type::BYTE, 1) => Ok(Unsigned(u32::from(self.offset[0]))),
            (Type::SHORT, 1) => Ok(Unsigned(u32::from(self.r(bo).read_u16()?))),
            (Type::SHORT, 2) => {
                let mut r = self.r(bo);
                Ok(List(vec![
                    Unsigned(u32::from(r.read_u16()?)),
                    Unsigned(u32::from(r.read_u16()?))
                ]))
            },
            (Type::SHORT, n) => {
                let mut v = Vec::with_capacity(n as usize);
                try!(decoder.goto_offset(try!(self.r(bo).read_u32())));
                for _ in 0 .. n {
                    v.push(Unsigned(u32::from(decoder.read_short()?)))
                }
                Ok(List(v))
            },
            (Type::LONG, 1) => Ok(Unsigned(try!(self.r(bo).read_u32()))),
            (Type::LONG, n) => {
                let mut v = Vec::with_capacity(n as usize);
                try!(decoder.goto_offset(try!(self.r(bo).read_u32())));
                for _ in 0 .. n {
                    v.push(Unsigned(try!(decoder.read_long())))
                }
                Ok(List(v))
            }
            (Type::RATIONAL, 1) => {
                try!(decoder.goto_offset(try!(self.r(bo).read_u32())));
                let numerator = try!(decoder.read_long());
                let denominator = try!(decoder.read_long());
                Ok(Rational(numerator, denominator))
            },
            (Type::RATIONAL, n) => {
                let mut v = Vec::with_capacity(n as usize);
                try!(decoder.goto_offset(try!(self.r(bo).read_u32())));
                for _ in 0 .. n {
                    let numerator = try!(decoder.read_long());
                    let denominator = try!(decoder.read_long());
                    v.push(Rational(numerator, denominator))
                }
                Ok(List(v))
            },
            (Type::ASCII, n) => {
                try!(decoder.goto_offset(try!(self.r(bo).read_u32())));
                let string = try!(decoder.read_string(n as usize));
                Ok(Ascii(string))
            }
            _ => Err(TiffError::UnsupportedError(TiffUnsupportedError::UnsupportedDataType))
        }
    }
}

/// Type representing an Image File Directory
pub type Directory = HashMap<Tag, Entry>;
