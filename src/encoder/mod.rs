pub use tiff_value::*;

use std::io::{Cursor, Read};
use std::{
    cmp,
    io::{self, Seek, Write},
    marker::PhantomData,
    mem,
};

use crate::{
    decoder::GenericTiffDecoder,
    error::{TiffResult, UsageError},
    ifd::{BufferedEntry, Directory},
    tags::{CompressionMethod, ResolutionUnit, SampleFormat, Tag, EXIF_TAGS},
    TiffError, TiffFormatError, TiffKind, TiffKindBig, TiffKindStandard,
};

pub mod colortype;
pub mod compression;
mod tiff_value;
mod writer;

use self::colortype::*;
use self::compression::Compression as Comp;
use self::compression::*;
pub use self::writer::*;

/// Type of prediction to prepare the image with.
///
/// Image data can be very unpredictable, and thus very hard to compress. Predictors are simple
/// passes ran over the image data to prepare it for compression. This is mostly used for LZW
/// compression, where using [Predictor::Horizontal] we see a 35% improvement in compression
/// ratio over the unpredicted compression !
///
/// [Predictor::FloatingPoint] is currently not supported.
pub type Predictor = crate::tags::Predictor;
pub type DeflateLevel = compression::DeflateLevel;

pub type TiffEncoder<W> = GenericTiffEncoder<W, TiffKindStandard>;
pub type BigTiffEncoder<W> = GenericTiffEncoder<W, TiffKindBig>;

#[derive(Clone, Copy, PartialEq)]
pub enum Compression {
    Uncompressed,
    Lzw,
    Deflate(DeflateLevel),
    Packbits,
}

impl Default for Compression {
    fn default() -> Self {
        Self::Uncompressed
    }
}

impl Compression {
    fn tag(&self) -> CompressionMethod {
        match self {
            Compression::Uncompressed => CompressionMethod::None,
            Compression::Lzw => CompressionMethod::LZW,
            Compression::Deflate(_) => CompressionMethod::Deflate,
            Compression::Packbits => CompressionMethod::PackBits,
        }
    }

    fn get_algorithm(&self) -> Compressor {
        match self {
            Compression::Uncompressed => compression::Uncompressed {}.get_algorithm(),
            Compression::Lzw => compression::Lzw {}.get_algorithm(),
            Compression::Deflate(level) => compression::Deflate::with_level(*level).get_algorithm(),
            Compression::Packbits => compression::Packbits {}.get_algorithm(),
        }
    }
}

/// Encoder for Tiff and BigTiff files.
///
/// With this type you can get a `DirectoryEncoder` or a `ImageEncoder`
/// to encode Tiff/BigTiff ifd directories with images.
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
/// // create a standard Tiff file
/// let mut tiff = GenericTiffEncoder::<_, tiff::TiffKindStandard>::new(&mut file).unwrap();
/// tiff.write_image::<colortype::RGB8>(100, 100, &image_data).unwrap();
///
/// // create a BigTiff file
/// let mut bigtiff = GenericTiffEncoder::<_, tiff::TiffKindBig>::new(&mut file).unwrap();
/// bigtiff.write_image::<colortype::RGB8>(100, 100, &image_data).unwrap();
///
/// # }
/// ```
pub struct GenericTiffEncoder<W, K: TiffKind> {
    writer: TiffWriter<W>,
    kind: PhantomData<K>,
    predictor: Predictor,
    compression: Compression,
}

/// Generic functions that are available for both Tiff and BigTiff encoders.
impl<W: Write + Seek, K: TiffKind> GenericTiffEncoder<W, K> {
    /// Creates a new Tiff or BigTiff encoder, inferred from the return type.
    pub fn new(writer: W) -> TiffResult<Self> {
        let mut encoder = GenericTiffEncoder {
            writer: TiffWriter::new(writer),
            kind: PhantomData,
            predictor: Predictor::None,
            compression: Compression::Uncompressed,
        };

        K::write_header(&mut encoder.writer)?;

        Ok(encoder)
    }

