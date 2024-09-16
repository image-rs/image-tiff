use crate::decoder_async::stream::AsyncSmartReader;
use crate::tags::Type;
pub use crate::{
    decoder::{
        ifd::Value::{
            self, Ascii, Byte, Double, Float, Ifd, IfdBig, List, Rational, RationalBig, SRational,
            SRationalBig, Short, Signed, SignedBig, SignedByte, SignedShort, Unsigned, UnsignedBig,
        },
        stream::{ByteOrder, EndianReader, SmartReader},
    },
    tags::Tag,
};
use crate::{TiffError, TiffFormatError, TiffResult};

use futures::{future::BoxFuture, io::SeekFrom, AsyncRead, AsyncReadExt, AsyncSeek, FutureExt};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use super::RangeReader;

pub type Directory = HashMap<Tag, Entry>;

/// Extracts a list of BYTE tags stored in an offset
#[inline]
pub fn offset_to_bytes(n: usize, entry: &Entry) -> Value {
    List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Unsigned(u32::from(e)))
            .collect(),
    )
}

/// Extracts a list of SBYTE tags stored in an offset
#[inline]
pub fn offset_to_sbytes(n: usize, entry: &Entry) -> Value {
    List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Signed(i32::from(e as i8)))
            .collect(),
    )
}

