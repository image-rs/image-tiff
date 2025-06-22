use crate::{
    TiffError, TiffFormatError, TiffKind, TiffResult,
    decoder::{
        fix_endianness,
        stream::{ByteOrder, EndianReader},
    },
    ifd::{
        BufferedEntry,
        Value::{
            self, Ascii, Byte, Double, Float, Ifd, IfdBig, List, Rational, SRational, Short,
            Signed, SignedBig, SignedByte, SignedShort, Unsigned, UnsignedBig,
        },
    },
    tags::Type,
};
use std::{
    io::{self, Read, Seek},
    mem,
};

#[derive(Clone)]
pub struct DecodedEntry<K: TiffKind> {
    type_: Type,
    count: K::OffsetType,
    offset: Vec<u8>,
}

impl<K: TiffKind> ::std::fmt::Debug for DecodedEntry<K> {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        fmt.write_str(&format!(
            "Entry {{ type_: {:?}, count: {:?}, offset: {:?} }}",
            self.type_, self.count, &self.offset
        ))
    }
}

impl<K: TiffKind> DecodedEntry<K> {
    pub fn new(type_: Type, count: K::OffsetType, offset: &[u8]) -> Self {
        Self {
            type_,
            count,
            offset: offset.to_vec(),
        }
    }

    /// Returns a mem_reader for the offset/value field
    fn r(&self, byte_order: ByteOrder) -> EndianReader<io::Cursor<Vec<u8>>> {
        EndianReader::new(io::Cursor::new(self.offset.clone()), byte_order)
    }

