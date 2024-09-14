//! Abstractions over TIFF tags

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::mem::size_of;

use crate::encoder::TiffValue;
use crate::tags::{Tag, Type};
use crate::{TiffError, TiffFormatError, TiffResult};

use self::Value::{
    Ascii, Byte, Double, Float, Ifd, IfdBig, List, Rational, RationalBig, SRational, SRationalBig,
    Short, Signed, SignedBig, SignedByte, SignedShort, Unsigned, UnsignedBig,
};

use itertools::Itertools;

#[allow(unused_qualifications)]
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    Byte(u8),
    Short(u16),
    SignedByte(i8),
    SignedShort(i16),
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
    Ifd(u32),
    IfdBig(u64),
    Undefined(u8),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Value::Byte(e) => write!(f, "{e}"),
            Value::Short(e) => write!(f, "{e}"),
            Value::SignedByte(e) => write!(f, "{e}"),
            Value::SignedShort(e) => write!(f, "{e}"),
            Value::Signed(e) => write!(f, "{e}"),
            Value::SignedBig(e) => write!(f, "{e}"),
            Value::Unsigned(e) => write!(f, "{e}"),
            Value::UnsignedBig(e) => write!(f, "{e}"),
            Value::Float(e) => write!(f, "{e}"),
            Value::Double(e) => write!(f, "{e}"),
            Value::Rational(e1, e2) => {
                let a_mul = (*e1 as u128) * 1000;
                let b = *e2 as u128;
                let div = a_mul / b;

                let frac = div % 1000;
                let rest = div / 1000;

                if frac != 0 {
                    write!(f, "{rest}.{frac:#03}")
                } else {
                    write!(f, "{rest}")
                }
            }
            Value::RationalBig(e1, e2) => write!(f, "{e1}/{e2}"),
            Value::SRational(e1, e2) => write!(f, "{e1}/{e2}"),
            Value::SRationalBig(e1, e2) => write!(f, "{e1}/{e2}"),
            Value::Ascii(e) => write!(f, "{e}"),
            Value::Ifd(e) => write!(f, "IFD offset: {e}"),
            Value::IfdBig(e) => write!(f, "IFD offset: {e}"),
            Value::Undefined(e) => write!(f, "{e}"),
            Value::List(_) => todo!(),
        }
    }
}

impl Value {
    pub fn into_u8(self) -> TiffResult<u8> {
        match self {
            Byte(val) => Ok(val),
            val => Err(TiffError::FormatError(TiffFormatError::ByteExpected(val))),
        }
    }

    pub fn into_i8(self) -> TiffResult<i8> {
        match self {
            SignedByte(val) => Ok(val),
            val => Err(TiffError::FormatError(TiffFormatError::SignedByteExpected(
                val,
            ))),
        }
    }

    pub fn into_u16(self) -> TiffResult<u16> {
        match self {
            Short(val) => Ok(val),
            Unsigned(val) => Ok(u16::try_from(val)?),
            UnsignedBig(val) => Ok(u16::try_from(val)?),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i16(self) -> TiffResult<i16> {
        match self {
            SignedByte(val) => Ok(val.into()),
            SignedShort(val) => Ok(val),
            Signed(val) => Ok(i16::try_from(val)?),
            SignedBig(val) => Ok(i16::try_from(val)?),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedShortExpected(val),
            )),
        }
    }

