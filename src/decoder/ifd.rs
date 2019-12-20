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
    Signed(i32),
    Unsigned(u32),
    List(Vec<Value>),
    Rational(u32, u32),
    SRational(i32, i32),
    Ascii(String),
    #[doc(hidden)] // Do not match against this.
    __NonExhaustive,
}

impl Value {
    pub fn into_u32(self) -> TiffResult<u32> {
        match self {
            Unsigned(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i32(self) -> TiffResult<i32> {
        match self {
            Signed(val) => Ok(val),
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
            Rational(numerator, denominator) => Ok(vec![numerator, denominator]),
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
                        _ => new_vec.push(v.into_i32()?)
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
    count: u32,
    offset: [u8; 4],
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
        match (self.type_, self.count) {
            // TODO check if this could give wrong results
            // at a different endianess of file/computer.
            (Type::BYTE, 1) => Ok(Unsigned(u32::from(self.offset[0]))),
            (Type::BYTE, 2) => offset_to_bytes(2, self),
            (Type::BYTE, 3) => offset_to_bytes(3, self),
            (Type::BYTE, 4) => offset_to_bytes(4, self),
            (Type::BYTE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(Unsigned(u32::from(decoder.read_byte()?)))
            }),
            (Type::SBYTE, 1) => Ok(Signed(i32::from(self.offset[0] as i8))),
            (Type::SBYTE, 2) => offset_to_sbytes(2, self),
            (Type::SBYTE, 3) => offset_to_sbytes(3, self),
            (Type::SBYTE, 4) => offset_to_sbytes(4, self),
            (Type::SBYTE, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(Signed(i32::from(decoder.read_byte()? as i8)))
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
                Ok(Unsigned(u32::from(decoder.read_short()?)))
            }),
            (Type::SSHORT, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(Signed(i32::from(decoder.read_sshort()?)))
            }),
            (Type::LONG, 1) => Ok(Unsigned(self.r(bo).read_u32()?)),
            (Type::SLONG, 1) => Ok(Signed(self.r(bo).read_i32()?)),
            (Type::LONG, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(Unsigned(decoder.read_long()?))
            }),
            (Type::SLONG, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(Signed(decoder.read_slong()?))
            }),
            (Type::RATIONAL, 1) => {
                decoder.goto_offset(self.r(bo).read_u32()?)?;
                let numerator = decoder.read_long()?;
                let denominator = decoder.read_long()?;
                Ok(Rational(numerator, denominator))
            }
            (Type::SRATIONAL, 1) => {
                decoder.goto_offset(self.r(bo).read_u32()?)?;
                let numerator = decoder.read_slong()?;
                let denominator = decoder.read_slong()?;
                Ok(SRational(numerator, denominator))
            }
            (Type::RATIONAL, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(Rational(
                    decoder.read_long()?,
                    decoder.read_long()?,
                ))
            }),
            (Type::SRATIONAL, n) => self.decode_offset(n, bo, limits, decoder, |decoder| {
                Ok(SRational(
                    decoder.read_slong()?,
                    decoder.read_slong()?,
                ))
            }),
            (Type::ASCII, n) => {
                let n = usize::try_from(n)?;
                if n > limits.decoding_buffer_size {
                    return Err(TiffError::LimitsExceeded);
                }
                decoder.goto_offset(self.r(bo).read_u32()?)?;
                let string = decoder.read_string(n)?;
                Ok(Ascii(string))
            }
            _ => Err(TiffError::UnsupportedError(
                TiffUnsupportedError::UnsupportedDataType,
            )),
        }
    }

    #[inline]
    fn decode_offset<R, F>(&self, value_count: u32, bo: ByteOrder, limits: &super::Limits, decoder: &mut super::Decoder<R>, decode_fn: F) -> TiffResult<Value>
        where
            R: Read + Seek,
            F: Fn(&mut super::Decoder<R>) -> TiffResult<Value>,
    {
        let value_count = usize::try_from(value_count)?;
        if value_count > limits.decoding_buffer_size / mem::size_of::<Value>() {
            return Err(TiffError::LimitsExceeded);
        }

        let mut v = Vec::with_capacity(value_count);
        decoder.goto_offset(self.r(bo).read_u32()?)?;
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
            .collect()
    ))
}

/// Extracts a list of SBYTE tags stored in an offset
#[inline]
fn offset_to_sbytes(n: usize, entry: &Entry) -> TiffResult<Value> {
    Ok(List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Signed(i32::from(e as i8)))
            .collect()
    ))
}

/// Type representing an Image File Directory
pub type Directory = HashMap<Tag, Entry>;