    pub fn val<R: Read + Seek>(
        &self,
        limits: &super::Limits,
        reader: &mut EndianReader<R>,
    ) -> TiffResult<Value> {
        let count: usize = self
            .count
            .clone()
            .try_into()
            .map_err(|_| TiffError::LimitsExceeded)?;

        // Case 1: there are no values so we can return immediately.
        if count == 0 {
            return Ok(List(Vec::new()));
        }

        let bo = reader.byte_order;

        let tag_size = match self.type_ {
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

        let value_bytes = match count.checked_mul(tag_size) {
            Some(n) => n,
            None => {
                return Err(TiffError::LimitsExceeded);
            }
        };

        // Case 2: there is one value.
        if count == 1 {
            // 2a: the value is 5-8 bytes and we're in BigTiff mode.
            if K::is_big() && value_bytes > 4 && value_bytes <= 8 {
                return Ok(match self.type_ {
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
                });
            }

            // 2b: the value is at most 4 bytes or doesn't fit in the offset field.
            return Ok(match self.type_ {
                Type::BYTE => Unsigned(u32::from(self.offset[0])),
                Type::SBYTE => Signed(i32::from(self.offset[0] as i8)),
                Type::UNDEFINED => Byte(self.offset[0]),
                Type::SHORT => Short(u16::from(self.r(bo).read_u16()?)),
                Type::SSHORT => SignedShort(i16::from(self.r(bo).read_i16()?)),
                Type::LONG => Unsigned(self.r(bo).read_u32()?),
                Type::SLONG => Signed(self.r(bo).read_i32()?),
                Type::FLOAT => Float(self.r(bo).read_f32()?),
                Type::ASCII => {
                    if self.offset[0] == 0 {
                        Ascii("".to_string())
                    } else {
                        return Err(TiffError::FormatError(TiffFormatError::InvalidTag));
                    }
                }
                Type::LONG8 => {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?;
                    UnsignedBig(reader.read_u64()?)
                }
                Type::SLONG8 => {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?;
                    SignedBig(reader.read_i64()?)
                }
                Type::DOUBLE => {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?;
                    Double(reader.read_f64()?)
                }
                Type::RATIONAL => {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?;
                    Rational(reader.read_u32()?, reader.read_u32()?)
                }
                Type::SRATIONAL => {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?;
                    SRational(reader.read_i32()?, reader.read_i32()?)
                }
                Type::IFD => Ifd(self.r(bo).read_u32()?),
                Type::IFD8 => {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?;
                    IfdBig(reader.read_u64()?)
                }
            });
        }

        // Case 3: There is more than one value, but it fits in the offset field.
        if value_bytes <= 4 || K::is_big() && value_bytes <= 8 {
            match self.type_ {
                Type::BYTE => return offset_to_bytes(count, self),
                Type::SBYTE => return offset_to_sbytes(count, self),
                Type::ASCII => {
                    let mut buf = vec![0; count];
                    buf.copy_from_slice(&self.offset[..count]);
                    if buf.is_ascii() && buf.ends_with(&[0]) {
                        let v = std::str::from_utf8(&buf)?;
                        let v = v.trim_matches(char::from(0));
                        return Ok(Ascii(v.into()));
                    } else {
                        return Err(TiffError::FormatError(TiffFormatError::InvalidTag));
                    }
                }
                Type::UNDEFINED => {
                    return Ok(List(
                        self.offset[0..count].iter().map(|&b| Byte(b)).collect(),
                    ));
                }
                Type::SHORT => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..count {
                        v.push(Short(r.read_u16()?));
                    }
                    return Ok(List(v));
                }
                Type::SSHORT => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..count {
                        v.push(Signed(i32::from(r.read_i16()?)));
                    }
                    return Ok(List(v));
                }
                Type::LONG => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..count {
                        v.push(Unsigned(r.read_u32()?));
                    }
                    return Ok(List(v));
                }
                Type::SLONG => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..count {
                        v.push(Signed(r.read_i32()?));
                    }
                    return Ok(List(v));
                }
                Type::FLOAT => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..count {
                        v.push(Float(r.read_f32()?));
                    }
                    return Ok(List(v));
                }
                Type::IFD => {
                    let mut r = self.r(bo);
                    let mut v = Vec::new();
                    for _ in 0..count {
                        v.push(Ifd(r.read_u32()?));
                    }
                    return Ok(List(v));
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

        // Case 4: there is more than one value, and it doesn't fit in the offset field.
        match self.type_ {
            // TODO check if this could give wrong results
            // at a different endianess of file/computer.
            Type::BYTE => self.decode_offset(count, bo, limits, reader, |reader| {
                let mut buf = [0; 1];
                reader.inner().read_exact(&mut buf)?;
                Ok(Byte(u8::from(buf[0])))
            }),
            Type::SBYTE => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(SignedByte(reader.read_i8()?))
            }),
            Type::SHORT => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Short(reader.read_u16()?))
            }),
            Type::SSHORT => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(SignedShort(reader.read_i16()?))
            }),
            Type::LONG => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Unsigned(reader.read_u32()?))
            }),
            Type::SLONG => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Signed(reader.read_i32()?))
            }),
            Type::FLOAT => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Float(reader.read_f32()?))
            }),
            Type::DOUBLE => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Double(reader.read_f64()?))
            }),
            Type::RATIONAL => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Rational(reader.read_u32()?, reader.read_u32()?))
            }),
            Type::SRATIONAL => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(SRational(reader.read_i32()?, reader.read_i32()?))
            }),
            Type::LONG8 => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(UnsignedBig(reader.read_u64()?))
            }),
            Type::SLONG8 => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(SignedBig(reader.read_i64()?))
            }),
            Type::IFD => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(Ifd(reader.read_u32()?))
            }),
            Type::IFD8 => self.decode_offset(count, bo, limits, reader, |reader| {
                Ok(IfdBig(reader.read_u64()?))
            }),
            Type::UNDEFINED => self.decode_offset(count, bo, limits, reader, |reader| {
                let mut buf = [0; 1];
                reader.inner().read_exact(&mut buf)?;
                Ok(Byte(buf[0]))
            }),
            Type::ASCII => {
                if count > limits.decoding_buffer_size {
                    return Err(TiffError::LimitsExceeded);
                }

                if K::is_big() {
                    reader.goto_offset(self.r(bo).read_u64()?)?
                } else {
                    reader.goto_offset(self.r(bo).read_u32()?.into())?
                }

                let mut out = vec![0; count];
                reader.inner().read_exact(&mut out)?;
                // Strings may be null-terminated, so we trim anything downstream of the null byte
                if let Some(first) = out.iter().position(|&b| b == 0) {
                    out.truncate(first);
                }
                Ok(Ascii(String::from_utf8(out)?))
            }
        }
    }

    #[inline]
    fn decode_offset<R, F>(
        &self,
        value_count: usize,
        bo: ByteOrder,
        limits: &super::Limits,
        reader: &mut EndianReader<R>,
        decode_fn: F,
    ) -> TiffResult<Value>
    where
        R: Read + Seek,
        F: Fn(&mut EndianReader<R>) -> TiffResult<Value>,
    {
        //let value_count = usize::try_from(value_count)?;
        if value_count > limits.decoding_buffer_size / mem::size_of::<Value>() {
            return Err(TiffError::LimitsExceeded);
        }

        let mut v = Vec::with_capacity(value_count);

        let offset = if K::is_big() {
            self.r(bo).read_u64()?
        } else {
            self.r(bo).read_u32()?.into()
        };
        reader.goto_offset(offset)?;

        for _ in 0..value_count {
            v.push(decode_fn(reader)?)
        }
        Ok(List(v))
    }

    /// retrieve entry with data read into a buffer (to cache it for writing)
    pub fn as_buffered<R: Read + Seek>(
        &self,
        reader: &mut EndianReader<R>,
    ) -> TiffResult<BufferedEntry> {
        let count: usize = self
            .count
            .clone()
            .try_into()
            .map_err(|_| TiffError::LimitsExceeded)?;

        // establish byte order
        let bo = reader.byte_order;
        let native_bo;
        #[cfg(target_endian = "little")]
        {
            native_bo = ByteOrder::LittleEndian;
        }
        #[cfg(not(target_endian = "little"))]
        {
            native_bo = ByteOrder::BigEndian;
        }

        // establish size
        let tag_size = match self.type_ {
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

        let value_bytes = match count.checked_mul(tag_size) {
            Some(n) => n,
            None => {
                return Err(TiffError::LimitsExceeded);
            }
        };

        let mut buf = vec![0; value_bytes];
        if value_bytes <= 4 || (K::is_big() && value_bytes <= 8) {
            // read values that fit within the IFD entry
            self.r(bo).inner().read_exact(&mut buf)?;
        } else {
            // values that use a pointer
            // read pointed data
            if K::is_big() {
                reader.goto_offset(self.r(bo).read_u64()?)?;
            } else {
                reader.goto_offset(self.r(bo).read_u32()?.into())?;
            }
            reader.inner().read_exact(&mut buf)?;
        }

        // convert buffer to native byte order
        if native_bo != bo {
            let bit_size = match self.type_ {
                Type::RATIONAL | Type::SRATIONAL => 32,
                _ => 8 * tag_size as u8,
            };

            fix_endianness(&mut buf, bo, bit_size);
        }

        Ok(BufferedEntry {
            type_: self.type_,
            count: self.count.clone().into(),
            data: buf,
        })
    }
}

/// Extracts a list of BYTE tags stored in an offset
#[inline]
fn offset_to_bytes<K: TiffKind>(n: usize, entry: &DecodedEntry<K>) -> TiffResult<Value> {
    Ok(List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Unsigned(u32::from(e)))
            .collect(),
    ))
}

/// Extracts a list of SBYTE tags stored in an offset
#[inline]
fn offset_to_sbytes<K: TiffKind>(n: usize, entry: &DecodedEntry<K>) -> TiffResult<Value> {
    Ok(List(
        entry.offset[0..n]
            .iter()
            .map(|&e| Signed(i32::from(e as i8)))
            .collect(),
    ))
}
