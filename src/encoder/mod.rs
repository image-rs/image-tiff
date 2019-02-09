use std::io::{Write, Seek};
use std::collections::BTreeMap;
use byteorder::{NativeEndian};

use crate::decoder::ifd;
use crate::error::{TiffResult, TiffFormatError, TiffError};

mod writer;
pub mod colortype;

use self::writer::*;
use self::colortype::*;

pub struct Rational {
    pub n: u32,
    pub d: u32,
}

pub trait TiffValue {
    const BYTE_LEN: u32;
    const FIELD_TYPE: u16;
    fn count(&self) -> u32;
    fn bytes(&self) -> u32 {
        self.count() * Self::BYTE_LEN
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()>;
}

impl<T: TiffValue> TiffValue for &[T] {
    const BYTE_LEN: u32 = T::BYTE_LEN;
    const FIELD_TYPE: u16 = T::FIELD_TYPE;

    fn count(&self) -> u32 {
        self.iter().map(|x| x.count()).sum()
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        for x in *self {
            x.write(writer)?;
        }
        Ok(())
    }
}

impl TiffValue for u8 {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: u16 = 1;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u8(*self)?;
        Ok(())
    }
}

impl TiffValue for u16 {
    const BYTE_LEN: u32 = 2;
    const FIELD_TYPE: u16 = 3;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u16(*self)?;
        Ok(())
    }
}

impl TiffValue for u32 {
    const BYTE_LEN: u32 = 4;
    const FIELD_TYPE: u16 = 4;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(*self)?;
        Ok(())
    }
}

impl TiffValue for Rational {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: u16 = 5;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(self.n)?;
        writer.write_u32(self.d)?;
        Ok(())
    }
}

impl TiffValue for str {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: u16 = 2;

    fn count(&self) -> u32 {
        self.len() as u32 + 1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        if self.is_ascii() {
            writer.write_bytes(self.as_bytes())?;
            writer.write_u8(0)?;
            Ok(())
        }
        else {
            Err(TiffError::FormatError(TiffFormatError::InvalidTag))
        }
    }
}




pub struct TiffEncoder<W> {
    writer: TiffWriter<W>,
}

impl<W: Write + Seek> TiffEncoder<W> {
    pub fn new(writer: W) -> TiffResult<TiffEncoder<W>> {
        let mut encoder = TiffEncoder {
            writer: TiffWriter::new(writer),
        };
        
        NativeEndian::write_header(&mut encoder.writer)?;

        Ok(encoder)
    }

    pub fn new_directory(&mut self) -> TiffResult<DirectoryEncoder<W>> {
        DirectoryEncoder::new(&mut self.writer)
    }

    pub fn new_image<C: ColorType>(&mut self, width: u32, height: u32) -> TiffResult<ImageEncoder<W, C>> {
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        ImageEncoder::new(encoder, width, height)
    }

    pub fn write_image<C: ColorType>(&mut self, width: u32, height: u32, data: &[C::Inner]) -> TiffResult<()> {
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        let mut image: ImageEncoder<W, C> = ImageEncoder::new(encoder, width, height).unwrap();

        let mut idx = 0;
        while image.next_strip_sample_count() > 0 {
            let sample_count = image.next_strip_sample_count() as usize;
            image.write_strip(&data[idx..idx+sample_count]).unwrap();
            idx += sample_count;
        }
        image.finish()
    }
}

pub struct DirectoryEncoder<'a, W> {
    writer: &'a mut TiffWriter<W>,
    dropped: bool,
    // We use BTreeMap to make sure tags are written in correct order
    ifd_pointer_pos: u64,
    ifd: BTreeMap<u16, (u16, u32, Vec<u8>)>,
}

