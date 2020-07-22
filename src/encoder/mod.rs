use byteorder::NativeEndian;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::io::{Cursor, Seek, Write};
use std::mem;
use lzw::{EncoderTIFF, MsbWriter};

use tags::{ResolutionUnit, Tag, Type, CompressionMethod};
use error::{TiffError, TiffFormatError, TiffResult};

pub mod colortype;
mod writer;

use self::colortype::*;
use self::writer::*;

/// Type to represent tiff values of type `RATIONAL`
#[derive(Clone, Copy)]
pub struct Rational {
    pub n: u32,
    pub d: u32,
}

/// Type to represent tiff values of type `SRATIONAL`
#[derive(Clone, Copy)]
pub struct SRational {
    pub n: i32,
    pub d: i32,
}

pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    U64(u64),
    F64(f64),
    Rational(Rational),
    SRational(SRational),
    AU8(Vec<u8>),
    AI8(Vec<i8>),
    AU16(Vec<u16>),
    AI16(Vec<i16>),
    AU32(Vec<u32>),
    AI32(Vec<i32>),
    AF32(Vec<f32>),
    AU64(Vec<u64>),
    AF64(Vec<f64>),
    ARational(Vec<Rational>),
    ASRational(Vec<SRational>),
    Str(String),
}

/// Trait for types that can be encoded in a tiff file
pub trait TiffValue {
    fn byte_len(&self) -> u32;
    fn field_type(&self) -> Type;
    fn count(&self) -> u32;
    fn bytes(&self) -> u32 {
        self.count() * self.byte_len()
    }
    fn get_slice(&self, start: usize, end: usize) -> Self;
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()>;
}

