use crate::{
    encoder::{TiffValue, TiffWriter},
    error::{TiffError, TiffResult, UsageError},
    ifd::{BufferedEntry, ImageFileDirectory},
    tags::{IsTag, Tag},
    TiffKind,
};
use std::{
    io::{Seek, Write},
    marker::PhantomData,
    mem,
};

/// Low level interface to encode ifd directories.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
pub struct DirectoryEncoder<'a, W: 'a + Write + Seek, K: TiffKind> {
    pub writer: &'a mut TiffWriter<W>,
    dropped: bool,
    ifd_pointer_pos: u64,
    ifd: ImageFileDirectory<u16, BufferedEntry>,
    sub_ifd: Option<ImageFileDirectory<u16, BufferedEntry>>,
    _phantom: PhantomData<K>,
}

impl<'a, W: 'a + Write + Seek, K: TiffKind> DirectoryEncoder<'a, W, K> {
    pub fn new(writer: &'a mut TiffWriter<W>) -> TiffResult<Self> {
        // the previous word is the IFD offset position
        let ifd_pointer_pos = writer.offset() - mem::size_of::<K::OffsetType>() as u64;
        writer.pad_word_boundary()?; // TODO: Do we need to adjust this for BigTiff?
        Ok(DirectoryEncoder::<W, K> {
            writer,
            dropped: false,
            ifd_pointer_pos,
            ifd: ImageFileDirectory::new(),
            sub_ifd: None,
            _phantom: ::std::marker::PhantomData,
        })
    }

    pub fn contains(&self, tag: &Tag) -> bool {
        self.ifd.contains_key(&(*tag).into())
    }

    /// Start writing to sub-IFD
    pub fn subdirectory_start(&mut self) {
        self.sub_ifd = Some(ImageFileDirectory::new());
    }

    /// Stop writing to sub-IFD and resume master IFD, returns offset of sub-IFD
    pub fn subdirectory_close(&mut self) -> TiffResult<u64> {
        let ifd = self
            .sub_ifd
            .to_owned()
            .ok_or(TiffError::UsageError(UsageError::CloseNonExistentIfd))?;
        self.sub_ifd = None;

        let offset = self.write_directory(ifd)?;
        K::write_offset(self.writer, 0)?;

        Ok(offset)
    }

    pub fn write_exif<T: IsTag + Copy, V: TiffValue>(
        &mut self,
        exif: &ImageFileDirectory<T, V>,
    ) -> TiffResult<()> {
        for (tag, value) in exif.iter() {
            self.write_tag(*tag, value)?;
        }

        Ok(())
    }

    /// Write a single ifd tag.
    pub fn write_tag<T: IsTag, V: TiffValue>(&mut self, tag: T, value: V) -> TiffResult<()> {
        let mut bytes = Vec::with_capacity(value.bytes());
        {
            let mut writer = TiffWriter::new(&mut bytes);
            value.write(&mut writer)?;
        }

        let active_ifd = match &self.sub_ifd {
            None => &mut self.ifd,
            Some(_v) => self.sub_ifd.as_mut().unwrap(),
        };

        active_ifd.insert(
            tag.into(),
            BufferedEntry {
                type_: value.is_type(),
                count: value.count().try_into()?,
                data: bytes,
            },
        );

        Ok(())
    }

    fn write_directory<T: Ord + Into<u16>>(
        &mut self,
        mut ifd: ImageFileDirectory<T, BufferedEntry>,
    ) -> TiffResult<u64> {
        // Prep work: go through the entries and write the ones that do not fit in an entry
        for &mut BufferedEntry {
            data: ref mut bytes,
            ..
        } in ifd.values_mut()
        {
            // Amount of bytes available in the Tiff type
            let data_bytes = K::OffsetType::BYTE_LEN as usize;

            // If the data does not fit in the entry, we write it down now, and write the
            // offset to it later
            if bytes.len() > data_bytes {
                // Record where we are now
                let offset = self.writer.offset();

                // Write the data
                self.writer.write_bytes(bytes)?;

                // Overwrite the data with a buffer matching the offset size
                *bytes = vec![0; data_bytes]; // TODO Maybe just truncate ?

                // Replace the entry value with the offset recorded above
                K::write_offset(&mut TiffWriter::new(bytes as &mut [u8]), offset)?;
            } else {
                // Pad the data with zeros to the correct length
                while bytes.len() < data_bytes {
                    bytes.push(0);
                }
            }
        }

        // Record the offset
        let ifd_offset = self.writer.offset();

        // Actually write the ifd - first the count, then entries in order
        K::write_entry_count(self.writer, ifd.len())?;
        for (
            tag,
            BufferedEntry {
                type_: field_type,
                count,
                data, // At this point data is [u8; K::OffsetType::BYTE_LEN]
            },
        ) in ifd.into_iter()
        {
            self.writer.write_u16(tag.into())?;
            self.writer.write_u16(field_type.to_u16())?;
            K::convert_offset(count)?.write(self.writer)?;
            self.writer.write_bytes(&data)?;
        }

        Ok(ifd_offset)
    }

    /// Write some data to the tiff file, the offset of the data is returned.
    ///
    /// This could be used to write tiff strips.
    pub fn write_data<T: TiffValue>(&mut self, value: T) -> TiffResult<u64> {
        let offset = self.writer.offset();
        value.write(self.writer)?;
        Ok(offset)
    }

    /// Provides the number of bytes written by the underlying TiffWriter during the last call.
    pub fn last_written(&self) -> u64 {
        self.writer.last_written()
    }

    pub fn finish_internal(&mut self) -> TiffResult<()> {
        if self.sub_ifd.is_some() {
            self.subdirectory_close()?;
        }

        let ifd_pointer = self.write_directory(self.ifd.to_owned())?;
        let curr_pos = self.writer.offset();

        self.writer.goto_offset(self.ifd_pointer_pos)?;
        K::write_offset(self.writer, ifd_pointer)?;
        self.writer.goto_offset(curr_pos)?;
        K::write_offset(self.writer, 0)?;

        self.dropped = true;

        Ok(())
    }

    /// Write out the ifd directory.
    pub fn finish(mut self) -> TiffResult<()> {
        self.finish_internal()
    }
}

impl<'a, W: Write + Seek, K: TiffKind> Drop for DirectoryEncoder<'a, W, K> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}