impl<'a, W: Write + Seek> DirectoryEncoder<'a, W> {
    fn new(writer: &'a mut TiffWriter<W>) -> TiffResult<DirectoryEncoder<'a, W>> {
        writer.pad_word_boundary()?;
        let ifd_pointer_pos = writer.offset();
        writer.write_u32(0)?;
        Ok(DirectoryEncoder {
            writer,
            dropped: false,
            ifd_pointer_pos,
            ifd: BTreeMap::new(),
        })
    }

    pub fn write_tag<T: TiffValue>(&mut self, tag: ifd::Tag, value: T) {
        let len = <T>::BYTE_LEN * value.count();
        let mut bytes = Vec::with_capacity(len as usize);
        {
            let mut writer = TiffWriter::new(&mut bytes);
            value.write(&mut writer).unwrap();
        }

        self.ifd.insert(tag.to_u16(), (<T>::FIELD_TYPE, value.count(), bytes));
    }

    pub fn modify_tag<T: TiffValue>(&mut self, tag: ifd::Tag, offset: u64, value: T) -> TiffResult<()> {
        let bytes = &mut self.ifd.get_mut(&tag.to_u16())
            .ok_or(TiffError::FormatError(TiffFormatError::RequiredTagNotFound(tag)))?.2;

        let mut writer = TiffWriter::new(std::io::Cursor::new(bytes));

        writer.goto_offset(offset)?;
        value.write(&mut writer)?;
        Ok(())
    }

    fn write_directory(&mut self) -> TiffResult<u64> {
        // Start by writing out all values
        for (_, _, ref mut bytes) in self.ifd.values_mut() {
            if bytes.len() > 4 {
                let offset = self.writer.offset();
                self.writer.write_bytes(bytes)?;
                *bytes = vec![0, 0, 0, 0];
                let mut writer = TiffWriter::new(bytes as &mut [u8]);
                writer.write_u32(offset as u32)?;
            }
            else {
                while bytes.len() < 4 {
                    bytes.push(0);
                }
            }
        }

        let offset = self.writer.offset();  

        self.writer.write_u16(self.ifd.len() as u16)?;
        for (tag, (field_type, count, offset)) in self.ifd.iter() {
            self.writer.write_u16(*tag)?;
            self.writer.write_u16(*field_type)?;
            self.writer.write_u32(*count)?;
            self.writer.write_bytes(offset)?;
        }

        Ok(offset)
    }

    pub fn write_data<T: TiffValue>(&mut self, value: T) -> TiffResult<u64> {
        let offset = self.writer.offset();
        value.write(&mut self.writer)?;
        Ok(offset)
    }

    pub fn finish(mut self) -> TiffResult<()> {
        let ifd_pointer = self.write_directory()?;
        let curr_pos = self.writer.offset();

        self.writer.goto_offset(self.ifd_pointer_pos)?;
        self.writer.write_u32(ifd_pointer as u32)?;
        self.writer.goto_offset(curr_pos)?;
        self.writer.write_u32(0)?;

        self.dropped = true;

        Ok(())
    }
}

impl<'a, W> Drop for DirectoryEncoder<'a, W> {
    fn drop(&mut self) {
        if !self.dropped {
            panic!("Illegal to drop DirectoryEncoder: you should call `DirectoryEncoder::finish`")
        }
    }
}

pub struct ImageEncoder<'a, W, T> {
    encoder: DirectoryEncoder<'a, W>,
    strip_idx: u64,
    strip_count: u64,
    row_samples: u64,
    height: u32,
    rows_per_strip: u64,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, W: Write + Seek, T: ColorType> ImageEncoder<'a, W, T> {
    fn new(mut encoder: DirectoryEncoder<'a, W>, width: u32, height: u32) -> TiffResult<ImageEncoder<'a, W, T>> {
        use self::ifd::Tag;
        let row_samples = width as u64 * <T>::bits_per_sample().len() as u64;
        let row_bytes = row_samples * <T::Inner>::BYTE_LEN as u64;

        // As per tiff spec each strip should be about 8k long
        let rows_per_strip = (8000 + row_bytes - 1) / row_bytes;

        let strip_count = (height as u64 + rows_per_strip - 1) / rows_per_strip;

        encoder.write_tag(Tag::ImageWidth, width);
        encoder.write_tag(Tag::ImageLength, height);
        encoder.write_tag(Tag::Compression, 1u16);

        encoder.write_tag(Tag::BitsPerSample, &<T>::bits_per_sample() as &[u16]);
        encoder.write_tag(Tag::PhotometricInterpretation, <T>::TIFF_VALUE);

        encoder.write_tag(Tag::StripOffsets, &vec![0u32; strip_count as usize] as &[u32]);
        encoder.write_tag(Tag::StripByteCounts, &vec![0u32; strip_count as usize] as &[u32]);
        encoder.write_tag(Tag::RowsPerStrip, rows_per_strip as u32);

        encoder.write_tag(Tag::SamplesPerPixel, <T>::bits_per_sample().len() as u16);
        encoder.write_tag(Tag::XResolution, Rational {n: 1, d: 1});
        encoder.write_tag(Tag::YResolution, Rational {n: 1, d: 1});
        encoder.write_tag(Tag::ResolutionUnit, 1u16);
        
        Ok(ImageEncoder {
            encoder,
            strip_count,
            strip_idx: 0,
            row_samples,
            rows_per_strip,
            height,
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn next_strip_sample_count(&self) -> u64 {
        if self.strip_idx >= self.strip_count {
            return 0
        }

        let start_row = std::cmp::min(self.height as u64, self.strip_idx * self.rows_per_strip);
        let end_row = std::cmp::min(self.height as u64, (self.strip_idx+1)*self.rows_per_strip);

        (end_row - start_row) * self.row_samples
    }

    pub fn write_strip(&mut self, value: &[T::Inner]) -> TiffResult<()> {
        // TODO: Compression
        let offset = self.encoder.write_data(value)?;
        self.encoder.modify_tag(ifd::Tag::StripOffsets, self.strip_idx as u64 * 4, offset as u32)?;
        self.encoder.modify_tag(ifd::Tag::StripByteCounts, self.strip_idx as u64 * 4, value.bytes() as u32)?;

        self.strip_idx += 1;
        Ok(())
    }

    pub fn finish(self) -> TiffResult<()> {
        self.encoder.finish()
    }
}