impl TiffValue for Value {
    fn byte_len(&self) -> u32 {
        match &self {
            Self::AU8(_) | Self::AI8(_) | Self::U8(_) | Self::I8(_) | Self::Str(_) => 1,
            Self::AU16(_) | Self::AI16(_) | Self::U16(_) | Self::I16(_) => 2,
            Self::AU32(_) | Self::AI32(_) | Self::AF32(_)  | Self::U32(_) | Self::I32(_) | Self::F32(_) => 4,
            Self::AU64(_) | Self::ARational(_) | Self::ASRational(_)  | Self::AF64(_) | Self::U64(_) | Self::Rational(_) | Self::SRational(_) | Self::F64(_) => 8,
        }
    }
    fn field_type(&self) -> Type {
        match &self {
            Self::AU8(_) | Self::U8(_) => Type::BYTE,
            Self::AI8(_) | Self::I8(_) => Type::SBYTE,
            Self::AU16(_) | Self::U16(_) => Type::SHORT,
            Self::AI16(_) | Self::I16(_) => Type::SSHORT,
            Self::AU32(_) | Self::U32(_) => Type::LONG,
            Self::AI32(_) | Self::I32(_) => Type::SLONG,
            Self::AF32(_) | Self::F32(_) => Type::FLOAT,
            Self::AU64(_) | Self::U64(_) => Type::LONG8,
            Self::AF64(_) | Self::F64(_) => Type::DOUBLE,
            Self::ARational(_) | Self::Rational(_) => Type::RATIONAL,
            Self::ASRational(_) | Self::SRational(_) => Type::SRATIONAL,
            Self::Str(_) => Type::ASCII,
        }
    }
    fn count(&self) -> u32 {
        match &self {
            Self::AU8(v) => v.len() as u32,
            Self::AI8(v) => v.len() as u32,
            Self::AU16(v) => v.len() as u32,
            Self::AI16(v) => v.len() as u32,
            Self::AU32(v) => v.len() as u32,
            Self::AI32(v) => v.len() as u32,
            Self::AF32(v) => v.len() as u32,
            Self::AU64(v) => v.len() as u32,
            Self::AF64(v) => v.len() as u32,
            Self::ARational(v) => v.len() as u32,
            Self::ASRational(v) => v.len() as u32,
            Self::U8(_) | Self::I8(_) | Self::U16(_) | Self::I16(_) | 
            Self::U32(_) | Self::I32(_) | Self::F32(_) | Self::U64(_) | 
            Self::Rational(_) | Self::SRational(_) | Self::F64(_) => 1,
            Self::Str(v) => v.len() as u32 + 1,
        }
    }
    fn get_slice(&self, start: usize, end: usize) -> Self {
        match &self {
            Self::AU8(v) => Self::AU8(v[start..end].into()), 
            Self::AI8(v) => Self::AI8(v[start..end].into()), 
            Self::AU16(v) => Self::AU16(v[start..end].into()), 
            Self::AI16(v) => Self::AI16(v[start..end].into()), 
            Self::AU32(v) => Self::AU32(v[start..end].into()), 
            Self::AI32(v) => Self::AI32(v[start..end].into()), 
            Self::AF32(v) => Self::AF32(v[start..end].into()), 
            Self::AU64(v) => Self::AU64(v[start..end].into()), 
            Self::AF64(v) => Self::AF64(v[start..end].into()), 
            Self::ARational(v) => Self::ARational(v[start..end].into()), 
            Self::ASRational(v) => Self::ASRational(v[start..end].into()), 
            Self::Str(v) => Self::Str(v[start..end].into()),
            Self::U8(_) | Self::I8(_) | Self::U16(_) | Self::I16(_) | 
            Self::U32(_) | Self::I32(_) | Self::F32(_) | Self::U64(_) | 
            Self::Rational(_) | Self::SRational(_) | Self::F64(_) => unimplemented!()
        }
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> TiffResult<()> {
        match &self {
            Self::AU8(v) => writer.write_bytes(v)?,
            Self::AI8(v) => {
                let slice = unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len()) };
                writer.write_bytes(slice)?;
            },
            Self::AU16(v) => {
                let slice =
                unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * self.byte_len() as usize) };
                writer.write_bytes(slice)?;
            }
            Self::AI16(v) => {
                let slice =
                    unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * self.byte_len() as usize) };
                writer.write_bytes(slice)?;
            },
            Self::AU32(v) => {
                let slice =
                unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * self.byte_len() as usize) };
                writer.write_bytes(slice)?;
            },
            Self::AI32(v) => {
                let slice =
                unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * self.byte_len() as usize) };
                writer.write_bytes(slice)?;
            },
            Self::AF32(v) => {
                let slice = unsafe {
                    ::std::slice::from_raw_parts(
                        v.as_ptr() as *const u8,
                        v.len() * self.byte_len() as usize,
                    )
                };
                writer.write_bytes(slice)?;
            },
            Self::AU64(v) => {
                let slice =
                unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * self.byte_len() as usize) };
                writer.write_bytes(slice)?;
            },
            Self::AF64(v) => {
                let slice = unsafe {
                    ::std::slice::from_raw_parts(
                        v.as_ptr() as *const u8,
                        v.len() * self.byte_len() as usize,
                    )
                };
                writer.write_bytes(slice)?;
            },
            Self::ARational(v) => {
                for x in v {
                    Value::Rational(*x).write(writer)?;
                }
            },
            Self::ASRational(v) => {
                for x in v {
                    Value::SRational(*x).write(writer)?;
                }
            },
            Self::U8(v) => writer.write_u8(*v)?,
            Self::I8(v) => writer.write_i8(*v)?,
            Self::U16(v) => writer.write_u16(*v)?, 
            Self::I16(v) => writer.write_i16(*v)?,
            Self::U32(v) => writer.write_u32(*v)?,
            Self::I32(v) => writer.write_i32(*v)?,
            Self::F32(v) => writer.write_f32(*v)?,
            Self::U64(v) => writer.write_u64(*v)?,
            Self::Rational(v) => {
                writer.write_u32(v.n)?;
                writer.write_u32(v.d)?;
            },
            Self::SRational(v) => {
                writer.write_i32(v.n)?;
                writer.write_i32(v.d)?;
            },
            Self::F64(v) => writer.write_f64(*v)?,
            Self::Str(v) => {
                if v.is_ascii() && !v.bytes().any(|b| b == 0) {
                    writer.write_bytes(v.as_bytes())?;
                    writer.write_u8(0)?;
                } else {
                    return Err(TiffError::FormatError(TiffFormatError::InvalidTag))
                }
            } ,
        }
        Ok(())
    }
}

impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::U8(v)
    }
}

impl From<i8> for Value {
    fn from(v: i8) -> Self {
        Value::I8(v)
    }
}

impl From<u16> for Value {
    fn from(v: u16) -> Self {
        Value::U16(v)
    }
}
impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Value::I16(v)
    }
}
impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::U32(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::I32(v)
    }
}
impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::F32(v)
    }
}
impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::U64(v)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}
impl From<Rational> for Value {
    fn from(v: Rational) -> Self {
        Value::Rational(v)
    }
}
impl From<SRational> for Value {
    fn from(v: SRational) -> Self {
        Value::SRational(v)
    }
}


