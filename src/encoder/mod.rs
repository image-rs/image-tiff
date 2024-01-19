pub mod colortype;
pub mod compression;
mod directory_encoder;
mod tiff_value;
mod writer;

pub use self::writer::*;
use self::{
    colortype::*,
    compression::{Compression as Comp, *},
};
use crate::{
    decoder::GenericTiffDecoder,
    error::{TiffResult, UsageError},
    ifd::{BufferedEntry, Directory, ImageFileDirectory, ProcessedEntry, Value},
    tags::{CompressionMethod, GpsTag, ResolutionUnit, SampleFormat, Tag},
    TiffError, TiffFormatError, TiffKind, TiffKindBig, TiffKindStandard,
};
pub use directory_encoder::DirectoryEncoder;
use std::{
    cmp,
    io::{self, Cursor, Read, Seek, Write},
    marker::PhantomData,
};
pub use tiff_value::*;

/// Type of prediction to prepare the image with.
///
/// Image data can be very unpredictable, and thus very hard to compress. Predictors are simple
/// passes ran over the image data to prepare it for compression. This is mostly used for LZW
/// compression, where using [Predictor::Horizontal] we see a 35% improvement in compression
/// ratio over the unpredicted compression !
///
/// [Predictor::FloatingPoint] is currently not supported.
pub type Predictor = crate::tags::Predictor;
#[cfg(feature = "deflate")]
pub type DeflateLevel = compression::DeflateLevel;

pub type TiffEncoder<W> = GenericTiffEncoder<W, TiffKindStandard>;
pub type BigTiffEncoder<W> = GenericTiffEncoder<W, TiffKindBig>;

