//! Function for reading TIFF tags

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::io::{self, Read, Seek};
use std::mem;

use super::stream::{ByteOrder, EndianReader, SmartReader};
use tags::{Tag, Type};
use {TiffError, TiffFormatError, TiffResult, TiffUnsupportedError};

use self::Value::{
    Ascii, Double, Float, List, Rational, RationalBig, SRational, SRationalBig, Signed, SignedBig,
    Unsigned, UnsignedBig,
};

#[allow(unused_qualifications)]
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Signed(i32),
    SignedBig(i64),
    Unsigned(u32),
    UnsignedBig(u64),
    Float(f32),
    Double(f64),
    List(Vec<Value>),
    Rational(u32, u32),
    RationalBig(u64, u64),
    SRational(i32, i32),
    SRationalBig(i64, i64),
    Ascii(String),
    #[doc(hidden)] // Do not match against this.
    __NonExhaustive,
}

impl Value {
    pub fn into_u32(self) -> TiffResult<u32> {
        match self {
            Unsigned(val) => Ok(val),
            UnsignedBig(val) => Ok(u32::try_from(val)?),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i32(self) -> TiffResult<i32> {
        match self {
            Signed(val) => Ok(val),
            SignedBig(val) => Ok(i32::try_from(val)?),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_u64(self) -> TiffResult<u64> {
        match self {
            Unsigned(val) => Ok(val.into()),
            UnsignedBig(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i64(self) -> TiffResult<i64> {
        match self {
            Signed(val) => Ok(val.into()),
            SignedBig(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_f32(self) -> TiffResult<f32> {
        match self {
            Float(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_f64(self) -> TiffResult<f64> {
        match self {
            Double(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
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
            }
            Unsigned(val) => Ok(vec![val]),
            UnsignedBig(val) => Ok(vec![u32::try_from(val)?]),
            Rational(numerator, denominator) => Ok(vec![numerator, denominator]),
            RationalBig(numerator, denominator) => {
                Ok(vec![u32::try_from(numerator)?, u32::try_from(denominator)?])
            }
            Ascii(val) => Ok(val.chars().map(u32::from).collect()),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i32_vec(self) -> TiffResult<Vec<i32>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    match v {
                        SRational(numerator, denominator) => {
                            new_vec.push(numerator);
                            new_vec.push(denominator);
                        }
                        SRationalBig(numerator, denominator) => {
                            new_vec.push(i32::try_from(numerator)?);
                            new_vec.push(i32::try_from(denominator)?);
                        }
                        _ => new_vec.push(v.into_i32()?),
                    }
                }
                Ok(new_vec)
            }
            Signed(val) => Ok(vec![val]),
            SignedBig(val) => Ok(vec![i32::try_from(val)?]),
            SRational(numerator, denominator) => Ok(vec![numerator, denominator]),
            SRationalBig(numerator, denominator) => {
                Ok(vec![i32::try_from(numerator)?, i32::try_from(denominator)?])
            }
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_f32_vec(self) -> TiffResult<Vec<f32>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    new_vec.push(v.into_f32()?)
                }
                Ok(new_vec)
            }
            Float(val) => Ok(vec![val]),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_f64_vec(self) -> TiffResult<Vec<f64>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    new_vec.push(v.into_f64()?)
                }
                Ok(new_vec)
            }
            Double(val) => Ok(vec![val]),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_u64_vec(self) -> TiffResult<Vec<u64>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    new_vec.push(v.into_u64()?)
                }
                Ok(new_vec)
            }
            Unsigned(val) => Ok(vec![val.into()]),
            UnsignedBig(val) => Ok(vec![val]),
            Rational(numerator, denominator) => Ok(vec![numerator.into(), denominator.into()]),
            RationalBig(numerator, denominator) => Ok(vec![numerator, denominator]),
            Ascii(val) => Ok(val.chars().map(u32::from).map(u64::from).collect()),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i64_vec(self) -> TiffResult<Vec<i64>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    match v {
                        SRational(numerator, denominator) => {
                            new_vec.push(numerator.into());
                            new_vec.push(denominator.into());
                        }
                        SRationalBig(numerator, denominator) => {
                            new_vec.push(numerator);
                            new_vec.push(denominator);
                        }
                        _ => new_vec.push(v.into_i64()?),
                    }
                }
                Ok(new_vec)
            }
            Signed(val) => Ok(vec![val.into()]),
            SignedBig(val) => Ok(vec![val]),
            SRational(numerator, denominator) => Ok(vec![numerator.into(), denominator.into()]),
            SRationalBig(numerator, denominator) => Ok(vec![numerator, denominator]),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    type_: Type,
    count: u64,
    offset: [u8; 8],
}

impl ::std::fmt::Debug for Entry {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.write_str(&format!(
            "Entry {{ type_: {:?}, count: {:?}, offset: {:?} }}",
            self.type_, self.count, &self.offset
        ))
    }
}

impl Entry {
    pub fn new(type_: Type, count: u32, offset: [u8; 4]) -> Entry {
        let mut offset = offset.to_vec();
        offset.append(&mut vec![0; 4]);
        Entry::new_u64(type_, count.into(), offset[..].try_into().unwrap())
    }

    pub fn new_u64(type_: Type, count: u64, offset: [u8; 8]) -> Entry {
        Entry {
            type_,
            count,
            offset,
        }
    }

    /// Returns a mem_reader for the offset/value field
    fn r(&self, byte_order: ByteOrder) -> SmartReader<io::Cursor<Vec<u8>>> {
        SmartReader::wrap(io::Cursor::new(self.offset.to_vec()), byte_order)
    }

    pub fn val<R: Read + Seek>(
        &self,
        limits: &super::Limits,
        decoder: &mut super::Decoder<R>,
    ) -> TiffResult<Value> {
        let bo = decoder.byte_order();
        let bigtiff_value = if decoder.bigtiff {
            match (self.type_, self.count) {
                (Type::BYTE, 5) => Some(offset_to_bytes(5, self)?),
                (Type::BYTE, 6) => Some(offset_to_bytes(6, self)?),
                (Type::BYTE, 7) => Some(offset_to_bytes(7, self)?),
                (Type::BYTE, 8) => Some(offset_to_bytes(8, self)?),
                (Type::SBYTE, 5) => Some(offset_to_sbytes(5, self)?),
                (Type::SBYTE, 6) => Some(offset_to_sbytes(6, self)?),
                (Type::SBYTE, 7) => Some(offset_to_sbytes(7, self)?),
                (Type::SBYTE, 8) => Some(offset_to_sbytes(8, self)?),
                (Type::SHORT, 3) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Unsigned(u32::from(r.read_u16()?)),
                        Unsigned(u32::from(r.read_u16()?)),
                        Unsigned(u32::from(r.read_u16()?)),
                    ]))
                }
                (Type::SSHORT, 3) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Signed(i32::from(r.read_i16()?)),
                        Signed(i32::from(r.read_i16()?)),
                        Signed(i32::from(r.read_i16()?)),
                    ]))
                }
                (Type::SHORT, 4) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Unsigned(u32::from(r.read_u16()?)),
                        Unsigned(u32::from(r.read_u16()?)),
                        Unsigned(u32::from(r.read_u16()?)),
                        Unsigned(u32::from(r.read_u16()?)),
                    ]))
                }
                (Type::SSHORT, 4) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Signed(i32::from(r.read_i16()?)),
                        Signed(i32::from(r.read_i16()?)),
                        Signed(i32::from(r.read_i16()?)),
                        Signed(i32::from(r.read_i16()?)),
                    ]))
                }
                (Type::LONG, 2) => {
                    let mut r = self.r(bo);
                    Some(List(vec![Unsigned(r.read_u32()?), Unsigned(r.read_u32()?)]))
                }
                (Type::SLONG, 2) => {
                    let mut r = self.r(bo);
                    Some(List(vec![Signed(r.read_i32()?), Signed(r.read_i32()?)]))
                }
                (Type::RATIONAL, 1) => {
                    let mut r = self.r(bo);
                    let numerator = r.read_u32()?;
                    let denominator = r.read_u32()?;
                    Some(Rational(numerator, denominator))
                }
                (Type::SRATIONAL, 1) => {
                    let mut r = self.r(bo);
                    let numerator = r.read_i32()?;
                    let denominator = r.read_i32()?;
                    Some(SRational(numerator, denominator))
                }
                _ => None,
            }
        } else {
            None
        };

        if let Some(v) = bigtiff_value {
            Ok(v)
        } else {
            match (self.type_, self.count) {
                // TODO check if this could give wrong results
                // at a different endianess of file/computer.
                (Type::BYTE, 1) => Ok(Unsigned(u32::from(self.offset[0]))),
                (Type::BYTE, 2) => offset_to_bytes(2, self),
                (Type::BYTE, 3) => offset_to_bytes(3, self),
                (Type::BYTE, 4) => offset_to_bytes(4, self),
                (Type::BYTE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(UnsignedBig(u64::from(decoder.read_byte()?)))
                }),
                (Type::SBYTE, 1) => Ok(Signed(i32::from(self.offset[0] as i8))),
                (Type::SBYTE, 2) => offset_to_sbytes(2, self),
                (Type::SBYTE, 3) => offset_to_sbytes(3, self),
                (Type::SBYTE, 4) => offset_to_sbytes(4, self),
                (Type::SBYTE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(SignedBig(i64::from(decoder.read_byte()? as i8)))
                }),
                (Type::SHORT, 1) => Ok(Unsigned(u32::from(self.r(bo).read_u16()?))),
                (Type::SSHORT, 1) => Ok(Signed(i32::from(self.r(bo).read_i16()?))),
                (Type::SHORT, 2) => {
                    let mut r = self.r(bo);
                    Ok(List(vec![
                        Unsigned(u32::from(r.read_u16()?)),
                        Unsigned(u32::from(r.read_u16()?)),
                    ]))
                }
                (Type::SSHORT, 2) => {
                    let mut r = self.r(bo);
                    Ok(List(vec![
                        Signed(i32::from(r.read_i16()?)),
                        Signed(i32::from(r.read_i16()?)),
                    ]))
                }
                (Type::SHORT, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(UnsignedBig(u64::from(decoder.read_short()?)))
                }),
                (Type::SSHORT, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(SignedBig(i64::from(decoder.read_sshort()?)))
                }),
                (Type::LONG, 1) => Ok(Unsigned(self.r(bo).read_u32()?)),
                (Type::SLONG, 1) => Ok(Signed(self.r(bo).read_i32()?)),
                (Type::LONG, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Unsigned(decoder.read_long()?))
                }),
                (Type::SLONG, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Signed(decoder.read_slong()?))
                }),
                (Type::FLOAT, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Float(decoder.read_float()?))
                }),
                (Type::DOUBLE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Double(decoder.read_double()?))
                }),
                (Type::RATIONAL, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Rational(decoder.read_long()?, decoder.read_long()?))
                }),
                (Type::SRATIONAL, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(SRational(decoder.read_slong()?, decoder.read_slong()?))
                }),
                (Type::LONG8, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(UnsignedBig(decoder.read_long8()?))
                }),
                (Type::ASCII, n) => {
                    let n = usize::try_from(n)?;
                    if n > limits.decoding_buffer_size {
                        return Err(TiffError::LimitsExceeded);
                    }

                    if (n <= 4 && !decoder.bigtiff) || (n <= 8 && decoder.bigtiff) {
                        let mut buf = vec![0; n];
                        self.r(bo).read_exact(&mut buf)?;
                        let v = String::from_utf8(buf)?;
                        let v = v.trim_matches(char::from(0));
                        Ok(Ascii(v.into()))
                    } else {
                        if decoder.bigtiff {
                            decoder.goto_offset_u64(self.r(bo).read_u64()?)?
                        } else {
                            decoder.goto_offset(self.r(bo).read_u32()?)?
                        }
                        let string = decoder.read_string(n)?;
                        Ok(Ascii(string))
                    }
                }
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedDataType,
                )),
            }
        }
    }

    #[inline]
    fn decode_offset<R, F>(
        &self,
        value_count: u64,
        bo: ByteOrder,
        limits: &super::Limits,
        decoder: &mut super::Decoder<R>,
        decode_fn: F,
    ) -> TiffResult<Value>
    where
        R: Read + Seek,
        F: Fn(&mut super::Decoder<R>) -> TiffResult<Value>,
    {
        let value_count = usize::try_from(value_count)?;
        if value_count > limits.decoding_buffer_size / mem::size_of::<Value>() {
            return Err(TiffError::LimitsExceeded);
        }

        let mut v = Vec::with_capacity(value_count);
        if decoder.bigtiff {
            decoder.goto_offset_u64(self.r(bo).read_u64()?)?
        } else {
            decoder.goto_offset(self.r(bo).read_u32()?)?
        }
        for _ in 0..value_count {
            v.push(decode_fn(decoder)?)
        }
        Ok(List(v))
    }
}

/// Extracts a list of BYTE tags stored in an offset
#[inline]
fn offset_to_bytes(n: usize, entry: &Entry) -> TiffResult<Value> {
    Ok(List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Unsigned(u32::from(e)))
            .collect(),
    ))
}

/// Extracts a list of SBYTE tags stored in an offset
#[inline]
fn offset_to_sbytes(n: usize, entry: &Entry) -> TiffResult<Value> {
    Ok(List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Signed(i32::from(e as i8)))
            .collect(),
    ))
}

/// Type representing an Image File Directory
pub type Directory = HashMap<Tag, Entry>;
