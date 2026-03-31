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
        base: u64,
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
                let mut offset = *offset.first_chunk::<4>().unwrap();

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
                    base: pointer.0,
                },
            })
        }
    }
}

impl<R: Read> Read for EntryBytesReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.variant {
            Variant::Immediate { data, .. } => data.read(buf),
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
            Variant::Immediate { data, .. } => data.read_exact(buf),
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
                    .seek(io::SeekFrom::Start(base + offset))?;

                self.variant = Variant::InFile {
                    value_bytes,
                    remainder,
                    base,
                };

                Ok(offset)
            }
        }
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(match &self.variant {
            Variant::Immediate { data, value_bytes } => {
                // Recall the value is at the end of the buffer.
                let from_end = OFFSET_BUFFER_LEN as u64 - data.position();
                u64::from(*value_bytes) - from_end
            }
            // Note: we might think about using the inner `stream_position` here. However that
            // exposes something interesting: If the internal stream is modified concurrently from
            // another instance (e.g. a shared `File`) then we may lose track of it anyways.
            // Especially the `Read` implementation is not bounded on `Seek` so it could not
            // acquire the position. And it should not since that is just a different form of race
            // where we risk the position being modified by a different thread just as well.
            //
            // So in the interest of least surprise we just assume that we can track the position
            // accurately here and that the inner reader uses for instance `read_at` consistently.
            // We own the value (or mutable reference) it after all.
            //
            // Also this one MUST be compatible with `seek(SeekFrom::Current(0))` as per trait
            // defintion—so at least keep those consistent if changing anything.
            &Variant::InFile {
                remainder,
                value_bytes,
                ..
            } => value_bytes - remainder,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{decoder::Decoder, encoder, tags};
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

        let mut buffer = crate::tags::ValueBuffer::default();

        decoder
            .current_ifd()
            .find_tag_buf(tags::Tag::StripByteCounts, &mut buffer)
            .expect("some strip byte count is present");

        let bytes_of_value = buffer.as_bytes().len() as u64;

        let mut reader = decoder
            .read_tag(tags::Tag::StripByteCounts)?
            .expect("some strip byte count is present");

        let mut data = vec![];
        reader.read_to_end(&mut data)?;

        // `ValueBuffer` also stores its data in the file endianess until converted.
        assert_eq!(reader.stream_position()?, bytes_of_value);
        assert_eq!(buffer.as_bytes(), data);

        reader.seek(SeekFrom::Start(0))?;
        assert_eq!(reader.stream_position()?, 0);
        reader.read_to_end(&mut data)?;
        assert_eq!(reader.stream_position()?, bytes_of_value);

        let (first, second) = data.split_at(bytes_of_value as usize);
        assert_eq!(first, second);

        Ok(())
    }

    #[test]
    #[allow(clippy::seek_from_current)] // We want to test that functionality.
    fn seek_from_end() -> Result<(), crate::error::TiffError> {
        const ARTIST: &str = "99 little bugs in the code";

        let data = {
            let mut data = std::io::Cursor::new(vec![]);
            let mut encoder = encoder::TiffEncoder::new(&mut data)?;

            let mut dir = encoder.image_directory()?;
            dir.write_tag(tags::Tag::Artist, ARTIST)?;
            dir.finish()?;

            data.into_inner()
        };

        let file = std::io::Cursor::new(data);
        let mut decoder = Decoder::open(file).unwrap();
        decoder.next_directory().unwrap();

        let mut reader = decoder
            .read_tag(tags::Tag::Artist)?
            .expect("the artist is present?");

        let mut value = vec![];
        reader.read_to_end(&mut value)?;

        let (&nul, value) = value.split_last().unwrap();
        assert_eq!(value, ARTIST.as_bytes());
        assert_eq!(nul, b'\0');

        assert_eq!(reader.seek(SeekFrom::End(0))?, ARTIST.len() as u64 + 1);
        assert!(reader.seek(SeekFrom::End(1)).is_err());
        assert_eq!(reader.stream_position()?, ARTIST.len() as u64 + 1);

        // Check relative seeks by comparing against the "code"
        let code_offset = reader.seek(SeekFrom::End(-5))?;
        let mut is_code = [0u8; 4];
        reader.read_exact(&mut is_code)?;
        assert_eq!(is_code, *value.last_chunk::<4>().unwrap());

        assert_eq!(reader.seek(SeekFrom::Start(0))?, 0);
        assert_eq!(reader.stream_position()?, 0);
        assert_eq!(reader.seek(SeekFrom::Current(0))?, 0);
        assert_eq!(reader.stream_position()?, 0);
        assert!(reader.seek(SeekFrom::Current(-1)).is_err());
        assert_eq!(reader.stream_position()?, 0);

        reader.seek(SeekFrom::Start(code_offset))?;
        reader.read_exact(&mut is_code)?;
        assert_eq!(reader.stream_position()?, ARTIST.len() as u64);
        assert_eq!(is_code, *value.last_chunk::<4>().unwrap());

        Ok(())
    }

    /// Check that the reader from an `offset` field also works.
    #[test]
    fn read_from_offset() -> Result<(), crate::error::TiffError> {
        let data = {
            let mut data = std::io::Cursor::new(vec![]);
            let mut encoder = encoder::TiffEncoder::new(&mut data)?;

            let mut dir = encoder.image_directory()?;
            dir.write_tag(
                tags::Tag::PlanarConfiguration,
                tags::PlanarConfiguration::Planar,
            )?;
            dir.finish()?;

            data.into_inner()
        };

        let file = std::io::Cursor::new(data);
        let mut decoder = Decoder::open(file).unwrap();
        decoder.next_directory().unwrap();
        let bo = decoder.byte_order();

        let mut reader = decoder
            .read_tag(tags::Tag::PlanarConfiguration)?
            .expect("some artist is present");

        let mut buf = [0; 2];
        reader.read_exact(&mut buf)?;

        bo.convert(tags::Type::SHORT, &mut buf, tags::ByteOrder::BigEndian);
        assert_eq!(buf, [0, 2]);

        // Check relative seeks, this item is only two bytes long.
        assert!(reader.seek(SeekFrom::End(-3)).is_err());

        Ok(())
    }
}
