//! Function for reading TIFF tags

use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::{self, Read, Seek};
use std::mem;

use super::stream::{ByteOrder, EndianReader, SmartReader};
use tags::{Tag, Type};
use {TiffError, TiffFormatError, TiffResult, TiffUnsupportedError};

use self::Value::{Ascii, List, Rational, Unsigned, Signed, SRational};


#[allow(unused_qualifications)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    Signed(i64),
    Unsigned(u64),
    List(Vec<Value>),
    Rational(u64, u64),
    SRational(i64, i64),
    Ascii(String),
    #[doc(hidden)] // Do not match against this.
    __NonExhaustive,
}

impl Value {
    pub fn into_u64(self) -> TiffResult<u64> {
        match self {
            Unsigned(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i64(self) -> TiffResult<i64> {
        match self {
            Signed(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
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
            Unsigned(val) => Ok(vec![val]),
            Rational(numerator, denominator) => Ok(vec![numerator, denominator]),
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
                            new_vec.push(numerator);
                            new_vec.push(denominator);
                        }
                        _ => new_vec.push(v.into_i64()?),
                    }
                }
                Ok(new_vec)
            }
            Signed(val) => Ok(vec![val]),
            SRational(numerator, denominator) => Ok(vec![numerator, denominator]),
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
    pub fn new(type_: Type, count: u64, offset: [u8; 8]) -> Entry {
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
        let bigtiff_value = if decoder.bigtiff && 4 < self.count && self.count <= 8 {
            match (self.type_, self.count) {
                (Type::BYTE, n) => Some(offset_to_bytes(n as usize, self)?),
                (Type::SBYTE, n) => Some(offset_to_sbytes(n as usize, self)?),
                (Type::SHORT, 3) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Unsigned(u64::from(r.read_u16()?)),
                        Unsigned(u64::from(r.read_u16()?)),
                        Unsigned(u64::from(r.read_u16()?)),
                    ]))
                }
                (Type::SSHORT, 3) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Signed(i64::from(r.read_i16()?)),
                        Signed(i64::from(r.read_i16()?)),
                        Signed(i64::from(r.read_i16()?)),
                    ]))
                }
                (Type::SHORT, 4) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Unsigned(u64::from(r.read_u16()?)),
                        Unsigned(u64::from(r.read_u16()?)),
                        Unsigned(u64::from(r.read_u16()?)),
                        Unsigned(u64::from(r.read_u16()?)),
                    ]))
                }
                (Type::SSHORT, 4) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Signed(i64::from(r.read_i16()?)),
                        Signed(i64::from(r.read_i16()?)),
                        Signed(i64::from(r.read_i16()?)),
                        Signed(i64::from(r.read_i16()?)),
                    ]))
                }
                (Type::LONG, 2) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Unsigned(r.read_u32()?.into()),
                        Unsigned(r.read_u32()?.into()),
                    ]))
                }
                (Type::SLONG, 2) => {
                    let mut r = self.r(bo);
                    Some(List(vec![
                        Signed(r.read_i32()?.into()),
                        Signed(r.read_i32()?.into()),
                    ]))
                }
                (Type::RATIONAL, 1) => {
                    let mut r = self.r(bo);
                    let numerator = r.read_u32()?.into();
                    let denominator = r.read_u32()?.into();
                    Some(Rational(numerator, denominator))
                }
                (Type::SRATIONAL, 1) => {
                    let mut r = self.r(bo);
                    let numerator = r.read_i32()?.into();
                    let denominator = r.read_i32()?.into();
                    Some(SRational(numerator, denominator))
                }
                _ => None
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
                (Type::BYTE, 1) => Ok(Unsigned(u64::from(self.offset[0]))),
                (Type::BYTE, 2) => offset_to_bytes(2, self),
                (Type::BYTE, 3) => offset_to_bytes(3, self),
                (Type::BYTE, 4) => offset_to_bytes(4, self),
                (Type::BYTE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Unsigned(u64::from(decoder.read_byte()?)))
                }),
                (Type::SBYTE, 1) => Ok(Signed(i64::from(self.offset[0] as i8))),
                (Type::SBYTE, 2) => offset_to_sbytes(2, self),
                (Type::SBYTE, 3) => offset_to_sbytes(3, self),
                (Type::SBYTE, 4) => offset_to_sbytes(4, self),
                (Type::SBYTE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Signed(i64::from(decoder.read_byte()? as i8)))
                }),
                (Type::SHORT, 1) => Ok(Unsigned(u64::from(self.r(bo).read_u16()?))),
                (Type::SSHORT, 1) => Ok(Signed(i64::from(self.r(bo).read_i16()?))),
                (Type::SHORT, 2) => {
                    let mut r = self.r(bo);
                    Ok(List(vec![
                        Unsigned(u64::from(r.read_u16()?)),
                        Unsigned(u64::from(r.read_u16()?)),
                    ]))
                }
                (Type::SSHORT, 2) => {
                    let mut r = self.r(bo);
                    Ok(List(vec![
                        Signed(i64::from(r.read_i16()?)),
                        Signed(i64::from(r.read_i16()?)),
                    ]))
                }
                (Type::SHORT, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Unsigned(u64::from(decoder.read_short()?)))
                }),
                (Type::SSHORT, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Signed(i64::from(decoder.read_sshort()?)))
                }),
                (Type::LONG, 1) => Ok(Unsigned(self.r(bo).read_u32()?.into())),
                (Type::SLONG, 1) => Ok(Signed(self.r(bo).read_i32()?.into())),
                (Type::LONG, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Unsigned(decoder.read_long()?.into()))
                }),
                (Type::SLONG, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Signed(decoder.read_slong()?.into()))
                }),
                (Type::RATIONAL, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(Rational(decoder.read_long()?.into(), decoder.read_long()?.into()))
                }),
                (Type::SRATIONAL, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                    Ok(SRational(decoder.read_slong()?.into(), decoder.read_slong()?.into()))
                }),
                (Type::ASCII, n) => {
                    println!("ASCIII COUNT {}", n);
                    let n = usize::try_from(n)?;
                    if n > limits.decoding_buffer_size {
                        return Err(TiffError::LimitsExceeded);
                    }
                    let offset = if decoder.bigtiff {
                        self.r(bo).read_u64()?
                    } else {
                        self.r(bo).read_u32()?.into()
                    };
                    decoder.goto_offset(offset)?;
                    let string = decoder.read_string(n)?;
                    Ok(Ascii(string))
                }
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedDataType,
                )),
            }
        }
    }

    #[inline]
    fn decode_offset<R, F>(&self, value_count: u64, bo: ByteOrder, limits: &super::Limits, decoder: &mut super::Decoder<R>, decode_fn: F) -> TiffResult<Value>
        where
            R: Read + Seek,
            F: Fn(&mut super::Decoder<R>) -> TiffResult<Value>,
    {
        let value_count = usize::try_from(value_count)?;
        if value_count > limits.decoding_buffer_size / mem::size_of::<Value>() {
            return Err(TiffError::LimitsExceeded);
        }

        let mut v = Vec::with_capacity(value_count);
        let offset = if decoder.bigtiff {
            self.r(bo).read_u64()?
        } else {
            self.r(bo).read_u32()?.into()
        };
        decoder.goto_offset(offset)?;
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
            .map(|&e| Unsigned(u64::from(e)))
            .collect()
    ))
}

/// Extracts a list of SBYTE tags stored in an offset
#[inline]
fn offset_to_sbytes(n: usize, entry: &Entry) -> TiffResult<Value> {
    Ok(List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Signed(i64::from(e as i8)))
            .collect()
    ))
}

/// Type representing an Image File Directory
pub type Directory = HashMap<Tag, Entry>;