impl From<&[u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::AU8(v.into())
    }
}
impl From<&[i8]> for Value {
    fn from(v: &[i8]) -> Self {
        Value::AI8(v.into())
    }
}
impl From<&[u16]> for Value {
    fn from(v: &[u16]) -> Self {
        Value::AU16(v.into())
    }
}
impl From<&[i16]> for Value {
    fn from(v: &[i16]) -> Self {
        Value::AI16(v.into())
    }
}
impl From<&[u32]> for Value {
    fn from(v: &[u32]) -> Self {
        Value::AU32(v.into())
    }
}
impl From<&[i32]> for Value {
    fn from(v: &[i32]) -> Self {
        Value::AI32(v.into())
    }
}
impl From<&[f32]> for Value {
    fn from(v: &[f32]) -> Self {
        Value::AF32(v.into())
    }
}
impl From<&[u64]> for Value {
    fn from(v: &[u64]) -> Self {
        Value::AU64(v.into())
    }
}
impl From<&[f64]> for Value {
    fn from(v: &[f64]) -> Self {
        Value::AF64(v.into())
    }
}
impl From<&[Rational]> for Value {
    fn from(v: &[Rational]) -> Self {
        Value::ARational(v.into())
    }
}
impl From<&[SRational]> for Value {
    fn from(v: &[SRational]) -> Self {
        Value::ASRational(v.into())
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
        // blank the IFD offset location
        encoder.writer.write_u32(0)?;

        Ok(encoder)
    }

    /// Create a `DirectoryEncoder` to encode an ifd directory.
    pub fn new_directory(&mut self) -> TiffResult<DirectoryEncoder<W>> {
        DirectoryEncoder::new(&mut self.writer)
    }

    /// Create an 'ImageEncoder' to encode an image one slice at a time.
    pub fn new_image(
        &mut self,
        width: u32,
        height: u32,
        colortype: ColorType,
        compression_method: CompressionMethod,
        additional_tags: Vec<(Tag, Value)>
    ) -> TiffResult<ImageEncoder<W>> {
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        ImageEncoder::new(encoder, width, height, colortype, compression_method, additional_tags)
    }

