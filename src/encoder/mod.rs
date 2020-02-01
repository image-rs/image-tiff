use byteorder::NativeEndian;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::io::{Seek, Write};
use std::mem;

use tags::{self, ResolutionUnit, Tag, Type};
use error::{TiffError, TiffFormatError, TiffResult};

pub mod colortype;
mod writer;

use self::colortype::*;
use self::writer::*;

/// Type to represent tiff values of type `RATIONAL`
#[derive(Clone)]
pub struct Rational {
    pub n: u32,
    pub d: u32,
}

/// Type to represent tiff values of type `SRATIONAL`
pub struct SRational {
    pub n: i32,
    pub d: i32,
}

/// Trait for types that can be encoded in a tiff file
pub trait TiffValue {
    const BYTE_LEN: u32;
    const FIELD_TYPE: Type;
    fn count(&self) -> u32;
    fn bytes(&self) -> u32 {
        self.count() * Self::BYTE_LEN
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()>;
}

impl TiffValue for [u8] {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: Type = Type::BYTE;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_bytes(self)?;
        Ok(())
    }
}

impl TiffValue for [i8] {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: Type = Type::SBYTE;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        // We write using nativeedian so this should be safe
        let slice =
            unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len()) };
        writer.write_bytes(slice)?;
        Ok(())
    }
}

impl TiffValue for [u16] {
    const BYTE_LEN: u32 = 2;
    const FIELD_TYPE: Type = Type::SHORT;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        // We write using nativeedian so this sould be safe
        let slice =
            unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * 2) };
        writer.write_bytes(slice)?;
        Ok(())
    }
}

impl TiffValue for [i16] {
    const BYTE_LEN: u32 = 2;
    const FIELD_TYPE: Type = Type::SSHORT;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        // We write using nativeedian so this should be safe
        let slice =
            unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::BYTE_LEN as usize) };
        writer.write_bytes(slice)?;
        Ok(())
    }
}

impl TiffValue for [u32] {
    const BYTE_LEN: u32 = 4;
    const FIELD_TYPE: Type = Type::LONG;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        // We write using nativeedian so this sould be safe
        let slice =
            unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * 4) };
        writer.write_bytes(slice)?;
        Ok(())
    }
}

impl TiffValue for [i32] {
    const BYTE_LEN: u32 = 4;
    const FIELD_TYPE: Type = Type::SLONG;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        // We write using nativeedian so this should be safe
        let slice =
            unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * Self::BYTE_LEN as usize) };
        writer.write_bytes(slice)?;
        Ok(())
    }
}

impl TiffValue for [u64] {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: Type = Type::LONG8;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        // We write using nativeedian so this sould be safe
        let slice =
            unsafe { ::std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * 8) };
        writer.write_bytes(slice)?;
        Ok(())
    }
}

impl TiffValue for [Rational] {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: Type = Type::RATIONAL;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        for x in self {
            x.write(writer)?;
        }
        Ok(())
    }
}

impl TiffValue for [SRational] {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: Type = Type::SRATIONAL;

    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        for x in self {
            x.write(writer)?;
        }
        Ok(())
    }
}

impl TiffValue for u8 {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: Type = Type::BYTE;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u8(*self)?;
        Ok(())
    }
}

impl TiffValue for i8 {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: Type = Type::SBYTE;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i8(*self)?;
        Ok(())
    }
}

impl TiffValue for u16 {
    const BYTE_LEN: u32 = 2;
    const FIELD_TYPE: Type = Type::SHORT;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u16(*self)?;
        Ok(())
    }
}

impl TiffValue for i16 {
    const BYTE_LEN: u32 = 2;
    const FIELD_TYPE: Type = Type::SSHORT;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i16(*self)?;
        Ok(())
    }
}

impl TiffValue for u32 {
    const BYTE_LEN: u32 = 4;
    const FIELD_TYPE: Type = Type::LONG;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(*self)?;
        Ok(())
    }
}

impl TiffValue for i32 {
    const BYTE_LEN: u32 = 4;
    const FIELD_TYPE: Type = Type::SLONG;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i32(*self)?;
        Ok(())
    }
}

impl TiffValue for u64 {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: Type = Type::LONG8;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u64(*self)?;
        Ok(())
    }
}

impl TiffValue for Rational {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: Type = Type::RATIONAL;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_u32(self.n)?;
        writer.write_u32(self.d)?;
        Ok(())
    }
}

impl TiffValue for SRational {
    const BYTE_LEN: u32 = 8;
    const FIELD_TYPE: Type = Type::SRATIONAL;

    fn count(&self) -> u32 {
        1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.write_i32(self.n)?;
        writer.write_i32(self.d)?;
        Ok(())
    }
}

impl TiffValue for str {
    const BYTE_LEN: u32 = 1;
    const FIELD_TYPE: Type = Type::ASCII;