    /// Set the predictor to use
    ///
    /// A predictor is used to simplify the file before writing it. This is very
    /// useful when writing a file compressed using LZW as it can improve efficiency
    pub fn with_predictor(mut self, predictor: Predictor) -> Self {
        self.predictor = predictor;

        self
    }

    /// Set the compression method to use
    pub fn with_compression(mut self, compression: Compression) -> Self {
        self.compression = compression;

        self
    }

    /// Create a [`DirectoryEncoder`] to encode an ifd directory.
    pub fn new_directory(&mut self) -> TiffResult<DirectoryEncoder<W, K>> {
        DirectoryEncoder::<W, K>::new(&mut self.writer)
    }

    /// Create an [`ImageEncoder`] to encode an image one slice at a time.
    pub fn new_image<C: ColorType>(
        &mut self,
        width: u32,
        height: u32,
    ) -> TiffResult<ImageEncoder<W, C, K>> {
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        ImageEncoder::new(encoder, width, height, self.compression, self.predictor)
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
        let encoder = DirectoryEncoder::new(&mut self.writer)?;
        let image: ImageEncoder<W, C, K> =
            ImageEncoder::new(encoder, width, height, self.compression, self.predictor)?;
        image.write_data(data)
    }
}

/// Low level interface to encode ifd directories.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
pub struct DirectoryEncoder<'a, W: 'a + Write + Seek, K: TiffKind> {
    writer: &'a mut TiffWriter<W>,
    dropped: bool,
    ifd_pointer_pos: u64,
    ifd: Directory<BufferedEntry>,
    sub_ifd: Option<Directory<BufferedEntry>>,
    _phantom: ::std::marker::PhantomData<K>,
}

