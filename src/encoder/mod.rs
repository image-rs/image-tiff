use std::io::{Write, Seek};
use std::collections::BTreeMap;
use byteorder::{NativeEndian};

use crate::decoder::ifd::Tag;
use crate::error::{TiffResult, TiffFormatError, TiffError};

mod writer;
pub mod colortype;

use self::writer::*;
use self::colortype::*;

/// Type to represent tiff values of type `RATIONAL`
pub struct Rational {
    pub n: u32,
    pub d: u32,
}

/// Trait for types that can be encoded in a tiff file
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

/// Tiff encoder.
///
/// With this type you can get a `DirectoryEncoder` or a `ImageEncoder`
/// to encode tiff ifd directories with images.
///
/// See `DirectoryEncoder` and `ImageEncoder`.
///
/// # Examples
/// ```
/// # extern crate tempfile;
/// # let mut file = tempfile::tempfile().unwrap();
/// # let mut image_data = Vec::new();
/// # for x in 0..100 {
/// #     for y in 0..100u8 {
/// #         let val = x + y;
/// #         image_data.push(val);
/// #         image_data.push(val);
/// #         image_data.push(val);
/// #     }
/// # }
/// use tiff::encoder::*;
///
/// let mut tiff = TiffEncoder::new(&mut file).unwrap();
///
/// tiff.write_image::<colortype::RGB8>(100, 100, &image_data).unwrap();
/// ```
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

    /// Create a `DirectoryEncoder` to encode an ifd directory.
    pub fn new_directory(&mut self) -> TiffResult<DirectoryEncoder<W>> {
        DirectoryEncoder::new(&mut self.writer)
    }

    /// Create an 'ImageEncoder' to encode an image one slice at a time.
    pub fn new_image<C: ColorType>(&mut self, width: u32, height: u32) -> TiffResult<ImageEncoder<W, C>> {
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        ImageEncoder::new(encoder, width, height)
    }

    /// Convenience function to write an entire image from memory.
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

/// Low level interface to encode ifd directories.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
pub struct DirectoryEncoder<'a, W: Write + Seek> {
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

    /// Write a single ifd tag.
    pub fn write_tag<T: TiffValue>(&mut self, tag: Tag, value: T) {
        let len = <T>::BYTE_LEN * value.count();
        let mut bytes = Vec::with_capacity(len as usize);
        {
            let mut writer = TiffWriter::new(&mut bytes);
            value.write(&mut writer).unwrap();
        }

        self.ifd.insert(tag.to_u16(), (<T>::FIELD_TYPE, value.count(), bytes));
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

    /// Write some data to the tiff file, the offset of the data is returned.
    ///
    /// This could be used to write tiff strips.
    pub fn write_data<T: TiffValue>(&mut self, value: T) -> TiffResult<u64> {
        let offset = self.writer.offset();
        value.write(&mut self.writer)?;
        Ok(offset)
    }

    fn finish_internal(&mut self) -> TiffResult<()> {
        let ifd_pointer = self.write_directory()?;
        let curr_pos = self.writer.offset();

        self.writer.goto_offset(self.ifd_pointer_pos)?;
        self.writer.write_u32(ifd_pointer as u32)?;
        self.writer.goto_offset(curr_pos)?;
        self.writer.write_u32(0)?;

        self.dropped = true;

        Ok(())
    }

    /// Write out the ifd directory.
    pub fn finish(mut self) -> TiffResult<()> {
        self.finish_internal()
    }
}

impl<'a, W: Write + Seek> Drop for DirectoryEncoder<'a, W> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}


/// Type to encode images strip by strip.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
///
/// # Examples
/// ```
/// # extern crate tempfile;
/// # let mut file = tempfile::tempfile().unwrap();
/// # let mut image_data = Vec::new();
/// # for x in 0..100 {
/// #     for y in 0..100u8 {
/// #         let val = x + y;
/// #         image_data.push(val);
/// #         image_data.push(val);
/// #         image_data.push(val);
/// #     }
/// # }
/// use tiff::encoder::*;
///
/// let mut tiff = TiffEncoder::new(&mut file).unwrap();
/// let mut image = tiff.new_image::<colortype::RGB8>(100, 100).unwrap();
///
/// let mut idx = 0;
/// while image.next_strip_sample_count() > 0 {
///     let sample_count = image.next_strip_sample_count() as usize;
///     image.write_strip(&image_data[idx..idx+sample_count]).unwrap();
///     idx += sample_count;
/// }
/// image.finish().unwrap();
/// ```
pub struct ImageEncoder<'a, W: Write + Seek, C: ColorType> {
    encoder: DirectoryEncoder<'a, W>,
    strip_idx: u64,
    strip_count: u64,
    row_samples: u64,
    height: u32,
    rows_per_strip: u64,
    strip_offsets: Vec<u32>,
    strip_byte_count: Vec<u32>,
    dropped: bool,
    _phantom: std::marker::PhantomData<C>,
}

impl<'a, W: Write + Seek, T: ColorType> ImageEncoder<'a, W, T> {
    fn new(mut encoder: DirectoryEncoder<'a, W>, width: u32, height: u32) -> TiffResult<ImageEncoder<'a, W, T>> {
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
            strip_offsets: Vec::new(),
            strip_byte_count: Vec::new(),
            dropped: false,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Number of samples the next strip should have.
    pub fn next_strip_sample_count(&self) -> u64 {
        if self.strip_idx >= self.strip_count {
            return 0
        }

        let start_row = std::cmp::min(self.height as u64, self.strip_idx * self.rows_per_strip);
        let end_row = std::cmp::min(self.height as u64, (self.strip_idx+1)*self.rows_per_strip);

        (end_row - start_row) * self.row_samples
    }

    /// Write a single strip.
    pub fn write_strip(&mut self, value: &[T::Inner]) -> TiffResult<()> {
        // TODO: Compression
        let samples = self.next_strip_sample_count();
        assert_eq!(value.len() as u64, samples);

        let offset = self.encoder.write_data(value)?;
        self.strip_offsets.push(offset as u32);
        self.strip_byte_count.push(value.bytes() as u32);

        self.strip_idx += 1;
        Ok(())
    }

    fn finish_internal(&mut self) -> TiffResult<()> {
        self.encoder.write_tag(Tag::StripOffsets, &*self.strip_offsets);
        self.encoder.write_tag(Tag::StripByteCounts, &*self.strip_byte_count);
        self.dropped = true;

        self.encoder.finish_internal()
    }

    /// Write out image and ifd directory.
    pub fn finish(mut self) -> TiffResult<()> {
        self.finish_internal()
    }
}

impl<'a, W: Write + Seek, C: ColorType> Drop for ImageEncoder<'a, W, C> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}