    fn count(&self) -> u32 {
        self.len() as u32 + 1
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        if self.is_ascii() && !self.bytes().any(|b| b == 0) {
            writer.write_bytes(self.as_bytes())?;
            writer.write_u8(0)?;
            Ok(())
        } else {
            Err(TiffError::FormatError(TiffFormatError::InvalidTag))
        }
    }
}

impl<'a, T: TiffValue + ?Sized> TiffValue for &'a T {
    const BYTE_LEN: u32 = T::BYTE_LEN;
    const FIELD_TYPE: Type = T::FIELD_TYPE;

    fn count(&self) -> u32 {
        (*self).count()
    }

    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        (*self).write(writer)
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
/// # extern crate tiff;
/// # fn main() {
/// # let mut file = std::io::Cursor::new(Vec::new());
/// # let image_data = vec![0; 100*100*3];
/// use tiff::encoder::*;
///
/// let mut tiff = TiffEncoder::new(&mut file).unwrap();
///
/// tiff.write_image::<colortype::RGB8>(100, 100, &image_data).unwrap();
/// # }
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
        // blank the IFD offset location
        encoder.writer.write_u32(0)?;

        Ok(encoder)
    }

    /// Create a `DirectoryEncoder` to encode an ifd directory.
    pub fn new_directory(&mut self) -> TiffResult<DirectoryEncoder<W>> {
        DirectoryEncoder::new(&mut self.writer)
    }

    /// Create an 'ImageEncoder' to encode an image one slice at a time.
    pub fn new_image<C: ColorType>(
        &mut self,
        width: u32,
        height: u32,
    ) -> TiffResult<ImageEncoder<W, C>> {
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        ImageEncoder::new(encoder, width, height)
    }

    /// Convenience function to write an entire image from memory.
    pub fn write_image<C: ColorType>(
        &mut self,
        width: u32,
        height: u32,
        data: &[C::Inner],
    ) -> TiffResult<()>
    where
        [C::Inner]: TiffValue,
    {
        let num_pix = usize::try_from(width)?.checked_mul(usize::try_from(height)?)
            .ok_or_else(|| ::std::io::Error::new(
                ::std::io::ErrorKind::InvalidInput,
                "Image width * height exceeds usize"))?;
        if data.len() < num_pix {
            return Err(::std::io::Error::new(
                ::std::io::ErrorKind::InvalidData,
                "Input data slice is undersized for provided dimensions").into());
        }

        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        let mut image: ImageEncoder<W, C> = ImageEncoder::new(encoder, width, height)?;

        let mut idx = 0;
        while image.next_strip_sample_count() > 0 {
            let sample_count = usize::try_from(image.next_strip_sample_count())?;
            image.write_strip(&data[idx..idx + sample_count])?;
            idx += sample_count;
        }
        image.finish()
    }
}

/// Low level interface to encode ifd directories.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
pub struct DirectoryEncoder<'a, W: 'a + Write + Seek> {
    writer: &'a mut TiffWriter<W>,
    dropped: bool,
    // We use BTreeMap to make sure tags are written in correct order
    ifd_pointer_pos: u64,
    ifd: BTreeMap<u16, (u16, u32, Vec<u8>)>,
}