    /// Convenience function to write an entire image from memory.
    pub fn write_image<V>(
        &mut self,
        width: u32,
        height: u32,
        data: &V,
        colortype: ColorType,
        compression: CompressionMethod,
        additional_tags: Vec<(Tag, Value)>
    ) -> TiffResult<()>
    where
        V: TiffValue,
    {
        let num_pix = usize::try_from(width)?.checked_mul(usize::try_from(height)?)
            .ok_or_else(|| ::std::io::Error::new(
                ::std::io::ErrorKind::InvalidInput,
                "Image width * height exceeds usize"))?;
        if usize::try_from(data.count())? < num_pix {
            return Err(::std::io::Error::new(
                ::std::io::ErrorKind::InvalidData,
                "Input data slice is undersized for provided dimensions").into());
        }

        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        let mut image: ImageEncoder<W> = ImageEncoder::new(encoder, width, height, colortype, compression, additional_tags)?;

        let mut idx = 0;
        while image.next_strip_sample_count() > 0 {
            let sample_count = usize::try_from(image.next_strip_sample_count())?;
            image.write_strip(&data.get_slice(idx, idx + sample_count))?;
            // image.write_strip(&data[idx..idx + sample_count])?;
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
        let len = value.byte_len() * value.count();
        let mut bytes = Vec::with_capacity(usize::try_from(len)?);
        {
            let mut writer = TiffWriter::new(&mut bytes);
            value.write(&mut writer)?;
        }

        self.ifd
            .insert(tag.to_u16(), (value.field_type().to_u16(), value.count(), bytes));

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
    pub fn write_data<T: TiffValue>(&mut self, value: &T) -> TiffResult<u64> {
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
/// # let image_data = Value::from(&image_data[..]);
/// use tiff::encoder::*;
/// use tiff::tags::CompressionMethod;
///
/// let mut tiff = TiffEncoder::new(&mut file).unwrap();
/// let mut image = tiff.new_image(100, 100, colortype::RGB8, CompressionMethod::None, vec![]).unwrap();
///
/// let mut idx = 0;
/// while image.next_strip_sample_count() > 0 {
///     let sample_count = image.next_strip_sample_count() as usize;
///     image.write_strip(&image_data.get_slice(idx, idx+sample_count)).unwrap();
///     idx += sample_count;
/// }
/// image.finish().unwrap();
/// # }
/// ```
pub struct ImageEncoder<'a, W: 'a + Write + Seek> {
    encoder: DirectoryEncoder<'a, W>,
    strip_idx: u64,
    strip_count: u64,
    row_samples: u64,
    height: u32,
    rows_per_strip: u64,
    strip_offsets: Vec<u32>,
    strip_byte_count: Vec<u32>,
    dropped: bool,
    compression_method: CompressionMethod,
}

impl<'a, W: 'a + Write + Seek> ImageEncoder<'a, W> {
    fn new(
        mut encoder: DirectoryEncoder<'a, W>,
        width: u32,
        height: u32,
        colortype: ColorType,
        compression_method: CompressionMethod,
        additional_tags: Vec<(Tag, Value)>
    ) -> TiffResult<ImageEncoder<'a, W>> {
        let row_samples = u64::from(width) * u64::try_from(colortype.bit_per_sample.len())?;
        let row_bytes = row_samples * u64::from(colortype.value.byte_len());

        // As per tiff spec each strip should be about 8k long
        let rows_per_strip = (8000 + row_bytes - 1) / row_bytes;

        let strip_count = (u64::from(height) + rows_per_strip - 1) / rows_per_strip;

        encoder.write_tag(Tag::ImageWidth, Value::from(width))?;
        encoder.write_tag(Tag::ImageLength, Value::from(height))?;
        encoder.write_tag(Tag::Compression, Value::from(compression_method.to_u16()))?;

        encoder.write_tag(Tag::BitsPerSample, Value::from(colortype.bit_per_sample))?;
        encoder.write_tag(Tag::PhotometricInterpretation, Value::from(colortype.tiff_value.to_u16()))?;

        encoder.write_tag(Tag::RowsPerStrip, Value::from(u32::try_from(rows_per_strip)?))?;

        encoder.write_tag(Tag::SamplesPerPixel, Value::from(u16::try_from(colortype.bit_per_sample.len())?))?;
        encoder.write_tag(Tag::XResolution, Value::from(Rational { n: 1, d: 1 }))?;
        encoder.write_tag(Tag::YResolution, Value::from(Rational { n: 1, d: 1 }))?;
        encoder.write_tag(Tag::ResolutionUnit, Value::from(ResolutionUnit::None.to_u16()))?;

        for (t, v) in additional_tags {
            encoder.write_tag(t, v)?;
        }

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
            compression_method,
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
    pub fn write_strip<T>(&mut self, value: &T) -> TiffResult<()>
    where
        T: TiffValue,
    {
        // TODO: Compression
        let samples = self.next_strip_sample_count();
        if u64::from(value.count()) != samples {
            return Err(::std::io::Error::new(
                ::std::io::ErrorKind::InvalidData,
                "Slice is wrong size for strip").into());
        }

        let offset = match self.compression_method {
            CompressionMethod::LZW => {
                let mut buf = TiffWriter::new(Cursor::new(Vec::new()));
                value.write(&mut buf)?;
                let mut compressed = vec![];
                {
                    let mut lzw_encoder = EncoderTIFF::new(MsbWriter::new(&mut compressed), 8)?;
                    lzw_encoder.encode_bytes(&buf.writer.into_inner()[..])?;
                }
                self.encoder.write_data(&Value::from(&compressed[..]))?
            },
            _ => {
                self.encoder.write_data(value)?
            }

        };

        self.strip_offsets.push(u32::try_from(offset)?);
        self.strip_byte_count.push(value.bytes());

        self.strip_idx += 1;
        Ok(())
    }

    /// Set image resolution
    pub fn resolution(&mut self, unit: ResolutionUnit, value: Rational) {
        self.encoder.write_tag(Tag::ResolutionUnit, Value::from(unit.to_u16())).unwrap();
        self.encoder.write_tag(Tag::XResolution, Value::from(value)).unwrap();
        self.encoder.write_tag(Tag::YResolution, Value::from(value)).unwrap();
    }

    /// Set image resolution unit
    pub fn resolution_unit(&mut self, unit: ResolutionUnit) {
        self.encoder.write_tag(Tag::ResolutionUnit, Value::from(unit.to_u16())).unwrap();
    }

    /// Set image x-resolution
    pub fn x_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::XResolution, Value::from(value)).unwrap();
    }

    /// Set image y-resolution
    pub fn y_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::YResolution, Value::from(value)).unwrap();
    }

    fn finish_internal(&mut self) -> TiffResult<()> {
        self.encoder
            .write_tag(Tag::StripOffsets, Value::from(&self.strip_offsets[..]))?;
        self.encoder
            .write_tag(Tag::StripByteCounts, Value::from(&self.strip_byte_count[..]))?;
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

impl<'a, W: Write + Seek> Drop for ImageEncoder<'a, W> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}