#[derive(Clone, Copy, PartialEq)]
pub enum Compression {
    Uncompressed,
    #[cfg(feature = "lzw")]
    Lzw,
    #[cfg(feature = "deflate")]
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
            #[cfg(feature = "lzw")]
            Compression::Lzw => CompressionMethod::LZW,
            #[cfg(feature = "deflate")]
            Compression::Deflate(_) => CompressionMethod::Deflate,
            Compression::Packbits => CompressionMethod::PackBits,
        }
    }

    fn get_algorithm(&self) -> Compressor {
        match self {
            Compression::Uncompressed => compression::Uncompressed {}.get_algorithm(),
            #[cfg(feature = "lzw")]
            Compression::Lzw => compression::Lzw {}.get_algorithm(),
            #[cfg(feature = "deflate")]
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
    exif: Directory<BufferedEntry>,
    gps: Option<ImageFileDirectory<GpsTag, BufferedEntry>>,
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
            exif: Directory::new(),
            gps: None,
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

    /// Set EXIF fields to write
    pub fn with_exif<E>(mut self, exif: Directory<E>) -> Self
    where
        E: Into<BufferedEntry>,
    {
        self.exif = exif.into_iter().map(|(k, v)| (k, v.into())).collect();

        self
    }

    /// Set EXIF fields to write
    pub fn with_gps<E>(mut self, exif: ImageFileDirectory<GpsTag, E>) -> Self
    where
        E: Into<BufferedEntry>,
    {
        self.gps = Some(exif.into_iter().map(|(k, v)| (k, v.into())).collect());

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
        ImageEncoder::new_with_exif(
            &mut self.writer,
            width,
            height,
            self.compression,
            self.predictor,
            &self.exif,
            &self.gps,
        )
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
        let image: ImageEncoder<W, C, K> = ImageEncoder::new_with_exif(
            &mut self.writer,
            width,
            height,
            self.compression,
            self.predictor,
            &self.exif,
            &self.gps,
        )?;
        image.write_data(data)
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
        writer: &'a mut TiffWriter<W>,
        width: u32,
        height: u32,
        compression: Compression,
        predictor: Predictor,
    ) -> TiffResult<Self> {
        Self::new_with_raw_exif(
            writer,
            width,
            height,
            compression,
            predictor,
            Self::default_exif(width, height, compression, predictor)?,
            None,
        )
    }

    /// Create a new [ImageEncoder] with a given EXIF block whose contents will be validated
    fn new_with_exif(
        writer: &'a mut TiffWriter<W>,
        width: u32,
        height: u32,
        compression: Compression,
        predictor: Predictor,
        exif: &Directory<BufferedEntry>,
        gps: &Option<ImageFileDirectory<GpsTag, BufferedEntry>>,
    ) -> TiffResult<Self> {
        let mut exif = exif.clone();
        for (tag, value) in Self::default_exif(width, height, compression, predictor)?.into_iter() {
            exif.insert(tag, value);
        }

        Self::new_with_raw_exif(
            writer,
            width,
            height,
            compression,
            predictor,
            exif.clone(),
            gps.clone(),
        )
    }

    /// Create a new [ImageEncoder] with unchecked EXIF block
    fn new_with_raw_exif(
        writer: &'a mut TiffWriter<W>,
        width: u32,
        height: u32,
        compression: Compression,
        predictor: Predictor,
        mut exif: Directory<BufferedEntry>,
        exif_gps: Option<ImageFileDirectory<GpsTag, BufferedEntry>>,
    ) -> TiffResult<Self> {
        let mut encoder = DirectoryEncoder::new(writer)?;

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

        if let Some(data) = exif_gps {
            encoder.subdirectory_start();
            encoder.write_exif(&data)?;
            let gps_ifd_offset = encoder.subdirectory_close()?;

            exif.insert(
                Tag::GpsIfd,
                ProcessedEntry::new(
                    K::is_big()
                        .then_some(Value::IfdBig(gps_ifd_offset))
                        .unwrap_or(Value::Ifd(gps_ifd_offset as u32)),
                ),
            );
        }

        encoder.write_exif(&exif)?;

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

    fn default_exif(
        width: u32,
        height: u32,
        compression: Compression,
        predictor: Predictor,
    ) -> TiffResult<Directory<BufferedEntry>> {
        let sample_format: Vec<_> = <T>::SAMPLE_FORMAT.iter().map(|s| s.to_u16()).collect();
        // Limit the strip size to prevent potential memory and security issues.
        // Also keep the multiple strip handling 'oiled'
        let row_samples = width * u32::try_from(<T>::BITS_PER_SAMPLE.len())?;
        let row_bytes = row_samples * u32::from(<T::Inner>::BYTE_LEN);
        let rows_per_strip = {
            match compression.tag() {
                CompressionMethod::PackBits => 1, // Each row must be packed separately. Do not compress across row boundaries
                _ => (1_000_000 + row_bytes - 1) / row_bytes,
            }
        };

        let mut exif = Directory::<BufferedEntry>::new();

        exif.insert(Tag::ImageWidth, ProcessedEntry::new(Value::Unsigned(width)));

        exif.insert(
            Tag::ImageLength,
            ProcessedEntry::new(Value::Unsigned(height)),
        );

        exif.insert(
            Tag::Compression,
            ProcessedEntry::new(Value::Short(compression.tag().to_u16())),
        );

        exif.insert(
            Tag::Predictor,
            ProcessedEntry::new(Value::Short(predictor.to_u16())),
        );

        exif.insert(
            Tag::BitsPerSample,
            ProcessedEntry::new_vec(
                &<T>::BITS_PER_SAMPLE
                    .iter()
                    .map(|b| Value::Short(*b))
                    .collect::<Vec<_>>(),
            ),
        );

        exif.insert(
            Tag::SamplesPerPixel,
            ProcessedEntry::new(Value::Short(u16::try_from(<T>::BITS_PER_SAMPLE.len())?)),
        );

        exif.insert(
            Tag::SampleFormat,
            ProcessedEntry::new_vec(
                &sample_format[..]
                    .iter()
                    .map(|s| Value::Short(*s))
                    .collect::<Vec<_>>(),
            ),
        );

        exif.insert(
            Tag::PhotometricInterpretation,
            ProcessedEntry::new(Value::Short(<T>::TIFF_VALUE.to_u16())),
        );

        exif.insert(
            Tag::RowsPerStrip,
            ProcessedEntry::new(Value::Unsigned(rows_per_strip)),
        );

        exif.insert(Tag::XResolution, ProcessedEntry::new(Value::Rational(1, 1)));

        exif.insert(Tag::YResolution, ProcessedEntry::new(Value::Rational(1, 1)));

        exif.insert(
            Tag::ResolutionUnit,
            ProcessedEntry::new(Value::Short(ResolutionUnit::None.to_u16())),
        );

        Ok(exif)
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

    pub fn set_exif_tag<E: TiffValue>(&mut self, tag: Tag, value: E) -> TiffResult<()> {
        self.encoder.write_tag(tag, value)
    }

    pub fn set_exif_tags<E: TiffValue>(&mut self, ifd: Directory<E>) -> TiffResult<()> {
        for (tag, value) in ifd.into_iter() {
            self.encoder.write_tag(tag, value)?;
        }

        Ok(())
    }

    /// Write Exif data from TIFF encoded byte block
    pub fn set_raw_exif_tags<F: TiffKind>(&mut self, source: Vec<u8>) -> TiffResult<()> {
        let mut decoder = GenericTiffDecoder::<_, F>::new(Cursor::new(source))?;

        for (t, e) in decoder.get_exif_data()?.into_iter() {
            if !self.encoder.contains(&t) {
                self.encoder.write_tag(t, e)?;
            }
        }

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
            let ifd_offset = self.encoder.subdirectory_close()?;
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