impl<'a, W: 'a + Write + Seek> DirectoryEncoder<'a, W> {
    fn new(writer: &'a mut TiffWriter<W>) -> TiffResult<DirectoryEncoder<'a, W>> {
        // the previous word is the IFD offset position
        let ifd_pointer_pos = writer.offset() - mem::size_of::<u32>() as u64;
        writer.pad_word_boundary()?;
        Ok(DirectoryEncoder {
            writer,
            dropped: false,
            ifd_pointer_pos,
            ifd: BTreeMap::new(),
        })
    }

    /// Write a single ifd tag.
    pub fn write_tag<T: TiffValue>(&mut self, tag: Tag, value: T) -> TiffResult<()> {
        let len = <T>::BYTE_LEN * value.count();
        let mut bytes = Vec::with_capacity(usize::try_from(len)?);
        {
            let mut writer = TiffWriter::new(&mut bytes);
            value.write(&mut writer)?;
        }

        self.ifd
            .insert(tag.to_u16(), (<T>::FIELD_TYPE.to_u16(), value.count(), bytes));

        Ok(())
    }

    fn write_directory(&mut self) -> TiffResult<u64> {
        // Start by writing out all values
        for &mut (_, _, ref mut bytes) in self.ifd.values_mut() {
            if bytes.len() > 4 {
                let offset = self.writer.offset();
                self.writer.write_bytes(bytes)?;
                *bytes = vec![0, 0, 0, 0];
                let mut writer = TiffWriter::new(bytes as &mut [u8]);
                writer.write_u32(u32::try_from(offset)?)?;
            } else {
                while bytes.len() < 4 {
                    bytes.push(0);
                }
            }
        }

        let offset = self.writer.offset();

        self.writer.write_u16(u16::try_from(self.ifd.len())?)?;
        for (tag, &(ref field_type, ref count, ref offset)) in self.ifd.iter() {
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
        self.writer.write_u32(u32::try_from(ifd_pointer)?)?;
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
/// # extern crate tiff;
/// # fn main() {
/// # let mut file = std::io::Cursor::new(Vec::new());
/// # let image_data = vec![0; 100*100*3];
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
/// # }
/// ```
pub struct ImageEncoder<'a, W: 'a + Write + Seek, C: ColorType> {
    encoder: DirectoryEncoder<'a, W>,
    strip_idx: u64,
    strip_count: u64,
    row_samples: u64,
    height: u32,
    rows_per_strip: u64,
    strip_offsets: Vec<u32>,
    strip_byte_count: Vec<u32>,
    dropped: bool,
    _phantom: ::std::marker::PhantomData<C>,
}

impl<'a, W: 'a + Write + Seek, T: ColorType> ImageEncoder<'a, W, T> {
    fn new(
        mut encoder: DirectoryEncoder<'a, W>,
        width: u32,
        height: u32,
    ) -> TiffResult<ImageEncoder<'a, W, T>> {
        let row_samples = u64::from(width) * u64::try_from(<T>::BITS_PER_SAMPLE.len())?;
        let row_bytes = row_samples * u64::from(<T::Inner>::BYTE_LEN);

        // As per tiff spec each strip should be about 8k long
        let rows_per_strip = (8000 + row_bytes - 1) / row_bytes;

        let strip_count = (u64::from(height) + rows_per_strip - 1) / rows_per_strip;

        encoder.write_tag(Tag::ImageWidth, width)?;
        encoder.write_tag(Tag::ImageLength, height)?;
        encoder.write_tag(Tag::Compression, tags::CompressionMethod::None.to_u16())?;

        encoder.write_tag(Tag::BitsPerSample, <T>::BITS_PER_SAMPLE)?;
        encoder.write_tag(Tag::PhotometricInterpretation, <T>::TIFF_VALUE.to_u16())?;

        encoder.write_tag(Tag::RowsPerStrip, u32::try_from(rows_per_strip)?)?;

        encoder.write_tag(Tag::SamplesPerPixel, u16::try_from(<T>::BITS_PER_SAMPLE.len())?)?;
        encoder.write_tag(Tag::XResolution, Rational { n: 1, d: 1 })?;
        encoder.write_tag(Tag::YResolution, Rational { n: 1, d: 1 })?;
        encoder.write_tag(Tag::ResolutionUnit, ResolutionUnit::None.to_u16())?;

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
            _phantom: ::std::marker::PhantomData,
        })
    }

    /// Number of samples the next strip should have.
    pub fn next_strip_sample_count(&self) -> u64 {
        if self.strip_idx >= self.strip_count {
            return 0;
        }

        let start_row = ::std::cmp::min(u64::from(self.height), self.strip_idx * self.rows_per_strip);
        let end_row = ::std::cmp::min(
            u64::from(self.height),
            (self.strip_idx + 1) * self.rows_per_strip,
        );

        (end_row - start_row) * self.row_samples
    }

    /// Write a single strip.
    pub fn write_strip(&mut self, value: &[T::Inner]) -> TiffResult<()>
    where
        [T::Inner]: TiffValue,
    {
        // TODO: Compression
        let samples = self.next_strip_sample_count();
        if u64::try_from(value.len())? != samples {
            return Err(::std::io::Error::new(
                ::std::io::ErrorKind::InvalidData,
                "Slice is wrong size for strip").into());
        }

        let offset = self.encoder.write_data(value)?;
        self.strip_offsets.push(u32::try_from(offset)?);
        self.strip_byte_count.push(value.bytes());

        self.strip_idx += 1;
        Ok(())
    }

    /// Set image resolution
    pub fn resolution(&mut self, unit: ResolutionUnit, value: Rational) {
        self.encoder.write_tag(Tag::ResolutionUnit, unit.to_u16()).unwrap();
        self.encoder.write_tag(Tag::XResolution, value.clone()).unwrap();
        self.encoder.write_tag(Tag::YResolution, value).unwrap();
    }

    /// Set image resolution unit
    pub fn resolution_unit(&mut self, unit: ResolutionUnit) {
        self.encoder.write_tag(Tag::ResolutionUnit, unit.to_u16()).unwrap();
    }

    /// Set image x-resolution
    pub fn x_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::XResolution, value).unwrap();
    }

    /// Set image y-resolution
    pub fn y_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::YResolution, value).unwrap();
    }

    fn finish_internal(&mut self) -> TiffResult<()> {
        self.encoder
            .write_tag(Tag::StripOffsets, &*self.strip_offsets)?;
        self.encoder
            .write_tag(Tag::StripByteCounts, &*self.strip_byte_count)?;
        self.dropped = true;

        self.encoder.finish_internal()
    }

    /// Get a reference of the underlying `DirectoryEncoder`
    pub fn encoder(&mut self) -> &mut DirectoryEncoder<'a, W> {
        &mut self.encoder
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