#[derive(Clone, Copy)]
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
    /// Construct a new Entry based on the tag
    /// ```
    /// /*
    /// tiff tag:
    ///  |tag|type|count|value|
    ///    |                |
    ///    in parent       offset (actually only really offset if it doesn't fit)
    /// */
    /// ```
    /// note that, `offset=[1,2,3,4] -> [1,2,3,4,0,0,0,0]`
    /// but this is taken care of when requesting val
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
    fn r(&self, byte_order: ByteOrder) -> SmartReader<Cursor<Vec<u8>>> {
        SmartReader::wrap(std::io::Cursor::new(self.offset.to_vec()), byte_order)
    }

    #[inline(always)]
    fn type_size(&self) -> u64 {
        match self.type_ {
            Type::BYTE | Type::SBYTE | Type::ASCII | Type::UNDEFINED => 1,
            Type::SHORT | Type::SSHORT => 2,
            Type::LONG | Type::SLONG | Type::FLOAT | Type::IFD => 4,
            Type::LONG8
            | Type::SLONG8
            | Type::DOUBLE
            | Type::RATIONAL
            | Type::SRATIONAL
            | Type::IFD8 => 8,
        }
    }

    #[inline(always)]
    pub fn count(&self) -> u64 {
        self.count
    }

    #[inline(always)]
    fn num_value_bytes(&self) -> TiffResult<u64> {
        // The number of bytes our value takes up
        match self.count.checked_mul(self.type_size()) {
            Some(n) => Ok(n),
            None => {
                return Err(TiffError::LimitsExceeded);
            }
        }
    }

    /// Get the tags value if it fits in the Value field.
    pub fn maybe_val(&self, bigtiff: bool, byte_order: ByteOrder) -> TiffResult<Option<Value>> {
        // Case 1: there are no values so we can return immediately.
        if self.count == 0 {
            return Ok(Some(List(Vec::new())));
        }

        let bo = byte_order;

        let value_bytes = self.num_value_bytes()?;

        if value_bytes > 8 || (!bigtiff && value_bytes > 4) {
            return Ok(None);
        }

        // Case 2: there is one value.
        if self.count == 1 {
            // 2a: the value is 5-8 bytes and we're in BigTiff mode.
            if bigtiff && value_bytes > 4 && value_bytes <= 8 {
                return Ok(Some(match self.type_ {
                    Type::LONG8 => UnsignedBig(self.r(bo).read_u64()?),
                    Type::SLONG8 => SignedBig(self.r(bo).read_i64()?),
                    Type::DOUBLE => Double(self.r(bo).read_f64()?),
                    Type::RATIONAL => {
                        let mut r = self.r(bo);
                        Rational(r.read_u32()?, r.read_u32()?)
                    }
                    Type::SRATIONAL => {
                        let mut r = self.r(bo);
                        SRational(r.read_i32()?, r.read_i32()?)
                    }
                    Type::IFD8 => IfdBig(self.r(bo).read_u64()?),
                    Type::BYTE
                    | Type::SBYTE
                    | Type::ASCII
                    | Type::UNDEFINED
                    | Type::SHORT
                    | Type::SSHORT
                    | Type::LONG
                    | Type::SLONG
                    | Type::FLOAT
                    | Type::IFD => unreachable!(),
                }));
            }

            // 2b: the value is at most 4 bytes or doesn't fit in the offset field.
            return Ok(match self.type_ {
                Type::BYTE => Some(Unsigned(u32::from(self.offset[0]))),
                Type::SBYTE => Some(Signed(i32::from(self.offset[0] as i8))),
                Type::UNDEFINED => Some(Byte(self.offset[0])),
                Type::SHORT => Some(Unsigned(u32::from(self.r(bo).read_u16()?))),
                Type::SSHORT => Some(Signed(i32::from(self.r(bo).read_i16()?))),
                Type::LONG => Some(Unsigned(self.r(bo).read_u32()?)),
                Type::SLONG => Some(Signed(self.r(bo).read_i32()?)),
                Type::FLOAT => Some(Float(self.r(bo).read_f32()?)),
                Type::ASCII => {
                    if self.offset[0] == 0 {
                        Some(Ascii("".to_string()))
                    } else {
                        return Err(TiffError::FormatError(TiffFormatError::InvalidTag));
                    }
                }
                Type::IFD => Some(Ifd(self.r(bo).read_u32()?)),
                _ => unreachable!("This should have been caught earlier"),
            });
        }

        // Case 3: There is more than one value, but it fits in the offset field.
        if value_bytes <= 4 || bigtiff && value_bytes <= 8 {
            match self.type_ {
                Type::BYTE => return Ok(Some(offset_to_bytes(self.count as usize, self))),
                Type::SBYTE => return Ok(Some(offset_to_sbytes(self.count as usize, self))),
                Type::ASCII => {
                    let mut buf = vec![0; self.count as usize];
                    self.r(bo).read_exact(&mut buf)?;
                    if buf.is_ascii() && buf.ends_with(&[0]) {
                        let v = std::str::from_utf8(&buf)?;
                        let v = v.trim_matches(char::from(0));
                        return Ok(Some(Ascii(v.into())));
                    } else {
                        return Err(TiffError::FormatError(TiffFormatError::InvalidTag));
                    }
                }
                Type::UNDEFINED => {
                    return Ok(Some(List(
                        self.offset[0..self.count as usize]
                            .iter()
                            .map(|&b| Byte(b))
                            .collect(),
                    )));
                }
                Type::SHORT => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..self.count {
                        v.push(Short(r.read_u16()?));
                    }
                    return Ok(Some(List(v)));
                }
                Type::SSHORT => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..self.count {
                        v.push(Signed(i32::from(r.read_i16()?)));
                    }
                    return Ok(Some(List(v)));
                }
                Type::LONG => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..self.count {
                        v.push(Unsigned(r.read_u32()?));
                    }
                    return Ok(Some(List(v)));
                }
                Type::SLONG => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..self.count {
                        v.push(Signed(r.read_i32()?));
                    }
                    return Ok(Some(List(v)));
                }
                Type::FLOAT => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..self.count {
                        v.push(Float(r.read_f32()?));
                    }
                    return Ok(Some(List(v)));
                }
                Type::IFD => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..self.count {
                        v.push(Ifd(r.read_u32()?));
                    }
                    return Ok(Some(List(v)));
                }
                Type::LONG8
                | Type::SLONG8
                | Type::RATIONAL
                | Type::SRATIONAL
                | Type::DOUBLE
                | Type::IFD8 => {
                    unreachable!()
                }
            }
        }

        // case 4: multiple and it doesn't fit
        unreachable!()
    }

    /// Gets the nth value of a List type, such as byte offsets and lenghts
    /// If it is not an offset,
    pub async fn nth_val<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        n: u64,
        limits: &super::Limits,
        bigtiff: bool,
        reader: &mut AsyncSmartReader<R>,
    ) -> TiffResult<Value> {
        if self.num_value_bytes()? <= 4 || (self.num_value_bytes()? <= 8 && bigtiff) {
            // return Err(TiffError::UsageError("Should not call nth val on a value that is in the Value tag field"));
            panic!("Should not call this function if bla")
        }
        if n > self.count {
            return Err(TiffError::LimitsExceeded);
        }
        let bo = reader.byte_order();
        let offset = if bigtiff {
            self.r(bo).read_u64()?
        } else {
            self.r(bo).read_u32()?.into()
        };
        reader.goto_offset(offset + n * self.type_size()).await?;
        Ok(UnsignedBig(reader.read_u64().await?))
    }

    /// get the tags value, if it doesn't fit, it will read the pointer.  
    /// may cause additional reading into the file
    pub async fn val<R: AsyncRead + AsyncSeek + Unpin + Send>(
        &self,
        limits: &super::Limits,
        bigtiff: bool,
        reader: &mut AsyncSmartReader<R>,
    ) -> TiffResult<Value> {
        let bo = reader.byte_order();

        // The number of bytes our value takes up
        let value_bytes = self.num_value_bytes()?;
        let offset = if bigtiff {
            self.r(bo).read_u64()?
        } else {
            self.r(bo).read_u32()?.into()
        };
        // case 1: the value fits in the value field
        if let Some(maybe_val) = self.maybe_val(bigtiff, bo)? {
            return Ok(maybe_val);
        }

        // Case 2: there is one value. This only
        if self.count == 1 {
            // 2b: the value is at most 4 bytes or doesn't fit in the offset field.
            return Ok(match self.type_ {
                Type::LONG8 => {
                    reader.goto_offset(offset).await?;
                    UnsignedBig(reader.read_u64().await?)
                }
                Type::SLONG8 => {
                    reader.goto_offset(offset).await?;
                    SignedBig(reader.read_i64().await?)
                }
                Type::DOUBLE => {
                    reader.goto_offset(offset).await?;
                    Double(reader.read_f64().await?)
                }
                Type::RATIONAL => {
                    reader.goto_offset(offset).await?;
                    Rational(reader.read_u32().await?, reader.read_u32().await?)
                }
                Type::SRATIONAL => {
                    reader.goto_offset(offset).await?;
                    SRational(reader.read_i32().await?, reader.read_i32().await?)
                }
                Type::IFD8 => {
                    reader.goto_offset(offset).await?;
                    IfdBig(reader.read_u64().await?)
                }
                _ => unreachable!(),
            });
        }

        // TODO: find out if this is actually faster (I think it is...)
        // initialize the buffer with all tag data inside it
        // let buf = reader.read_range(offset, offset + self.count * self.type_size()).await?;
        // let synr = SmartReader::wrap(Cursor::new(buf), bo);

        // Case 4: there is more than one value, and it doesn't fit in the offset field.
        // Async help found here: https://users.rust-lang.org/t/function-that-takes-an-async-closure/61663
        match self.type_ {
            // TODO check if this could give wrong results
            // at a different endianess of file/computer.
            Type::BYTE => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move {
                        let mut buf = [0; 1];
                        reader.read_exact(&mut buf).await?;
                        Ok(UnsignedBig(u64::from(buf[0])))
                    }
                    .boxed()
                })
                .await
            }
            Type::SBYTE => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(SignedBig(i64::from(reader.read_i8().await?))) }.boxed()
                })
                .await
            }
            Type::SHORT => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(UnsignedBig(u64::from(reader.read_u16().await?))) }.boxed()
                })
                .await
            }
            Type::SSHORT => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(SignedBig(i64::from(reader.read_i16().await?))) }.boxed()
                })
                .await
            }
            Type::LONG => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(Unsigned(reader.read_u32().await?)) }.boxed()
                })
                .await
            }
            Type::SLONG => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(Signed(reader.read_i32().await?)) }.boxed()
                })
                .await
            }
            Type::FLOAT => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(Float(reader.read_f32().await?)) }.boxed()
                })
                .await
            }
            Type::DOUBLE => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(Double(reader.read_f64().await?)) }.boxed()
                })
                .await
            }
            Type::RATIONAL => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(Rational(reader.read_u32().await?, reader.read_u32().await?)) }
                        .boxed()
                })
                .await
            }
            Type::SRATIONAL => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move {
                        Ok(SRational(
                            reader.read_i32().await?,
                            reader.read_i32().await?,
                        ))
                    }
                    .boxed()
                })
                .await
            }
            Type::LONG8 => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(UnsignedBig(reader.read_u64().await?)) }.boxed()
                })
                .await
            }
            Type::SLONG8 => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(SignedBig(reader.read_i64().await?)) }.boxed()
                })
                .await
            }
            Type::IFD => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(Ifd(reader.read_u32().await?)) }.boxed()
                })
                .await
            }
            Type::IFD8 => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move { Ok(IfdBig(reader.read_u64().await?)) }.boxed()
                })
                .await
            }
            Type::UNDEFINED => {
                self.decode_offset(self.count, bo, bigtiff, limits, reader, |reader| {
                    async move {
                        let mut buf = [0; 1];
                        reader.read_exact(&mut buf).await?;
                        Ok(Byte(buf[0]))
                    }
                    .boxed()
                })
                .await
            }
            Type::ASCII => {
                let n = usize::try_from(self.count)?;
                if n > limits.decoding_buffer_size {
                    return Err(TiffError::LimitsExceeded);
                }

                if bigtiff {
                    reader.goto_offset(self.r(bo).read_u64()?).await?
                } else {
                    reader.goto_offset(offset).await?
                }

                let mut out = vec![0; n];
                reader.read_exact(&mut out).await?;
                // Strings may be null-terminated, so we trim anything downstream of the null byte
                if let Some(first) = out.iter().position(|&b| b == 0) {
                    out.truncate(first);
                }
                Ok(Ascii(String::from_utf8(out)?))
            }
        }
    }

    /// Goes to offset and decodes all values there
    /// This is the interesting part where tile offsets are read
    #[inline]
    async fn decode_offset<R, F>(
        &self,
        value_count: u64,
        bo: ByteOrder,
        bigtiff: bool,
        limits: &super::Limits,
        reader: &mut AsyncSmartReader<R>,
        decode_fn: F,
    ) -> TiffResult<Value>
    where
        R: AsyncRead + AsyncSeek + Unpin,
        // F: Fn(&mut AsyncSmartReader<R>) -> TiffResult<Value>,
        F: Fn(&'_ mut AsyncSmartReader<R>) -> BoxFuture<'_, TiffResult<Value>>,
    {
        let value_count = usize::try_from(value_count)?;
        if value_count > limits.decoding_buffer_size / std::mem::size_of::<Value>() {
            return Err(TiffError::LimitsExceeded);
        }

        let mut v = Vec::with_capacity(value_count);

        let offset = if bigtiff {
            self.r(bo).read_u64()?
        } else {
            self.r(bo).read_u32()?.into()
        };
        reader.goto_offset(offset).await?;

        for _ in 0..value_count {
            v.push(decode_fn(reader).await?)
        }
        Ok(List(v))
    }
}