    pub fn into_u32(self) -> TiffResult<u32> {
        match self {
            Short(val) => Ok(val.into()),
            Unsigned(val) => Ok(val),
            UnsignedBig(val) => Ok(u32::try_from(val)?),
            Ifd(val) => Ok(val),
            IfdBig(val) => Ok(u32::try_from(val)?),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i32(self) -> TiffResult<i32> {
        match self {
            SignedByte(val) => Ok(val.into()),
            SignedShort(val) => Ok(val.into()),
            Signed(val) => Ok(val),
            SignedBig(val) => Ok(i32::try_from(val)?),
            val => Err(TiffError::FormatError(
                TiffFormatError::SignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_u64(self) -> TiffResult<u64> {
        match self {
            Short(val) => Ok(val.into()),
            Unsigned(val) => Ok(val.into()),
            UnsignedBig(val) => Ok(val),
            Ifd(val) => Ok(val.into()),
            IfdBig(val) => Ok(val),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_i64(self) -> TiffResult<i64> {
        match self {
            SignedByte(val) => Ok(val.into()),
            SignedShort(val) => Ok(val.into()),
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

    pub fn into_string(self) -> TiffResult<String> {
        match self {
            Ascii(val) => Ok(val),
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
            Ifd(val) => Ok(vec![val]),
            IfdBig(val) => Ok(vec![u32::try_from(val)?]),
            Ascii(val) => Ok(val.chars().map(u32::from).collect()),
            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_u8_vec(self) -> TiffResult<Vec<u8>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    new_vec.push(v.into_u8()?)
                }
                Ok(new_vec)
            }
            Byte(val) => Ok(vec![val]),

            val => Err(TiffError::FormatError(
                TiffFormatError::UnsignedIntegerExpected(val),
            )),
        }
    }

    pub fn into_u16_vec(self) -> TiffResult<Vec<u16>> {
        match self {
            List(vec) => {
                let mut new_vec = Vec::with_capacity(vec.len());
                for v in vec {
                    new_vec.push(v.into_u16()?)
                }
                Ok(new_vec)
            }
            Short(val) => Ok(vec![val]),
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
            SignedByte(val) => Ok(vec![val.into()]),
            SignedShort(val) => Ok(vec![val.into()]),
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
            Ifd(val) => Ok(vec![val.into()]),
            IfdBig(val) => Ok(vec![val]),
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
            SignedByte(val) => Ok(vec![val.into()]),
            SignedShort(val) => Ok(vec![val.into()]),
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

/// Entry with buffered instead of read data
#[derive(Clone, Debug)]
pub struct BufferedEntry {
    pub type_: Type,
    pub count: u64,
    pub data: Vec<u8>,
}

/// Implement TiffValue to allow writing this data with encoder
impl TiffValue for BufferedEntry {
    const BYTE_LEN: u8 = 1;

    fn is_type(&self) -> Type {
        self.type_
    }

    fn count(&self) -> usize {
        self.count.clone().try_into().unwrap()
    }

    fn bytes(&self) -> usize {
        let tag_size: u32 = match self.type_ {
            Type::BYTE | Type::SBYTE | Type::ASCII | Type::UNDEFINED => 1,
            Type::SHORT | Type::SSHORT => 2,
            Type::LONG | Type::SLONG | Type::FLOAT | Type::IFD => 4,
            Type::LONG8
            | Type::SLONG8
            | Type::DOUBLE
            | Type::RATIONAL
            | Type::SRATIONAL
            | Type::IFD8 => 8,
        };

        match self.count.checked_mul(tag_size.into()) {
            Some(n) => n.try_into().unwrap(),
            None => 0usize,
        }
    }

    fn data(&self) -> Cow<[u8]> {
        Cow::Borrowed(&self.data)
    }
}

macro_rules! step_through {
    ($vec:expr, $type:ty) => {
        (0..$vec.len()).step_by(size_of::<$type>()).map(|i| {
            Ok(<$type>::from_ne_bytes(
                $vec[i..i + size_of::<$type>()].try_into()?,
            ))
        })
    };
}

macro_rules! cast {
    ($be:expr, $type:ty, $value:expr) => {{
        assert!($be.data.len() as u64 == size_of::<$type>() as u64 * $be.count);
        step_through!($be.data, $type)
            .collect::<Result<Vec<$type>, Box<dyn std::error::Error>>>()?
            .into_iter()
            .map($value)
            .collect()
    }};

    ($be:expr, $type:ty, $second:ty, $value:expr) => {{
        assert!($be.data.len() as u64 == size_of::<$type>() as u64 * $be.count * 2);
        step_through!($be.data, $type)
            .collect::<Result<Vec<$type>, Box<dyn std::error::Error>>>()?
            .into_iter()
            .tuples::<($type, $type)>()
            .map(|(n, d)| $value(n, d))
            .collect()
    }};
}

pub fn process(be: BufferedEntry) -> Result<ProcessedEntry, Box<dyn std::error::Error>> {
    let contents: Vec<Value> = match be.type_ {
        Type::BYTE => be.data.into_iter().map(Value::Byte).collect(),
        Type::SBYTE => be
            .data
            .into_iter()
            .map(|b| i8::from_be_bytes([b; 1]))
            .map(Value::SignedByte)
            .collect(),
        Type::SHORT => cast!(be, u16, Value::Short),
        Type::LONG => cast!(be, u32, Value::Unsigned),
        Type::SLONG8 => cast!(be, u64, Value::UnsignedBig),
        Type::SSHORT => cast!(be, i16, Value::SignedShort),
        Type::SLONG => cast!(be, i32, Value::Signed),
        Type::LONG8 => cast!(be, i64, Value::SignedBig),
        Type::FLOAT => cast!(be, f32, Value::Float),
        Type::DOUBLE => cast!(be, f64, Value::Double),
        Type::RATIONAL => cast!(be, u32, u32, Value::Rational),
        Type::SRATIONAL => cast!(be, i32, i32, Value::SRational),
        Type::IFD => cast!(be, u32, Value::Ifd),
        Type::IFD8 => cast!(be, u64, Value::IfdBig),
        Type::UNDEFINED => be.data.into_iter().map(Value::Undefined).collect(),
        Type::ASCII => {
            vec![Value::Ascii(String::from_utf8(be.data)?)]
        }
    };

    Ok(ProcessedEntry(contents))
}

/// Entry with buffered instead of read data
#[derive(Clone, Debug)]
pub struct ProcessedEntry(Vec<Value>);

impl std::fmt::Display for ProcessedEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0.iter().map(|v| format!("{v}")).join(", "))
    }
}

/// Type representing an Image File Directory
#[derive(Debug, Clone)]
pub struct ImageFileDirectory<T: Ord, E>(BTreeMap<T, E>);
pub type Directory<E> = ImageFileDirectory<Tag, E>;

impl<T, E> Default for ImageFileDirectory<T, E>
where
    T: Ord,
{
    fn default() -> Self {
        ImageFileDirectory(BTreeMap::new())
    }
}

impl<T, E> ImageFileDirectory<T, E>
where
    T: Ord,
{
    pub fn new() -> Self {
        ImageFileDirectory(BTreeMap::new())
    }

    pub fn insert(&mut self, tag: T, entry: E) -> Option<E> {
        self.0.insert(tag, entry)
    }

    pub fn into_iter(self) -> std::collections::btree_map::IntoIter<T, E> {
        self.0.into_iter()
    }

    pub fn contains_key(&self, tag: &T) -> bool {
        self.0.contains_key(&tag)
    }

    pub fn get(&self, tag: &T) -> Option<&E> {
        self.0.get(&tag)
    }

    pub fn get_mut(&mut self, tag: &T) -> Option<&mut E> {
        self.0.get_mut(&tag)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<T, E> {
        self.0.iter()
    }

    pub fn values_mut(&mut self) -> std::collections::btree_map::ValuesMut<T, E> {
        self.0.values_mut()
    }
}

impl<T, E> FromIterator<(T, E)> for ImageFileDirectory<T, E>
where
    T: Ord,
{
    fn from_iter<I: IntoIterator<Item = (T, E)>>(iter: I) -> Self {
        ImageFileDirectory(iter.into_iter().collect())
    }
}
