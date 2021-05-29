use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::{self, Read, Seek};
use std::{cmp, ops::Range};

use crate::{
    bytecast, ColorType, TiffError, TiffFormatError, TiffResult, TiffUnsupportedError, UsageError,
};

use self::ifd::Directory;
use crate::tags::{
    CompressionMethod, PhotometricInterpretation, Predictor, SampleFormat, Tag, Type,
};

use self::stream::{
    ByteOrder, DeflateReader, EndianReader, JpegReader, LZWReader, PackBitsReader, SmartReader,
};

pub mod ifd;
mod stream;

/// Result of a decoding process
#[derive(Debug)]
pub enum DecodingResult {
    /// A vector of unsigned bytes
    U8(Vec<u8>),
    /// A vector of unsigned words
    U16(Vec<u16>),
    /// A vector of 32 bit unsigned ints
    U32(Vec<u32>),
    /// A vector of 64 bit unsigned ints
    U64(Vec<u64>),
    /// A vector of 32 bit IEEE floats
    F32(Vec<f32>),
    /// A vector of 64 bit IEEE floats
    F64(Vec<f64>),
    /// A vector of 8 bit signed ints
    I8(Vec<i8>),
    /// A vector of 16 bit signed ints
    I16(Vec<i16>),
    /// A vector of 32 bit signed ints
    I32(Vec<i32>),
    /// A vector of 64 bit signed ints
    I64(Vec<i64>),
}

impl DecodingResult {
    fn new_u8(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::U8(vec![0; size]))
        }
    }

    fn new_u16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / 2 {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::U16(vec![0; size]))
        }
    }

    fn new_u32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / 4 {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::U32(vec![0; size]))
        }
    }

    fn new_u64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / 8 {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::U64(vec![0; size]))
        }
    }

    fn new_f32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / std::mem::size_of::<f32>() {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::F32(vec![0.0; size]))
        }
    }

    fn new_f64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / std::mem::size_of::<f64>() {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::F64(vec![0.0; size]))
        }
    }

    fn new_i8(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / std::mem::size_of::<i8>() {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::I8(vec![0; size]))
        }
    }

    fn new_i16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / 2 {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::I16(vec![0; size]))
        }
    }

    fn new_i32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / 4 {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::I32(vec![0; size]))
        }
    }

    fn new_i64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / 8 {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::I64(vec![0; size]))
        }
    }

    pub fn as_buffer(&mut self, start: usize) -> DecodingBuffer {
        match *self {
            DecodingResult::U8(ref mut buf) => DecodingBuffer::U8(&mut buf[start..]),
            DecodingResult::U16(ref mut buf) => DecodingBuffer::U16(&mut buf[start..]),
            DecodingResult::U32(ref mut buf) => DecodingBuffer::U32(&mut buf[start..]),
            DecodingResult::U64(ref mut buf) => DecodingBuffer::U64(&mut buf[start..]),
            DecodingResult::F32(ref mut buf) => DecodingBuffer::F32(&mut buf[start..]),
            DecodingResult::F64(ref mut buf) => DecodingBuffer::F64(&mut buf[start..]),
            DecodingResult::I8(ref mut buf) => DecodingBuffer::I8(&mut buf[start..]),
            DecodingResult::I16(ref mut buf) => DecodingBuffer::I16(&mut buf[start..]),
            DecodingResult::I32(ref mut buf) => DecodingBuffer::I32(&mut buf[start..]),
            DecodingResult::I64(ref mut buf) => DecodingBuffer::I64(&mut buf[start..]),
        }
    }
}

