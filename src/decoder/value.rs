use std::io::{self, Cursor, Read, Seek};

use crate::{
    decoder::{ifd::Entry, stream::EndianReader, ValueReader},
    tags::{ByteOrder, IfdPointer, Type},
    TiffResult,
};

pub struct EntryBytesReader<'lt, R> {
    inner: &'lt mut EndianReader<R>,
    variant: Variant,
}

const OFFSET_BUFFER_LEN: usize = 8;

enum Variant {
    Immediate {
        /// An in-value reader if the offset field contains the data. Note that we rotate it to the
        /// back of this buffer, not the front, so that `Cursor` already contains the right end.
        /// We do not let you seek back beyond our start.
        data: Cursor<[u8; OFFSET_BUFFER_LEN]>,
        value_bytes: u8,
    },
    InFile {
        // the length relative to the base so we can seek relative without querying the position.
        value_bytes: u64,
        remainder: u64,
        base: IfdPointer,
    },
}

impl<'lt, R> EntryBytesReader<'lt, R> {
    pub(super) fn from_entry(inner: &'lt mut ValueReader<R>, entry: Entry) -> TiffResult<Self>
    where
        R: Read + Seek,
    {
        let offset: [_; OFFSET_BUFFER_LEN] = *entry.offset_raw();
        let limit = if inner.bigtiff { 8 } else { 4 };
        assert!(limit as usize <= OFFSET_BUFFER_LEN);

        let value_bytes = entry.field_type().value_bytes(entry.count())?;

        if value_bytes <= limit {
            // We move the data to the back of the array whereas it is in the front right now.
            let mut value = offset;
            value.rotate_left(value_bytes as usize);
            let mut cursor = Cursor::new(value);
            cursor.set_position(OFFSET_BUFFER_LEN as u64 - value_bytes);

            Ok(EntryBytesReader {
                inner: &mut inner.reader,
                variant: Variant::Immediate {
                    data: cursor,
                    value_bytes: value_bytes as u8,
                },
            })
        } else {
            // remainder > limit >= 0 so this is an unreachable panic.
            let pointer = if inner.bigtiff {
                let mut offset = offset;

                inner
                    .reader
                    .byte_order
                    .convert(Type::IFD8, &mut offset[..], ByteOrder::native());

                IfdPointer(u64::from_ne_bytes(offset))
            } else {
                let mut offset = *offset[..4].as_array::<4>().unwrap();

                inner
                    .reader
                    .byte_order
                    .convert(Type::IFD, &mut offset[..], ByteOrder::native());

                let offset = u32::from_ne_bytes(offset);
                IfdPointer(u64::from(offset))
            };

            inner.reader.goto_offset(pointer.0)?;

            Ok(EntryBytesReader {
                inner: &mut inner.reader,
                variant: Variant::InFile {
                    value_bytes,
                    remainder: value_bytes,
                    base: pointer,
                },
            })
        }
    }
}

impl<R: Read> Read for EntryBytesReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.variant {
            Variant::Immediate { data, .. } => {
                return data.read(buf);
            }
            Variant::InFile { remainder, .. } => {
                let bound = usize::try_from(*remainder).unwrap_or(usize::MAX);
                let avail = buf.len().min(bound);
                let read = self.inner.inner().read(&mut buf[..avail])?;

                // On a well-behaved `Read`: read <= avail <= bound <=(mathematical) remainder
                let consume = read as u64;
                *remainder -= consume;
                Ok(read)
            }
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match &mut self.variant {
            Variant::Immediate { data, .. } => {
                return data.read_exact(buf);
            }
            Variant::InFile { remainder, .. } => {
                let bound = usize::try_from(*remainder).unwrap_or(usize::MAX);

                // Fail early if we would be sure to over-read.
                if buf.len() > bound {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Tiff value shorter than the requested buffer size",
                    ));
                }

                // since buf.len() <= bound <=(mathematical) remainder
                let consume = buf.len() as u64;
                self.inner.inner().read_exact(buf)?;
                *remainder -= consume;

                Ok(())
            }
        }
    }
}

impl<R: Read + Seek> Seek for EntryBytesReader<'_, R> {
    fn seek(&mut self, offset: io::SeekFrom) -> io::Result<u64> {
        let (value_bytes, remainder) = match &self.variant {
            Variant::Immediate { data, value_bytes } => {
                let remainder = data.get_ref().len() as u64 - data.position();
                (*value_bytes as u64, remainder)
            }
            &Variant::InFile {
                value_bytes,
                remainder,
                base: _,
            } => (value_bytes, remainder),
        };

        // Normalize to an offset from the start
        let offset_from_start = match offset {
            io::SeekFrom::Start(n) => Some(n),
            io::SeekFrom::Current(n) => {
                let front = value_bytes - remainder;
                front.checked_add_signed(n)
            }
            io::SeekFrom::End(n) => value_bytes.checked_add_signed(n),
        };

        let Some((offset, remainder)) = offset_from_start.and_then(|n| {
            let remainder = value_bytes.checked_sub(n)?;
            Some((n, remainder))
        }) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek out of bounds of entry value",
            ));
        };

        match &mut self.variant {
            Variant::Immediate { data, .. } => {
                let start = OFFSET_BUFFER_LEN as u64 - value_bytes;
                data.set_position(start + offset);
                Ok(offset)
            }
            &mut Variant::InFile { base, .. } => {
                // We checked this against overflow on construction. Ignore the offset we get.
                self.inner
                    .inner()
                    .seek(io::SeekFrom::Start(base.0 + offset))?;

                self.variant = Variant::InFile {
                    value_bytes,
                    remainder,
                    base,
                };

                Ok(offset)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{decoder::Decoder, tags::Tag};
    use std::io::{Read as _, Seek as _, SeekFrom};

    #[test]
    fn can_seek_tag_bytes() -> Result<(), crate::error::TiffError> {
        let file = std::fs::File::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/images/int8_rgb.tif"
        ))
        .unwrap();

        let mut decoder = Decoder::open(file).unwrap();
        decoder.next_directory().unwrap();

        let strips = decoder
            .current_ifd()
            .find_entry(Tag::StripByteCounts)
            .expect("some strip byte count is present");

        let mut buffer = crate::tags::ValueBuffer::default();

        decoder
            .current_ifd()
            .find_tag_buf(Tag::StripByteCounts, &mut buffer)
            .expect("some strip byte count is present");

        let mut reader = decoder.read_entry(strips)?;

        let mut data = vec![];
        reader.read_to_end(&mut data)?;
        assert_eq!(buffer.as_bytes(), data);

        reader.seek(SeekFrom::Start(0))?;
        reader.read_to_end(&mut data)?;

        let (first, second) = data.split_at(data.len() / 2);
        assert_eq!(first, second);

        Ok(())
    }
}