impl<'a, W: 'a + Write + Seek, K: TiffKind> DirectoryEncoder<'a, W, K> {
    fn new(writer: &'a mut TiffWriter<W>) -> TiffResult<Self> {
        // the previous word is the IFD offset position
        let ifd_pointer_pos = writer.offset() - mem::size_of::<K::OffsetType>() as u64;
        writer.pad_word_boundary()?; // TODO: Do we need to adjust this for BigTiff?
        Ok(DirectoryEncoder::<W, K> {
            writer,
            dropped: false,
            ifd_pointer_pos,
            ifd: Directory::new(),
            sub_ifd: None,
            _phantom: ::std::marker::PhantomData,
        })
    }

    /// Start writing to sub-IFD
    pub fn subdirectory_start(&mut self) {
        self.sub_ifd = Some(Directory::new());
    }

    /// Stop writing to sub-IFD and resume master IFD, returns offset of sub-IFD
    pub fn subirectory_close(&mut self) -> TiffResult<u64> {
        let offset = self.write_directory()?;
        K::write_offset(self.writer, 0)?;
        self.sub_ifd = None;
        Ok(offset)
    }

    /// Write a single ifd tag.
    pub fn write_tag<T: TiffValue>(&mut self, tag: Tag, value: T) -> TiffResult<()> {
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
            tag,
            BufferedEntry {
                type_: value.is_type(),
                count: value.count().try_into()?,
                data: bytes,
            },
        );

        Ok(())
    }

    fn write_directory(&mut self) -> TiffResult<u64> {
        let active_ifd = match &self.sub_ifd {
            None => &mut self.ifd,
            Some(_v) => self.sub_ifd.as_mut().unwrap(),
        };

        // Start by writing out all values
        for &mut BufferedEntry {
            data: ref mut bytes,
            ..
        } in active_ifd.values_mut()
        {
            let data_bytes = K::OffsetType::BYTE_LEN as usize;

            if bytes.len() > data_bytes {
                let offset = self.writer.offset();
                self.writer.write_bytes(bytes)?;
                *bytes = vec![0; data_bytes];
                let mut writer = TiffWriter::new(bytes as &mut [u8]);
                K::write_offset(&mut writer, offset)?;
            } else {
                while bytes.len() < data_bytes {
                    bytes.push(0);
                }
            }
        }

        let offset = self.writer.offset();

        K::write_entry_count(self.writer, active_ifd.len())?;
        for (
            tag,
            BufferedEntry {
                type_: field_type,
                count,
                data: offset,
            },
        ) in active_ifd.iter()
        {
            self.writer.write_u16(tag.to_u16())?;
            self.writer.write_u16(field_type.to_u16())?;
            K::convert_offset(*count)?.write(self.writer)?;
            self.writer.write_bytes(&offset)?;
        }

        Ok(offset)
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
    fn last_written(&self) -> u64 {
        self.writer.last_written()
    }

    fn finish_internal(&mut self) -> TiffResult<()> {
        if self.sub_ifd.is_some() {
            self.subirectory_close()?;
        }
        let ifd_pointer = self.write_directory()?;
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
/// use tiff::tags::Tag;
///
/// let mut tiff = GenericTiffEncoder::<_, tiff::TiffKindStandard>::new(&mut file).unwrap();
/// let mut image = tiff.new_image::<colortype::RGB8>(100, 100).unwrap();
///
/// // You can encode tags here
/// image.encoder().write_tag(Tag::Artist, "Image-tiff").unwrap();
///
/// // Strip size can be configured before writing data
/// image.rows_per_strip(2).unwrap();
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
/// You can also call write_data function wich will encode by strip and finish
pub struct ImageEncoder<'a, W: 'a + Write + Seek, C: ColorType, K: TiffKind> {
    encoder: DirectoryEncoder<'a, W, K>,
    strip_idx: u64,
    strip_count: u64,
    row_samples: u64,
    width: u32,
    height: u32,
    rows_per_strip: u64,
    strip_offsets: Vec<K::OffsetType>,
    strip_byte_count: Vec<K::OffsetType>,
    dropped: bool,
    compression: Compression,
    predictor: Predictor,
    _phantom: ::std::marker::PhantomData<C>,
}

impl<'a, W: 'a + Write + Seek, T: ColorType, K: TiffKind> ImageEncoder<'a, W, T, K> {
    fn sanity_check(compression: Compression, predictor: Predictor) -> TiffResult<()> {
        match (predictor, compression, T::SAMPLE_FORMAT[0]) {
            (Predictor::Horizontal, _, SampleFormat::IEEEFP | SampleFormat::Void) => {
                Err(TiffError::UsageError(UsageError::PredictorIncompatible))
            }
            (Predictor::FloatingPoint, _, _) => {
                Err(TiffError::UsageError(UsageError::PredictorUnavailable))
            }
            _ => Ok(()),
        }
    }

    fn new(
        mut encoder: DirectoryEncoder<'a, W, K>,
        width: u32,
        height: u32,
        compression: Compression,
        predictor: Predictor,
    ) -> TiffResult<Self> {
        if width == 0 || height == 0 {
            return Err(TiffError::FormatError(TiffFormatError::InvalidDimensions(
                width, height,
            )));
        }

        Self::sanity_check(compression, predictor)?;

        let row_samples = u64::from(width) * u64::try_from(<T>::BITS_PER_SAMPLE.len())?;
        let row_bytes = row_samples * u64::from(<T::Inner>::BYTE_LEN);

        // Limit the strip size to prevent potential memory and security issues.
        // Also keep the multiple strip handling 'oiled'
        let rows_per_strip = {
            match compression.tag() {
                CompressionMethod::PackBits => 1, // Each row must be packed separately. Do not compress across row boundaries
                _ => (1_000_000 + row_bytes - 1) / row_bytes,
            }
        };

        let strip_count = (u64::from(height) + rows_per_strip - 1) / rows_per_strip;

        encoder.write_tag(Tag::ImageWidth, width)?;
        encoder.write_tag(Tag::ImageLength, height)?;
        encoder.write_tag(Tag::Compression, compression.tag().to_u16())?;
        encoder.write_tag(Tag::Predictor, predictor.to_u16())?;

        encoder.write_tag(Tag::BitsPerSample, <T>::BITS_PER_SAMPLE)?;
        let sample_format: Vec<_> = <T>::SAMPLE_FORMAT.iter().map(|s| s.to_u16()).collect();
        encoder.write_tag(Tag::SampleFormat, &sample_format[..])?;
        encoder.write_tag(Tag::PhotometricInterpretation, <T>::TIFF_VALUE.to_u16())?;

        encoder.write_tag(Tag::RowsPerStrip, u32::try_from(rows_per_strip)?)?;

        encoder.write_tag(
            Tag::SamplesPerPixel,
            u16::try_from(<T>::BITS_PER_SAMPLE.len())?,
        )?;
        encoder.write_tag(Tag::XResolution, Rational { n: 1, d: 1 })?;
        encoder.write_tag(Tag::YResolution, Rational { n: 1, d: 1 })?;
        encoder.write_tag(Tag::ResolutionUnit, ResolutionUnit::None.to_u16())?;

        Ok(ImageEncoder {
            encoder,
            strip_count,
            strip_idx: 0,
            row_samples,
            rows_per_strip,
            width,
            height,
            strip_offsets: Vec::new(),
            strip_byte_count: Vec::new(),
            dropped: false,
            compression,
            predictor,
            _phantom: ::std::marker::PhantomData,
        })
    }

    /// Number of samples the next strip should have.
    pub fn next_strip_sample_count(&self) -> u64 {
        if self.strip_idx >= self.strip_count {
            return 0;
        }

        let raw_start_row = self.strip_idx * self.rows_per_strip;
        let start_row = cmp::min(u64::from(self.height), raw_start_row);
        let end_row = cmp::min(u64::from(self.height), raw_start_row + self.rows_per_strip);

        (end_row - start_row) * self.row_samples
    }

    /// Write a single strip.
    pub fn write_strip(&mut self, value: &[T::Inner]) -> TiffResult<()>
    where
        [T::Inner]: TiffValue,
    {
        let samples = self.next_strip_sample_count();
        if u64::try_from(value.len())? != samples {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Slice is wrong size for strip",
            )
            .into());
        }

        // Write the (possible compressed) data to the encoder.
        let offset = match self.predictor {
            Predictor::None => self.encoder.write_data(value)?,
            Predictor::Horizontal => {
                let mut row_result = Vec::with_capacity(value.len());
                for row in value.chunks_exact(self.row_samples as usize) {
                    T::horizontal_predict(row, &mut row_result);
                }
                self.encoder.write_data(row_result.as_slice())?
            }
            _ => unimplemented!(),
        };

        let byte_count = self.encoder.last_written() as usize;

        self.strip_offsets.push(K::convert_offset(offset)?);
        self.strip_byte_count.push(byte_count.try_into()?);

        self.strip_idx += 1;
        Ok(())
    }

    /// Write strips from data
    pub fn write_data(mut self, data: &[T::Inner]) -> TiffResult<()>
    where
        [T::Inner]: TiffValue,
    {
        let num_pix = usize::try_from(self.width)?
            .checked_mul(usize::try_from(self.height)?)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Image width * height exceeds usize",
                )
            })?;
        if data.len() < num_pix {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Input data slice is undersized for provided dimensions",
            )
            .into());
        }

        self.encoder
            .writer
            .set_compression(self.compression.get_algorithm());

        let mut idx = 0;
        while self.next_strip_sample_count() > 0 {
            let sample_count = usize::try_from(self.next_strip_sample_count())?;
            self.write_strip(&data[idx..idx + sample_count])?;
            idx += sample_count;
        }

        self.encoder.writer.reset_compression();
        self.finish()?;
        Ok(())
    }

    /// Set image resolution
    pub fn resolution(&mut self, unit: ResolutionUnit, value: Rational) {
        self.encoder
            .write_tag(Tag::ResolutionUnit, unit.to_u16())
            .unwrap();
        self.encoder
            .write_tag(Tag::XResolution, value.clone())
            .unwrap();
        self.encoder.write_tag(Tag::YResolution, value).unwrap();
    }

    /// Set image resolution unit
    pub fn resolution_unit(&mut self, unit: ResolutionUnit) {
        self.encoder
            .write_tag(Tag::ResolutionUnit, unit.to_u16())
            .unwrap();
    }

    /// Set image x-resolution
    pub fn x_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::XResolution, value).unwrap();
    }

    /// Set image y-resolution
    pub fn y_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::YResolution, value).unwrap();
    }

    /// Write Exif data from TIFF encoded byte block
    pub fn exif_tags<F: TiffKind>(&mut self, source: Vec<u8>) -> TiffResult<()> {
        let mut decoder = GenericTiffDecoder::<_, F>::new(Cursor::new(source))?;

        // copy Exif tags to main IFD
        let exif_tags = EXIF_TAGS;
        exif_tags.into_iter().for_each(|tag| {
            let entry = decoder.find_tag_entry(tag);
            if entry.is_some() && !self.encoder.ifd.contains_key(&tag) {
                let b_entry = entry.unwrap().as_buffered(decoder.inner()).unwrap();
                self.encoder.write_tag(tag, b_entry).unwrap();
            }
        });

        // copy sub-ifds
        self.copy_ifd(Tag::ExifIfd, &mut decoder)?;
        self.copy_ifd(Tag::GpsIfd, &mut decoder)?;
        self.copy_ifd(Tag::InteropIfd, &mut decoder)?;

        Ok(())
    }

    fn copy_ifd<R: Read + Seek, F: TiffKind>(
        &mut self,
        tag: Tag,
        decoder: &mut GenericTiffDecoder<R, F>,
    ) -> TiffResult<()> {
        let exif_ifd_offset = decoder.find_tag(tag)?;
        if exif_ifd_offset.is_some() {
            let offset = exif_ifd_offset.unwrap().into_u32()?.into();

            // create sub-ifd
            self.encoder.subdirectory_start();

            let (ifd, _trash1) = GenericTiffDecoder::<_, F>::read_ifd(decoder.inner(), offset)?;

            // loop through entries
            ifd.into_iter().for_each(|(tag, value)| {
                let b_entry = value.as_buffered(decoder.inner()).unwrap();
                self.encoder.write_tag(tag, b_entry).unwrap();
            });

            // return to ifd0 and write offset
            let ifd_offset = self.encoder.subirectory_close()?;
            self.encoder.write_tag(tag, ifd_offset as u32)?;
        }

        Ok(())
    }

    /// Set image number of lines per strip
    ///
    /// This function needs to be called before any calls to `write_data` or
    /// `write_strip` and will return an error otherwise.
    pub fn rows_per_strip(&mut self, value: u32) -> TiffResult<()> {
        if self.strip_idx != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot change strip size after data was written",
            )
            .into());
        }
        // Write tag as 32 bits
        self.encoder.write_tag(Tag::RowsPerStrip, value)?;

        let value: u64 = value as u64;
        self.strip_count = (self.height as u64 + value - 1) / value;
        self.rows_per_strip = value;

        Ok(())
    }

    fn finish_internal(&mut self) -> TiffResult<()> {
        self.encoder
            .write_tag(Tag::StripOffsets, K::convert_slice(&self.strip_offsets))?;
        self.encoder.write_tag(
            Tag::StripByteCounts,
            K::convert_slice(&self.strip_byte_count),
        )?;
        self.dropped = true;

        self.encoder.finish_internal()
    }

    /// Get a reference of the underlying `DirectoryEncoder`
    pub fn encoder(&mut self) -> &mut DirectoryEncoder<'a, W, K> {
        &mut self.encoder
    }

    /// Write out image and ifd directory.
    pub fn finish(mut self) -> TiffResult<()> {
        self.finish_internal()
    }
}

impl<'a, W: Write + Seek, C: ColorType, K: TiffKind> Drop for ImageEncoder<'a, W, C, K> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}