// A buffer for image decoding
pub enum DecodingBuffer<'a> {
    /// A slice of unsigned bytes
    U8(&'a mut [u8]),
    /// A slice of unsigned words
    U16(&'a mut [u16]),
    /// A slice of 32 bit unsigned ints
    U32(&'a mut [u32]),
    /// A slice of 64 bit unsigned ints
    U64(&'a mut [u64]),
    /// A slice of 32 bit IEEE floats
    F32(&'a mut [f32]),
    /// A slice of 64 bit IEEE floats
    F64(&'a mut [f64]),
    /// A slice of 8 bits signed ints
    I8(&'a mut [i8]),
    /// A slice of 16 bits signed ints
    I16(&'a mut [i16]),
    /// A slice of 32 bits signed ints
    I32(&'a mut [i32]),
    /// A slice of 64 bits signed ints
    I64(&'a mut [i64]),
}

impl<'a> DecodingBuffer<'a> {
    fn len(&self) -> usize {
        match *self {
            DecodingBuffer::U8(ref buf) => buf.len(),
            DecodingBuffer::U16(ref buf) => buf.len(),
            DecodingBuffer::U32(ref buf) => buf.len(),
            DecodingBuffer::U64(ref buf) => buf.len(),
            DecodingBuffer::F32(ref buf) => buf.len(),
            DecodingBuffer::F64(ref buf) => buf.len(),
            DecodingBuffer::I8(ref buf) => buf.len(),
            DecodingBuffer::I16(ref buf) => buf.len(),
            DecodingBuffer::I32(ref buf) => buf.len(),
            DecodingBuffer::I64(ref buf) => buf.len(),
        }
    }

    fn byte_len(&self) -> usize {
        match *self {
            DecodingBuffer::U8(_) => 1,
            DecodingBuffer::U16(_) => 2,
            DecodingBuffer::U32(_) => 4,
            DecodingBuffer::U64(_) => 8,
            DecodingBuffer::F32(_) => 4,
            DecodingBuffer::F64(_) => 8,
            DecodingBuffer::I8(_) => 1,
            DecodingBuffer::I16(_) => 2,
            DecodingBuffer::I32(_) => 4,
            DecodingBuffer::I64(_) => 8,
        }
    }

    fn copy<'b>(&'b mut self) -> DecodingBuffer<'b>
    where
        'a: 'b,
    {
        match *self {
            DecodingBuffer::U8(ref mut buf) => DecodingBuffer::U8(buf),
            DecodingBuffer::U16(ref mut buf) => DecodingBuffer::U16(buf),
            DecodingBuffer::U32(ref mut buf) => DecodingBuffer::U32(buf),
            DecodingBuffer::U64(ref mut buf) => DecodingBuffer::U64(buf),
            DecodingBuffer::F32(ref mut buf) => DecodingBuffer::F32(buf),
            DecodingBuffer::F64(ref mut buf) => DecodingBuffer::F64(buf),
            DecodingBuffer::I8(ref mut buf) => DecodingBuffer::I8(buf),
            DecodingBuffer::I16(ref mut buf) => DecodingBuffer::I16(buf),
            DecodingBuffer::I32(ref mut buf) => DecodingBuffer::I32(buf),
            DecodingBuffer::I64(ref mut buf) => DecodingBuffer::I64(buf),
        }
    }

    fn subrange<'b>(&'b mut self, range: Range<usize>) -> DecodingBuffer<'b>
    where
        'a: 'b,
    {
        match *self {
            DecodingBuffer::U8(ref mut buf) => DecodingBuffer::U8(&mut buf[range]),
            DecodingBuffer::U16(ref mut buf) => DecodingBuffer::U16(&mut buf[range]),
            DecodingBuffer::U32(ref mut buf) => DecodingBuffer::U32(&mut buf[range]),
            DecodingBuffer::U64(ref mut buf) => DecodingBuffer::U64(&mut buf[range]),
            DecodingBuffer::F32(ref mut buf) => DecodingBuffer::F32(&mut buf[range]),
            DecodingBuffer::F64(ref mut buf) => DecodingBuffer::F64(&mut buf[range]),
            DecodingBuffer::I8(ref mut buf) => DecodingBuffer::I8(&mut buf[range]),
            DecodingBuffer::I16(ref mut buf) => DecodingBuffer::I16(&mut buf[range]),
            DecodingBuffer::I32(ref mut buf) => DecodingBuffer::I32(&mut buf[range]),
            DecodingBuffer::I64(ref mut buf) => DecodingBuffer::I64(&mut buf[range]),
        }
    }
}

#[derive(Debug)]
struct StripDecodeState {
    strip_index: usize,
    strip_offsets: Vec<u64>,
    strip_bytes: Vec<u64>,
}

#[derive(Debug)]
/// Computed values useful for tile decoding
struct TileAttributes {
    tile_width: usize,
    tile_length: usize,
    tiles_down: usize,
    tiles_across: usize,
    /// Length of padding for rightmost tiles in pixels
    padding_right: usize,
    /// length of padding for bottommost tile in pixels
    padding_down: usize,
    tile_samples: usize,
    /// Sample count of one row of one tile
    row_samples: usize,
    /// Sample count of one row of tiles
    tile_strip_samples: usize,
}

impl TileAttributes {
    /// Returns the tile offset in the result buffer, counted in samples
    fn get_offset(&self, tile: usize) -> usize {
        let row = tile / self.tiles_across;
        let column = tile % self.tiles_across;

        (row * self.tile_strip_samples) + (column * self.row_samples)
    }

    fn get_padding(&self, tile: usize) -> (usize, usize) {
        let row = tile / self.tiles_across;
        let column = tile % self.tiles_across;

        let padding_right = if column == self.tiles_across - 1 {
            self.padding_right
        } else {
            0
        };

        let padding_down = if row == self.tiles_down - 1 {
            self.padding_down
        } else {
            0
        };

        (padding_right, padding_down)
    }
}

#[derive(Debug)]
/// Stateful variables for tile decoding
struct TileDecodeState {
    current_tile: usize,
    tile_offsets: Vec<u64>,
    tile_bytes: Vec<u64>,
    /// Pixel width of one row of the decoding result (tile / whole image)
    result_width: usize,
}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Chunk type of the internal representation
pub enum ChunkType {
    Strip,
    Tile,
}

/// Decoding limits
#[derive(Clone, Debug)]
pub struct Limits {
    /// The maximum size of any `DecodingResult` in bytes, the default is
    /// 256MiB. If the entire image is decoded at once, then this will
    /// be the maximum size of the image. If it is decoded one strip at a
    /// time, this will be the maximum size of a strip.
    pub decoding_buffer_size: usize,
    /// The maximum size of any ifd value in bytes, the default is
    /// 1MiB.
    pub ifd_value_size: usize,
    /// Maximum size for intermediate buffer which may be used to limit the amount of data read per
    /// segment even if the entire image is decoded at once.
    pub intermediate_buffer_size: usize,
    /// The purpose of this is to prevent all the fields of the struct from
    /// being public, as this would make adding new fields a major version
    /// bump.
    _non_exhaustive: (),
}

impl Limits {
    /// A configuration that does not impose any limits.
    ///
    /// This is a good start if the caller only wants to impose selective limits, contrary to the
    /// default limits which allows selectively disabling limits.
    ///
    /// Note that this configuration is likely to crash on excessively large images since,
    /// naturally, the machine running the program does not have infinite memory.
    pub fn unlimited() -> Limits {
        Limits {
            decoding_buffer_size: usize::max_value(),
            ifd_value_size: usize::max_value(),
            intermediate_buffer_size: usize::max_value(),
            _non_exhaustive: (),
        }
    }
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            decoding_buffer_size: 256 * 1024 * 1024,
            intermediate_buffer_size: 128 * 1024 * 1024,
            ifd_value_size: 1024 * 1024,
            _non_exhaustive: (),
        }
    }
}

/// The representation of a TIFF decoder
///
/// Currently does not support decoding of interlaced images
#[derive(Debug)]
pub struct Decoder<R>
where
    R: Read + Seek,
{
    reader: SmartReader<R>,
    byte_order: ByteOrder,
    bigtiff: bool,
    limits: Limits,
    next_ifd: Option<u64>,
    ifd: Option<Directory>,
    width: u32,
    height: u32,
    bits_per_sample: Vec<u8>,
    samples: u8,
    sample_format: Vec<SampleFormat>,
    photometric_interpretation: PhotometricInterpretation,
    compression_method: CompressionMethod,
    chunk_type: ChunkType,
    strip_decoder: Option<StripDecodeState>,
    tile_decoder: Option<TileDecodeState>,
    tile_attributes: Option<TileAttributes>,
}

trait Wrapping {
    fn wrapping_add(&self, other: Self) -> Self;
}

impl Wrapping for u8 {
    fn wrapping_add(&self, other: Self) -> Self {
        u8::wrapping_add(*self, other)
    }
}

impl Wrapping for u16 {
    fn wrapping_add(&self, other: Self) -> Self {
        u16::wrapping_add(*self, other)
    }
}

impl Wrapping for u32 {
    fn wrapping_add(&self, other: Self) -> Self {
        u32::wrapping_add(*self, other)
    }
}

impl Wrapping for u64 {
    fn wrapping_add(&self, other: Self) -> Self {
        u64::wrapping_add(*self, other)
    }
}

impl Wrapping for i8 {
    fn wrapping_add(&self, other: Self) -> Self {
        i8::wrapping_add(*self, other)
    }
}

impl Wrapping for i16 {
    fn wrapping_add(&self, other: Self) -> Self {
        i16::wrapping_add(*self, other)
    }
}

impl Wrapping for i32 {
    fn wrapping_add(&self, other: Self) -> Self {
        i32::wrapping_add(*self, other)
    }
}

impl Wrapping for i64 {
    fn wrapping_add(&self, other: Self) -> Self {
        i64::wrapping_add(*self, other)
    }
}

fn rev_hpredict_nsamp<T>(
    image: &mut [T],
    size: (u32, u32), // Size of the block
    img_width: usize, // Width of the image (this distinction is needed for tiles)
    samples: usize,
) -> TiffResult<()>
where
    T: Copy + Wrapping,
{
    let width = usize::try_from(size.0)?;
    let height = usize::try_from(size.1)?;
    for row in 0..height {
        for col in samples..width * samples {
            let prev_pixel = image[(row * img_width * samples + col - samples)];
            let pixel = &mut image[(row * img_width * samples + col)];
            *pixel = pixel.wrapping_add(prev_pixel);
        }
    }
    Ok(())
}

fn rev_hpredict(
    image: DecodingBuffer,
    size: (u32, u32),
    img_width: usize,
    color_type: ColorType,
) -> TiffResult<()> {
    // TODO: use bits_per_sample.len() after implementing type 3 predictor
    let samples = match color_type {
        ColorType::Gray(8) | ColorType::Gray(16) | ColorType::Gray(32) | ColorType::Gray(64) => 1,
        ColorType::RGB(8) | ColorType::RGB(16) | ColorType::RGB(32) | ColorType::RGB(64) => 3,
        ColorType::RGBA(8)
        | ColorType::RGBA(16)
        | ColorType::RGBA(32)
        | ColorType::RGBA(64)
        | ColorType::CMYK(8)
        | ColorType::CMYK(16)
        | ColorType::CMYK(32)
        | ColorType::CMYK(64) => 4,
        _ => {
            return Err(TiffError::UnsupportedError(
                TiffUnsupportedError::HorizontalPredictor(color_type),
            ))
        }
    };

    match image {
        DecodingBuffer::U8(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::U16(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::U32(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::U64(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::F32(_buf) => {
            // FIXME: check how this is defined.
            // See issue #89.
            // rev_hpredict_nsamp(buf, size, img_width,samples)?;
            return Err(TiffError::UnsupportedError(
                TiffUnsupportedError::HorizontalPredictor(color_type),
            ));
        }
        DecodingBuffer::F64(_buf) => {
            //FIXME: check how this is defined.
            // See issue #89.
            // rev_hpredict_nsamp(buf, size, img_width,samples)?;
            return Err(TiffError::UnsupportedError(
                TiffUnsupportedError::HorizontalPredictor(color_type),
            ));
        }
        DecodingBuffer::I8(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::I16(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::I32(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
        DecodingBuffer::I64(buf) => {
            rev_hpredict_nsamp(buf, size, img_width, samples)?;
        }
    }
    Ok(())
}

impl<R: Read + Seek> Decoder<R> {
    /// Create a new decoder that decodes from the stream ```r```
    pub fn new(r: R) -> TiffResult<Decoder<R>> {
        Decoder {
            reader: SmartReader::wrap(r, ByteOrder::LittleEndian),
            byte_order: ByteOrder::LittleEndian,
            bigtiff: false,
            limits: Default::default(),
            next_ifd: None,
            ifd: None,
            width: 0,
            height: 0,
            bits_per_sample: vec![1],
            samples: 1,
            sample_format: vec![SampleFormat::Uint],
            photometric_interpretation: PhotometricInterpretation::BlackIsZero,
            compression_method: CompressionMethod::None,
            chunk_type: ChunkType::Strip,
            strip_decoder: None,
            tile_decoder: None,
            tile_attributes: None,
        }
        .init()
    }

    pub fn with_limits(mut self, limits: Limits) -> Decoder<R> {
        self.limits = limits;
        self
    }

    pub fn dimensions(&mut self) -> TiffResult<(u32, u32)> {
        Ok((self.width, self.height))
    }

    pub fn colortype(&mut self) -> TiffResult<ColorType> {
        match self.photometric_interpretation {
            PhotometricInterpretation::RGB => match self.bits_per_sample[..] {
                [r, g, b] if [r, r] == [g, b] => Ok(ColorType::RGB(r)),
                [r, g, b, a] if [r, r, r] == [g, b, a] => Ok(ColorType::RGBA(r)),
                // FIXME: We should _ignore_ other components. In particular:
                // > Beware of extra components. Some TIFF files may have more components per pixel
                // than you think. A Baseline TIFF reader must skip over them gracefully,using the
                // values of the SamplesPerPixel and BitsPerSample fields.
                // > -- TIFF 6.0 Specification, Section 7, Additional Baseline requirements.
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::InterpretationWithBits(
                        self.photometric_interpretation,
                        self.bits_per_sample.clone(),
                    ),
                )),
            },
            PhotometricInterpretation::CMYK => match self.bits_per_sample[..] {
                [c, m, y, k] if [c, c, c] == [m, y, k] => Ok(ColorType::CMYK(c)),
                _ => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::InterpretationWithBits(
                        self.photometric_interpretation,
                        self.bits_per_sample.clone(),
                    ),
                )),
            },
            PhotometricInterpretation::BlackIsZero | PhotometricInterpretation::WhiteIsZero
                if self.bits_per_sample.len() == 1 =>
            {
                Ok(ColorType::Gray(self.bits_per_sample[0]))
            }

            // TODO: this is bad we should not fail at this point
            _ => Err(TiffError::UnsupportedError(
                TiffUnsupportedError::InterpretationWithBits(
                    self.photometric_interpretation,
                    self.bits_per_sample.clone(),
                ),
            )),
        }
    }

    fn read_header(&mut self) -> TiffResult<()> {
        let mut endianess = Vec::with_capacity(2);
        self.reader.by_ref().take(2).read_to_end(&mut endianess)?;
        match &*endianess {
            b"II" => {
                self.byte_order = ByteOrder::LittleEndian;
                self.reader.byte_order = ByteOrder::LittleEndian;
            }
            b"MM" => {
                self.byte_order = ByteOrder::BigEndian;
                self.reader.byte_order = ByteOrder::BigEndian;
            }
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureNotFound,
                ))
            }
        }
        match self.read_short()? {
            42 => self.bigtiff = false,
            43 => {
                self.bigtiff = true;
                // Read bytesize of offsets (in bigtiff it's alway 8 but provide a way to move to 16 some day)
                if self.read_short()? != 8 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                // This constant should always be 0
                if self.read_short()? != 0 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
            }
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureInvalid,
                ))
            }
        }
        self.next_ifd = match self.read_ifd_offset()? {
            0 => None,
            n => Some(n),
        };
        Ok(())
    }

    /// Initializes the decoder.
    pub fn init(mut self) -> TiffResult<Decoder<R>> {
        self.read_header()?;
        self.next_image()?;
        Ok(self)
    }

    /// Reads in the next image.
    /// If there is no further image in the TIFF file a format error is returned.
    /// To determine whether there are more images call `TIFFDecoder::more_images` instead.
    pub fn next_image(&mut self) -> TiffResult<()> {
        self.ifd = Some(self.read_ifd()?);
        self.width = self.get_tag_u32(Tag::ImageWidth)?;
        self.height = self.get_tag_u32(Tag::ImageLength)?;
        self.strip_decoder = None;

        self.photometric_interpretation = self
            .find_tag_unsigned(Tag::PhotometricInterpretation)?
            .and_then(PhotometricInterpretation::from_u16)
            .ok_or(TiffUnsupportedError::UnknownInterpretation)?;

        if let Some(val) = self.find_tag_unsigned(Tag::Compression)? {
            self.compression_method = CompressionMethod::from_u16(val)
                .ok_or(TiffUnsupportedError::UnknownCompressionMethod)?;
        }
        if let Some(val) = self.find_tag_unsigned(Tag::SamplesPerPixel)? {
            self.samples = val;
        }
        if let Some(vals) = self.find_tag_unsigned_vec(Tag::SampleFormat)? {
            self.sample_format = vals
                .into_iter()
                .map(SampleFormat::from_u16_exhaustive)
                .collect();

            // TODO: for now, only homogenous formats across samples are supported.
            if !self.sample_format.windows(2).all(|s| s[0] == s[1]) {
                return Err(TiffUnsupportedError::UnsupportedSampleFormat(
                    self.sample_format.clone(),
                )
                .into());
            }
        }
        match self.samples {
            1 | 3 | 4 => {
                if let Some(val) = self.find_tag_unsigned_vec(Tag::BitsPerSample)? {
                    self.bits_per_sample = val;
                }
            }
            _ => return Err(TiffUnsupportedError::UnsupportedSampleDepth(self.samples).into()),
        }

        self.chunk_type =
            match (
                self.get_tag_u32(Tag::RowsPerStrip),
                self.get_tag_u32(Tag::TileWidth),
                self.get_tag_u32(Tag::TileLength),
            ) {
                (Ok(_), Err(_), Err(_)) => ChunkType::Strip,
                (Err(_), Ok(_), Ok(_)) => ChunkType::Tile,
                // TODO: The spec says not to use both strip-oriented fields and tile-oriented fields.
                // We can relax this later if it becomes a problem
                _ => return Err(TiffError::FormatError(TiffFormatError::Format(
                    String::from(
                        "Neither strips nor tiles were found or both were used in the same file",
                    ),
                ))),
            };

        Ok(())
    }

    /// Returns `true` if there is at least one more image available.
    pub fn more_images(&self) -> bool {
        self.next_ifd.is_some()
    }

    /// Returns the byte_order
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    #[inline]
    pub fn read_ifd_offset(&mut self) -> Result<u64, io::Error> {
        if self.bigtiff {
            self.read_long8()
        } else {
            self.read_long().map(u64::from)
        }
    }

    /// Reads a TIFF byte value
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, io::Error> {
        let mut buf = [0; 1];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Reads a TIFF short value
    #[inline]
    pub fn read_short(&mut self) -> Result<u16, io::Error> {
        self.reader.read_u16()
    }

    /// Reads a TIFF sshort value
    #[inline]
    pub fn read_sshort(&mut self) -> Result<i16, io::Error> {
        self.reader.read_i16()
    }

    /// Reads a TIFF long value
    #[inline]
    pub fn read_long(&mut self) -> Result<u32, io::Error> {
        self.reader.read_u32()
    }

    /// Reads a TIFF slong value
    #[inline]
    pub fn read_slong(&mut self) -> Result<i32, io::Error> {
        self.reader.read_i32()
    }

    /// Reads a TIFF float value
    #[inline]
    pub fn read_float(&mut self) -> Result<f32, io::Error> {
        self.reader.read_f32()
    }

    /// Reads a TIFF double value
    #[inline]
    pub fn read_double(&mut self) -> Result<f64, io::Error> {
        self.reader.read_f64()
    }

    #[inline]
    pub fn read_long8(&mut self) -> Result<u64, io::Error> {
        self.reader.read_u64()
    }

    #[inline]
    pub fn read_slong8(&mut self) -> Result<i64, io::Error> {
        self.reader.read_i64()
    }

    /// Reads a string
    #[inline]
    pub fn read_string(&mut self, length: usize) -> TiffResult<String> {
        let mut out = vec![0; length];
        self.reader.read_exact(&mut out)?;
        // Strings may be null-terminated, so we trim anything downstream of the null byte
        if let Some(first) = out.iter().position(|&b| b == 0) {
            out.truncate(first);
        }
        Ok(String::from_utf8(out)?)
    }

    /// Reads a TIFF IFA offset/value field
    #[inline]
    pub fn read_offset(&mut self) -> TiffResult<[u8; 4]> {
        if self.bigtiff {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }
        let mut val = [0; 4];
        self.reader.read_exact(&mut val)?;
        Ok(val)
    }

    /// Reads a TIFF IFA offset/value field
    #[inline]
    pub fn read_offset_u64(&mut self) -> Result<[u8; 8], io::Error> {
        let mut val = [0; 8];
        self.reader.read_exact(&mut val)?;
        Ok(val)
    }

    /// Moves the cursor to the specified offset
    #[inline]
    pub fn goto_offset(&mut self, offset: u32) -> io::Result<()> {
        self.goto_offset_u64(offset.into())
    }

    #[inline]
    pub fn goto_offset_u64(&mut self, offset: u64) -> io::Result<()> {
        self.reader.seek(io::SeekFrom::Start(offset)).map(|_| ())
    }

    /// Reads a IFD entry.
    // An IFD entry has four fields:
    //
    // Tag   2 bytes
    // Type  2 bytes
    // Count 4 bytes
    // Value 4 bytes either a pointer the value itself
    fn read_entry(&mut self) -> TiffResult<Option<(Tag, ifd::Entry)>> {
        let tag = Tag::from_u16_exhaustive(self.read_short()?);
        let type_ = match Type::from_u16(self.read_short()?) {
            Some(t) => t,
            None => {
                // Unknown type. Skip this entry according to spec.
                self.read_long()?;
                self.read_long()?;
                return Ok(None);
            }
        };
        let entry = if self.bigtiff {
            ifd::Entry::new_u64(type_, self.read_long8()?, self.read_offset_u64()?)
        } else {
            ifd::Entry::new(type_, self.read_long()?, self.read_offset()?)
        };
        Ok(Some((tag, entry)))
    }

    /// Reads the next IFD
    fn read_ifd(&mut self) -> TiffResult<Directory> {
        let mut dir: Directory = HashMap::new();
        match self.next_ifd {
            None => {
                return Err(TiffError::FormatError(
                    TiffFormatError::ImageFileDirectoryNotFound,
                ))
            }
            Some(offset) => self.goto_offset_u64(offset)?,
        }
        let num_tags = if self.bigtiff {
            self.read_long8()?
        } else {
            self.read_short()?.into()
        };
        for _ in 0..num_tags {
            let (tag, entry) = match self.read_entry()? {
                Some(val) => val,
                None => {
                    continue;
                } // Unknown data type in tag, skip
            };
            dir.insert(tag, entry);
        }
        self.next_ifd = match self.read_ifd_offset()? {
            0 => None,
            n => Some(n),
        };
        Ok(dir)
    }

    /// Tries to retrieve a tag.
    /// Return `Ok(None)` if the tag is not present.
    pub fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<ifd::Value>> {
        let entry = match self.ifd.as_ref().unwrap().get(&tag) {
            None => return Ok(None),
            Some(entry) => entry.clone(),
        };

        let limits = self.limits.clone();

        Ok(Some(entry.val(&limits, self)?))
    }

    /// Tries to retrieve a tag and convert it to the desired unsigned type.
    pub fn find_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<T>> {
        self.find_tag(tag)?
            .map(|v| v.into_u64())
            .transpose()?
            .map(|value| {
                T::try_from(value).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
            })
            .transpose()
    }

    /// Tries to retrieve a vector of all a tag's values and convert them to
    /// the desired unsigned type.
    pub fn find_tag_unsigned_vec<T: TryFrom<u64>>(
        &mut self,
        tag: Tag,
    ) -> TiffResult<Option<Vec<T>>> {
        self.find_tag(tag)?
            .map(|v| v.into_u64_vec())
            .transpose()?
            .map(|v| {
                v.into_iter()
                    .map(|u| {
                        T::try_from(u).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
                    })
                    .collect()
            })
            .transpose()
    }

    /// Tries to retrieve a tag and convert it to the desired unsigned type.
    /// Returns an error if the tag is not present.
    pub fn get_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<T> {
        self.find_tag_unsigned(tag)?
            .ok_or_else(|| TiffFormatError::RequiredTagNotFound(tag).into())
    }

    /// Tries to retrieve a tag.
    /// Returns an error if the tag is not present
    pub fn get_tag(&mut self, tag: Tag) -> TiffResult<ifd::Value> {
        match self.find_tag(tag)? {
            Some(val) => Ok(val),
            None => Err(TiffError::FormatError(
                TiffFormatError::RequiredTagNotFound(tag),
            )),
        }
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub fn get_tag_u32(&mut self, tag: Tag) -> TiffResult<u32> {
        self.get_tag(tag)?.into_u32()
    }
    pub fn get_tag_u64(&mut self, tag: Tag) -> TiffResult<u64> {
        self.get_tag(tag)?.into_u64()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub fn get_tag_f32(&mut self, tag: Tag) -> TiffResult<f32> {
        self.get_tag(tag)?.into_f32()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub fn get_tag_f64(&mut self, tag: Tag) -> TiffResult<f64> {
        self.get_tag(tag)?.into_f64()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub fn get_tag_u32_vec(&mut self, tag: Tag) -> TiffResult<Vec<u32>> {
        self.get_tag(tag)?.into_u32_vec()
    }

    pub fn get_tag_u16_vec(&mut self, tag: Tag) -> TiffResult<Vec<u16>> {
        self.get_tag(tag)?.into_u16_vec()
    }
    pub fn get_tag_u64_vec(&mut self, tag: Tag) -> TiffResult<Vec<u64>> {
        self.get_tag(tag)?.into_u64_vec()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub fn get_tag_f32_vec(&mut self, tag: Tag) -> TiffResult<Vec<f32>> {
        self.get_tag(tag)?.into_f32_vec()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub fn get_tag_f64_vec(&mut self, tag: Tag) -> TiffResult<Vec<f64>> {
        self.get_tag(tag)?.into_f64_vec()
    }

    /// Tries to retrieve a tag and convert it to a 8bit vector.
    pub fn get_tag_u8_vec(&mut self, tag: Tag) -> TiffResult<Vec<u8>> {
        self.get_tag(tag)?.into_u8_vec()
    }

    /// Tries to retrieve a tag and convert it to a ascii vector.
    pub fn get_tag_ascii_string(&mut self, tag: Tag) -> TiffResult<String> {
        self.get_tag(tag)?.into_string()
    }

    fn invert_colors_unsigned<T>(buffer: &mut [T], max: T)
    where
        T: std::ops::Sub<T> + std::ops::Sub<Output = T> + Copy,
    {
        for datum in buffer.iter_mut() {
            *datum = max - *datum
        }
    }

    fn invert_colors_fp<T>(buffer: &mut [T], max: T)
    where
        T: std::ops::Sub<T> + std::ops::Sub<Output = T> + Copy,
    {
        for datum in buffer.iter_mut() {
            // FIXME: assumes [0, 1) range for floats
            *datum = max - *datum
        }
    }

    fn invert_colors(buf: &mut DecodingBuffer, color_type: ColorType) {
        match (color_type, buf) {
            (ColorType::Gray(64), DecodingBuffer::U64(ref mut buffer)) => {
                Self::invert_colors_unsigned(buffer, 0xffff_ffff_ffff_ffff);
            }
            (ColorType::Gray(32), DecodingBuffer::U32(ref mut buffer)) => {
                Self::invert_colors_unsigned(buffer, 0xffff_ffff);
            }
            (ColorType::Gray(16), DecodingBuffer::U16(ref mut buffer)) => {
                Self::invert_colors_unsigned(buffer, 0xffff);
            }
            (ColorType::Gray(n), DecodingBuffer::U8(ref mut buffer)) if n <= 8 => {
                Self::invert_colors_unsigned(buffer, 0xff);
            }
            (ColorType::Gray(32), DecodingBuffer::F32(ref mut buffer)) => {
                Self::invert_colors_fp(buffer, 1.0);
            }
            (ColorType::Gray(64), DecodingBuffer::F64(ref mut buffer)) => {
                Self::invert_colors_fp(buffer, 1.0);
            }
            _ => {}
        }
    }

    /// Fix endianness. If `byte_order` matches the host, then conversion is a no-op.
    fn fix_endianness(buf: &mut DecodingBuffer, byte_order: ByteOrder) {
        match byte_order {
            ByteOrder::LittleEndian => match buf {
                DecodingBuffer::U8(_) | DecodingBuffer::I8(_) => {}
                DecodingBuffer::U16(b) => b.iter_mut().for_each(|v| *v = u16::from_le(*v)),
                DecodingBuffer::I16(b) => b.iter_mut().for_each(|v| *v = i16::from_le(*v)),
                DecodingBuffer::U32(b) => b.iter_mut().for_each(|v| *v = u32::from_le(*v)),
                DecodingBuffer::I32(b) => b.iter_mut().for_each(|v| *v = i32::from_le(*v)),
                DecodingBuffer::U64(b) => b.iter_mut().for_each(|v| *v = u64::from_le(*v)),
                DecodingBuffer::I64(b) => b.iter_mut().for_each(|v| *v = i64::from_le(*v)),
                DecodingBuffer::F32(b) => b
                    .iter_mut()
                    .for_each(|v| *v = f32::from_bits(u32::from_le(v.to_bits()))),
                DecodingBuffer::F64(b) => b
                    .iter_mut()
                    .for_each(|v| *v = f64::from_bits(u64::from_le(v.to_bits()))),
            },
            ByteOrder::BigEndian => match buf {
                DecodingBuffer::U8(_) | DecodingBuffer::I8(_) => {}
                DecodingBuffer::U16(b) => b.iter_mut().for_each(|v| *v = u16::from_be(*v)),
                DecodingBuffer::I16(b) => b.iter_mut().for_each(|v| *v = i16::from_be(*v)),
                DecodingBuffer::U32(b) => b.iter_mut().for_each(|v| *v = u32::from_be(*v)),
                DecodingBuffer::I32(b) => b.iter_mut().for_each(|v| *v = i32::from_be(*v)),
                DecodingBuffer::U64(b) => b.iter_mut().for_each(|v| *v = u64::from_be(*v)),
                DecodingBuffer::I64(b) => b.iter_mut().for_each(|v| *v = i64::from_be(*v)),
                DecodingBuffer::F32(b) => b
                    .iter_mut()
                    .for_each(|v| *v = f32::from_bits(u32::from_be(v.to_bits()))),
                DecodingBuffer::F64(b) => b
                    .iter_mut()
                    .for_each(|v| *v = f64::from_bits(u64::from_be(v.to_bits()))),
            },
        };
    }

    /// Decompresses the strip into the supplied buffer.
    fn expand_strip<'a>(
        &mut self,
        mut buffer: DecodingBuffer<'a>,
        offset: u64,
        length: u64,
    ) -> TiffResult<()> {
        // Validate that the provided buffer is of the expected type.
        let color_type = self.colortype()?;
        match (color_type, &buffer) {
            (ColorType::RGB(n), _)
            | (ColorType::RGBA(n), _)
            | (ColorType::CMYK(n), _)
            | (ColorType::Gray(n), _)
                if usize::from(n) == buffer.byte_len() * 8 => {}
            (ColorType::Gray(n), DecodingBuffer::U8(_)) if n <= 8 => {}
            (type_, _) => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedColorType(type_),
                ))
            }
        }

        // Construct necessary reader to perform decompression.
        self.goto_offset_u64(offset)?;
        let byte_order = self.reader.byte_order;

        let reader = Self::create_reader(
            &mut self.reader,
            self.compression_method,
            length,
            buffer.len(),
            buffer.byte_len(),
            self.limits.intermediate_buffer_size,
        )?;

        // Read into output buffer.
        {
            let mut buffer = match &mut buffer {
                DecodingBuffer::U8(buf) => &mut *buf,
                DecodingBuffer::I8(buf) => bytecast::i8_as_ne_mut_bytes(buf),
                DecodingBuffer::U16(buf) => bytecast::u16_as_ne_mut_bytes(buf),
                DecodingBuffer::I16(buf) => bytecast::i16_as_ne_mut_bytes(buf),
                DecodingBuffer::U32(buf) => bytecast::u32_as_ne_mut_bytes(buf),
                DecodingBuffer::I32(buf) => bytecast::i32_as_ne_mut_bytes(buf),
                DecodingBuffer::U64(buf) => bytecast::u64_as_ne_mut_bytes(buf),
                DecodingBuffer::I64(buf) => bytecast::i64_as_ne_mut_bytes(buf),
                DecodingBuffer::F32(buf) => bytecast::f32_as_ne_mut_bytes(buf),
                DecodingBuffer::F64(buf) => bytecast::f64_as_ne_mut_bytes(buf),
            };

            // Note that writing updates the slice to point to the yet unwritten part.
            std::io::copy(&mut reader.take(buffer.len() as u64), &mut buffer)?;

            // If less than the expected amount of bytes was read, set the remaining data to 0.
            for b in buffer {
                *b = 0;
            }
        }

        Self::fix_endianness(&mut buffer, byte_order);

        if self.photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
            Self::invert_colors(&mut buffer, color_type);
        }

        Ok(())
    }

    /// Decompresses the tile into the supplied buffer.
    fn expand_tile<'a>(
        &mut self,
        mut buffer: DecodingBuffer<'a>,
        offset: u64,
        compressed_length: u64,
        tile: usize,
    ) -> TiffResult<()> {
        let color_type = self.colortype()?;
        let byte_len = buffer.byte_len();

        let tile_attrs = self.tile_attributes.as_mut().unwrap();
        let (padding_right, padding_down) = tile_attrs.get_padding(tile);
        let tile_samples = tile_attrs.tile_samples;
        let tile_length = tile_attrs.tile_length;
        let row_samples = tile_attrs.row_samples;
        let padding_right_samples = padding_right * self.bits_per_sample.len();

        self.goto_offset_u64(offset)?;

        let tile_decoder = self.tile_decoder.as_mut().unwrap();
        let line_samples = tile_decoder.result_width * self.bits_per_sample.len();

        let mut reader = Self::create_reader(
            &mut self.reader,
            self.compression_method,
            compressed_length,
            tile_samples,
            byte_len,
            self.limits.intermediate_buffer_size,
        )?;

        for row in 0..(tile_length - padding_down) {
            let buf = match &mut buffer {
                DecodingBuffer::U8(buf) => &mut *buf,
                DecodingBuffer::I8(buf) => bytecast::i8_as_ne_mut_bytes(buf),
                DecodingBuffer::U16(buf) => bytecast::u16_as_ne_mut_bytes(buf),
                DecodingBuffer::I16(buf) => bytecast::i16_as_ne_mut_bytes(buf),
                DecodingBuffer::U32(buf) => bytecast::u32_as_ne_mut_bytes(buf),
                DecodingBuffer::I32(buf) => bytecast::i32_as_ne_mut_bytes(buf),
                DecodingBuffer::U64(buf) => bytecast::u64_as_ne_mut_bytes(buf),
                DecodingBuffer::I64(buf) => bytecast::i64_as_ne_mut_bytes(buf),
                DecodingBuffer::F32(buf) => bytecast::f32_as_ne_mut_bytes(buf),
                DecodingBuffer::F64(buf) => bytecast::f64_as_ne_mut_bytes(buf),
            };

            let row_start = row * line_samples;
            let row_end = row_start + row_samples - padding_right_samples;

            let row = &mut buf[(row_start * byte_len)..(row_end * byte_len)];
            reader.read_exact(row)?;

            // Skip horizontal padding
            if padding_right > 0 {
                let len = u64::try_from(padding_right_samples * byte_len)?;
                io::copy(&mut reader.by_ref().take(len), &mut io::sink())?;
            }

            Self::fix_endianness(&mut buffer.subrange(row_start..row_end), self.byte_order);

            if self.photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
                Self::invert_colors(&mut buffer.subrange(row_start..row_end), color_type);
            }
        }

        Ok(())
    }

    fn create_reader<'r>(
        reader: &'r mut SmartReader<R>,
        compression_method: CompressionMethod,
        compressed_length: u64,
        samples: usize,  // Expected chunk length in samples
        byte_len: usize, // Byte length of the samples in result buffer
        intermediate_buffer_size: usize,
    ) -> TiffResult<Box<dyn Read + 'r>> {
        Ok(match compression_method {
            CompressionMethod::None => Box::new(reader),
            CompressionMethod::LZW => {
                let clen = usize::try_from(compressed_length)?;

                if samples * byte_len > intermediate_buffer_size || clen > intermediate_buffer_size
                {
                    return Err(TiffError::LimitsExceeded);
                }

                Box::new(LZWReader::new(reader, clen, samples * byte_len)?.1)
            }
            CompressionMethod::PackBits => {
                Box::new(PackBitsReader::new(reader, usize::try_from(compressed_length)?)?.1)
            }
            CompressionMethod::Deflate | CompressionMethod::OldDeflate => {
                Box::new(DeflateReader::new(reader))
            }
            method => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedCompressionMethod(method),
                ))
            }
        })
    }

    fn check_chunk_type(&self, expected: ChunkType) -> TiffResult<()> {
        if expected != self.chunk_type {
            return Err(TiffError::UsageError(UsageError::InvalidChunkType(
                expected,
                self.chunk_type,
            )));
        }

        Ok(())
    }

    /// The chunk type (Strips / Tiles) of the image
    pub fn get_chunk_type(&self) -> ChunkType {
        self.chunk_type
    }

    /// Number of strips in image
    pub fn strip_count(&mut self) -> TiffResult<u32> {
        self.check_chunk_type(ChunkType::Strip)?;
        let rows_per_strip = self.get_tag_u32(Tag::RowsPerStrip).unwrap_or(self.height);

        if rows_per_strip == 0 {
            return Ok(0);
        }

        // rows_per_strip - 1 can never fail since we know it's at least 1
        let height = match self.height.checked_add(rows_per_strip - 1) {
            Some(h) => h,
            None => return Err(TiffError::IntSizeError),
        };

        Ok(height / rows_per_strip)
    }

    /// Number of tiles in image
    pub fn tile_count(&mut self) -> TiffResult<u32> {
        self.check_chunk_type(ChunkType::Tile)?;
        self.init_tile_attributes()?;
        let tile_attrs = self.tile_attributes.as_ref().unwrap();
        Ok(u32::try_from(
            tile_attrs.tiles_across * tile_attrs.tiles_down,
        )?)
    }

    fn initialize_strip_decoder(&mut self) -> TiffResult<()> {
        if self.strip_decoder.is_none() {
            let strip_offsets = self.get_tag_u64_vec(Tag::StripOffsets)?;
            let strip_bytes = self.get_tag_u64_vec(Tag::StripByteCounts)?;

            self.strip_decoder = Some(StripDecodeState {
                strip_index: 0,
                strip_offsets,
                strip_bytes,
            });
        }
        Ok(())
    }

    fn init_tile_attributes(&mut self) -> TiffResult<()> {
        if self.tile_attributes.is_none() {
            let tile_width = usize::try_from(self.get_tag_u32(Tag::TileWidth)?)?;
            let tile_length = usize::try_from(self.get_tag_u32(Tag::TileLength)?)?;

            if tile_width == 0 {
                return Err(TiffFormatError::InvalidTagValueType(Tag::TileWidth).into());
            }

            if tile_length == 0 {
                return Err(TiffFormatError::InvalidTagValueType(Tag::TileLength).into());
            }

            let tiles_across = (usize::try_from(self.width)? + tile_width - 1) / tile_width;
            let tiles_down = (usize::try_from(self.height)? + tile_length - 1) / tile_length;

            let samples_per_pixel = self.bits_per_sample.len();

            let tile_samples = tile_length * tile_width * samples_per_pixel;
            let padding_right = (tiles_across * tile_width) - usize::try_from(self.width)?;
            let tile_strip_samples =
                (tile_samples * tiles_across) - (padding_right * tile_length * samples_per_pixel);

            self.tile_attributes = Some(TileAttributes {
                tile_width,
                tile_length,
                tiles_across,
                tiles_down,
                tile_samples,
                padding_right,
                padding_down: (tiles_down * tile_length) - usize::try_from(self.height)?,
                row_samples: (tile_width * samples_per_pixel),
                tile_strip_samples,
            });
        }

        Ok(())
    }

    fn update_tile_decoder(&mut self, result_width: usize) -> TiffResult<()> {
        if self.tile_decoder.is_none() {
            self.tile_decoder = Some(TileDecodeState {
                current_tile: 0,
                tile_offsets: self.get_tag_u64_vec(Tag::TileOffsets)?,
                tile_bytes: self.get_tag_u64_vec(Tag::TileByteCounts)?,
                result_width: 0, // needs to be updated for differently padded_tiles, see below
            })
        }

        self.tile_decoder.as_mut().unwrap().result_width = result_width;

        Ok(())
    }

    pub fn read_jpeg(&mut self) -> TiffResult<DecodingResult> {
        let offsets = self.get_tag_u32_vec(Tag::StripOffsets)?;
        let bytes = self.get_tag_u32_vec(Tag::StripByteCounts)?;

        let jpeg_tables: Option<Vec<u8>> = match self.find_tag(Tag::JPEGTables) {
            Ok(None) => None,
            Ok(_) => {
                let vec = self.get_tag_u8_vec(Tag::JPEGTables)?;

                if vec.len() < 2 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                    ));
                }

                Some(vec)
            }
            Err(e) => return Err(e),
        };

        if offsets.len() == 0 {
            return Err(TiffError::FormatError(TiffFormatError::RequiredTagEmpty(
                Tag::StripOffsets,
            )));
        }
        if offsets.len() != bytes.len() {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }

        if offsets[0] as usize > self.limits.intermediate_buffer_size
            || bytes
                .iter()
                .any(|&x| x as usize > self.limits.intermediate_buffer_size)
        {
            return Err(TiffError::LimitsExceeded);
        }

        let mut res_img = Vec::with_capacity(offsets[0] as usize);

        for (idx, offset) in offsets.iter().enumerate() {
            self.goto_offset(*offset)?;
            let length = bytes[idx];

            if jpeg_tables.is_some() && length < 2 {
                return Err(TiffError::FormatError(
                    TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                ));
            }
            let jpeg_reader = JpegReader::new(&mut self.reader, length, &jpeg_tables)?;
            let mut decoder = jpeg::Decoder::new(jpeg_reader);

            match decoder.decode() {
                Ok(mut val) => res_img.append(&mut val),
                Err(e) => {
                    return match e {
                        jpeg::Error::Io(io_err) => Err(TiffError::IoError(io_err)),
                        jpeg::Error::Format(fmt_err) => {
                            Err(TiffError::FormatError(TiffFormatError::Format(fmt_err)))
                        }
                        jpeg::Error::Unsupported(_) => Err(TiffError::UnsupportedError(
                            TiffUnsupportedError::UnknownInterpretation,
                        )),
                        jpeg::Error::Internal(_) => Err(TiffError::UnsupportedError(
                            TiffUnsupportedError::UnknownInterpretation,
                        )),
                    }
                }
            }
        }

        Ok(DecodingResult::U8(res_img))
    }

    pub fn read_strip_to_buffer(&mut self, mut buffer: DecodingBuffer) -> TiffResult<()> {
        self.initialize_strip_decoder()?;
        let index = self.strip_decoder.as_ref().unwrap().strip_index;
        let offset = *self
            .strip_decoder
            .as_ref()
            .unwrap()
            .strip_offsets
            .get(index)
            .ok_or(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ))?;
        let byte_count = *self
            .strip_decoder
            .as_ref()
            .unwrap()
            .strip_bytes
            .get(index)
            .ok_or(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ))?;
        let tag_rows = self.get_tag_u32(Tag::RowsPerStrip).unwrap_or(self.height);
        let rows_per_strip = usize::try_from(tag_rows)?;

        let sized_width = usize::try_from(self.width)?;
        let sized_height = usize::try_from(self.height)?;

        let strip_height_without_padding = index
            .checked_mul(rows_per_strip)
            .and_then(|x| sized_height.checked_sub(x))
            .ok_or(TiffError::IntSizeError)?;

        // Ignore potential vertical padding on the bottommost strip
        let strip_height = rows_per_strip.min(strip_height_without_padding);

        let buffer_size = sized_width
            .checked_mul(strip_height)
            .and_then(|x| x.checked_mul(self.bits_per_sample.len()))
            .ok_or(TiffError::LimitsExceeded)?;

        if buffer.len() < buffer_size {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }

        self.expand_strip(buffer.subrange(0..buffer_size), offset, byte_count)?;
        self.strip_decoder.as_mut().unwrap().strip_index += 1;

        if u32::try_from(index)? == self.strip_count()? {
            self.strip_decoder = None;
        }
        if let Ok(predictor) = self.get_tag_unsigned(Tag::Predictor) {
            match Predictor::from_u16(predictor) {
                Some(Predictor::None) => (),
                Some(Predictor::Horizontal) => {
                    rev_hpredict(
                        buffer.copy(),
                        (self.width, u32::try_from(strip_height)?),
                        usize::try_from(self.width)?,
                        self.colortype()?,
                    )?;
                }
                None => {
                    return Err(TiffError::FormatError(TiffFormatError::UnknownPredictor(
                        predictor,
                    )))
                }
                Some(Predictor::__NonExhaustive) => unreachable!(),
            }
        }
        Ok(())
    }

    fn read_tile_to_buffer(&mut self, result: &mut DecodingBuffer, tile: usize) -> TiffResult<()> {
        let file_offset = *self
            .tile_decoder
            .as_ref()
            .unwrap()
            .tile_offsets
            .get(tile)
            .ok_or(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ))?;

        let compressed_bytes = *self
            .tile_decoder
            .as_ref()
            .unwrap()
            .tile_bytes
            .get(tile)
            .ok_or(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ))?;

        let tile_attrs = self.tile_attributes.as_ref().unwrap();
        let tile_width = tile_attrs.tile_width;
        let tile_length = tile_attrs.tile_length;

        let (padding_right, padding_down) = tile_attrs.get_padding(tile);

        self.expand_tile(result.copy(), file_offset, compressed_bytes, tile)?;

        if let Ok(predictor) = self.get_tag_unsigned(Tag::Predictor) {
            match Predictor::from_u16(predictor) {
                Some(Predictor::None) => (),
                Some(Predictor::Horizontal) => {
                    rev_hpredict(
                        result.copy(),
                        (
                            u32::try_from(tile_width - padding_right)?,
                            u32::try_from(tile_length - padding_down)?,
                        ),
                        self.tile_decoder.as_ref().unwrap().result_width,
                        self.colortype()?,
                    )?;
                }
                None => {
                    return Err(TiffError::FormatError(TiffFormatError::UnknownPredictor(
                        predictor,
                    )))
                }
                Some(Predictor::__NonExhaustive) => unreachable!(),
            }
        }
        Ok(())
    }

    fn result_buffer(&self, width: usize, height: usize) -> TiffResult<DecodingResult> {
        let buffer_size = match width
            .checked_mul(height)
            .and_then(|x| x.checked_mul(self.bits_per_sample.len()))
        {
            Some(s) => s,
            None => return Err(TiffError::LimitsExceeded),
        };

        let max_sample_bits = self.bits_per_sample.iter().cloned().max().unwrap_or(8);
        match self.sample_format.first().unwrap_or(&SampleFormat::Uint) {
            SampleFormat::Uint => match max_sample_bits {
                n if n <= 8 => DecodingResult::new_u8(buffer_size, &self.limits),
                n if n <= 16 => DecodingResult::new_u16(buffer_size, &self.limits),
                n if n <= 32 => DecodingResult::new_u32(buffer_size, &self.limits),
                n if n <= 64 => DecodingResult::new_u64(buffer_size, &self.limits),
                n => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedBitsPerChannel(n),
                )),
            },
            SampleFormat::IEEEFP => match max_sample_bits {
                32 => DecodingResult::new_f32(buffer_size, &self.limits),
                64 => DecodingResult::new_f64(buffer_size, &self.limits),
                n => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedBitsPerChannel(n),
                )),
            },
            SampleFormat::Int => match max_sample_bits {
                n if n <= 8 => DecodingResult::new_i8(buffer_size, &self.limits),
                n if n <= 16 => DecodingResult::new_i16(buffer_size, &self.limits),
                n if n <= 32 => DecodingResult::new_i32(buffer_size, &self.limits),
                n if n <= 64 => DecodingResult::new_i64(buffer_size, &self.limits),
                n => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedBitsPerChannel(n),
                )),
            },
            format => {
                Err(TiffUnsupportedError::UnsupportedSampleFormat(vec![format.clone()]).into())
            }
        }
    }

    /// Read a single strip from the image and return it as a Vector
    pub fn read_strip(&mut self) -> TiffResult<DecodingResult> {
        self.check_chunk_type(ChunkType::Strip)?;
        self.initialize_strip_decoder()?;
        let index = self.strip_decoder.as_ref().unwrap().strip_index;

        let rows_per_strip =
            usize::try_from(self.get_tag_u32(Tag::RowsPerStrip).unwrap_or(self.height))?;

        let strip_height = cmp::min(
            rows_per_strip,
            usize::try_from(self.height)? - index * rows_per_strip,
        );

        let mut result = self.result_buffer(usize::try_from(self.width)?, strip_height)?;
        self.read_strip_to_buffer(result.as_buffer(0))?;

        Ok(result)
    }

    /// Read a single tile from the image and return it as a Vector
    pub fn read_tile(&mut self) -> TiffResult<DecodingResult> {
        self.check_chunk_type(ChunkType::Tile)?;
        self.init_tile_attributes()?;

        let tile = self.tile_decoder.as_ref().map_or(0, |d| d.current_tile);

        let tile_attrs = self.tile_attributes.as_ref().unwrap();
        let (padding_right, padding_down) = tile_attrs.get_padding(tile);

        let tile_width = tile_attrs.tile_width - padding_right;
        let tile_length = tile_attrs.tile_length - padding_down;

        let mut result = self.result_buffer(tile_width, tile_length)?;
        self.update_tile_decoder(tile_width)?;

        self.read_tile_to_buffer(&mut result.as_buffer(0), tile)?;

        self.tile_decoder.as_mut().unwrap().current_tile += 1;

        Ok(result)
    }

    fn read_tiled_image(&mut self) -> TiffResult<DecodingResult> {
        let width = usize::try_from(self.width)?;
        let mut result = self.result_buffer(width, usize::try_from(self.height)?)?;

        self.init_tile_attributes()?;
        self.update_tile_decoder(width)?;

        let tile_attrs = self.tile_attributes.as_ref().unwrap();
        let tiles_across = tile_attrs.tiles_across;
        let tiles_down = tile_attrs.tiles_down;

        for tile in 0..(tiles_across * tiles_down) {
            let buffer_offset = self.tile_attributes.as_ref().unwrap().get_offset(tile);
            self.read_tile_to_buffer(&mut result.as_buffer(buffer_offset), tile)?;
        }

        Ok(result)
    }

    fn read_stripped_image(&mut self) -> TiffResult<DecodingResult> {
        self.initialize_strip_decoder()?;
        let rows_per_strip =
            usize::try_from(self.get_tag_u32(Tag::RowsPerStrip).unwrap_or(self.height))?;

        let samples_per_strip = match usize::try_from(self.width)?
            .checked_mul(rows_per_strip)
            .and_then(|x| x.checked_mul(self.bits_per_sample.len()))
        {
            Some(s) => s,
            None => return Err(TiffError::LimitsExceeded),
        };

        let mut result =
            self.result_buffer(usize::try_from(self.width)?, usize::try_from(self.height)?)?;

        for i in 0..usize::try_from(self.strip_count()?)? {
            let r = result.as_buffer(samples_per_strip * i);
            self.read_strip_to_buffer(r)?;
        }
        Ok(result)
    }

    /// Decodes the entire image and return it as a Vector
    pub fn read_image(&mut self) -> TiffResult<DecodingResult> {
        let result = match (self.chunk_type, self.compression_method) {
            (_, CompressionMethod::ModernJPEG) => self.read_jpeg()?,
            (ChunkType::Strip, _) => self.read_stripped_image()?,
            (ChunkType::Tile, _) => self.read_tiled_image()?,
        };

        Ok(result)
    }
}
