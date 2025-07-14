use std::collections::BTreeMap;
use std::io::{self, Read, Seek};

use crate::tags::{
    CompressionMethod, IfdPointer, PhotometricInterpretation, PlanarConfiguration, Predictor,
    SampleFormat, Tag, Type,
};
use crate::{
    bytecast, ColorType, Directory, TiffError, TiffFormatError, TiffResult, TiffUnsupportedError,
    UsageError,
};
use half::f16;

use self::image::Image;
use self::stream::{ByteOrder, EndianReader};

mod cycles;
pub mod ifd;
mod image;
mod stream;
mod tag_reader;

/// Result of a decoding process
#[derive(Debug, PartialEq)]
pub enum DecodingResult {
    /// A vector of unsigned bytes
    U8(Vec<u8>),
    /// A vector of unsigned words
    U16(Vec<u16>),
    /// A vector of 32 bit unsigned ints
    U32(Vec<u32>),
    /// A vector of 64 bit unsigned ints
    U64(Vec<u64>),
    /// A vector of 16 bit IEEE floats (held in u16)
    F16(Vec<f16>),
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

    fn new_f16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / std::mem::size_of::<u16>() {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(DecodingResult::F16(vec![f16::ZERO; size]))
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
            DecodingResult::F16(ref mut buf) => DecodingBuffer::F16(&mut buf[start..]),
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
    /// A slice of 16 bit IEEE floats
    F16(&'a mut [f16]),
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
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            DecodingBuffer::U8(ref mut buf) => buf,
            DecodingBuffer::I8(buf) => bytecast::i8_as_ne_mut_bytes(buf),
            DecodingBuffer::U16(buf) => bytecast::u16_as_ne_mut_bytes(buf),
            DecodingBuffer::I16(buf) => bytecast::i16_as_ne_mut_bytes(buf),
            DecodingBuffer::U32(buf) => bytecast::u32_as_ne_mut_bytes(buf),
            DecodingBuffer::I32(buf) => bytecast::i32_as_ne_mut_bytes(buf),
            DecodingBuffer::U64(buf) => bytecast::u64_as_ne_mut_bytes(buf),
            DecodingBuffer::I64(buf) => bytecast::i64_as_ne_mut_bytes(buf),
            DecodingBuffer::F16(buf) => bytecast::f16_as_ne_mut_bytes(buf),
            DecodingBuffer::F32(buf) => bytecast::f32_as_ne_mut_bytes(buf),
            DecodingBuffer::F64(buf) => bytecast::f64_as_ne_mut_bytes(buf),
        }
    }
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
    /// There are grouped for borrow checker reasons. This allows us to implement methods that
    /// borrow the stream access and the other fields mutably at the same time.
    value_reader: ValueReader<R>,
    current_ifd: Option<IfdPointer>,
    next_ifd: Option<IfdPointer>,
    /// The IFDs we visited already in this chain of IFDs.
    ifd_offsets: Vec<IfdPointer>,
    /// Map from the ifd into the `ifd_offsets` ordered list.
    seen_ifds: cycles::IfdCycles,
    /// The directory, if we have not yet read it as an image.
    /// This is prioritized _over_ the image. Hence it must _not_ be set if we are currently
    /// reading a true image IFD that is instead stored in the `image` attribute.
    non_image_ifd: Option<Directory>,
    image: Image,
}

/// All the information needed to read and interpret byte slices from the underlying file, i.e. to
/// turn an entry of a tag into an `ifd::Value` or otherwise fetch arrays of similar types. Used
/// only as the type of the field [`Decoder::value_reader`] and passed to submodules.
#[derive(Debug)]
struct ValueReader<R> {
    reader: EndianReader<R>,
    bigtiff: bool,
    limits: Limits,
}

/// Reads a directory's tag values from an underlying stream.
pub struct IfdDecoder<'lt> {
    inner: tag_reader::TagReader<'lt, dyn tag_reader::EntryDecoder + 'lt>,
}

fn rev_hpredict_nsamp(buf: &mut [u8], bit_depth: u8, samples: usize) {
    match bit_depth {
        0..=8 => {
            for i in samples..buf.len() {
                buf[i] = buf[i].wrapping_add(buf[i - samples]);
            }
        }
        9..=16 => {
            for i in (samples * 2..buf.len()).step_by(2) {
                let v = u16::from_ne_bytes(buf[i..][..2].try_into().unwrap());
                let p = u16::from_ne_bytes(buf[i - 2 * samples..][..2].try_into().unwrap());
                buf[i..][..2].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
            }
        }
        17..=32 => {
            for i in (samples * 4..buf.len()).step_by(4) {
                let v = u32::from_ne_bytes(buf[i..][..4].try_into().unwrap());
                let p = u32::from_ne_bytes(buf[i - 4 * samples..][..4].try_into().unwrap());
                buf[i..][..4].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
            }
        }
        33..=64 => {
            for i in (samples * 8..buf.len()).step_by(8) {
                let v = u64::from_ne_bytes(buf[i..][..8].try_into().unwrap());
                let p = u64::from_ne_bytes(buf[i - 8 * samples..][..8].try_into().unwrap());
                buf[i..][..8].copy_from_slice(&(v.wrapping_add(p)).to_ne_bytes());
            }
        }
        _ => {
            unreachable!("Caller should have validated arguments. Please file a bug.")
        }
    }
}

fn predict_f32(input: &mut [u8], output: &mut [u8], samples: usize) {
    for i in samples..input.len() {
        input[i] = input[i].wrapping_add(input[i - samples]);
    }

    for (i, chunk) in output.chunks_mut(4).enumerate() {
        chunk.copy_from_slice(&u32::to_ne_bytes(u32::from_be_bytes([
            input[i],
            input[input.len() / 4 + i],
            input[input.len() / 4 * 2 + i],
            input[input.len() / 4 * 3 + i],
        ])));
    }
}

fn predict_f16(input: &mut [u8], output: &mut [u8], samples: usize) {
    for i in samples..input.len() {
        input[i] = input[i].wrapping_add(input[i - samples]);
    }

    for (i, chunk) in output.chunks_mut(2).enumerate() {
        chunk.copy_from_slice(&u16::to_ne_bytes(u16::from_be_bytes([
            input[i],
            input[input.len() / 2 + i],
        ])));
    }
}

fn predict_f64(input: &mut [u8], output: &mut [u8], samples: usize) {
    for i in samples..input.len() {
        input[i] = input[i].wrapping_add(input[i - samples]);
    }

    for (i, chunk) in output.chunks_mut(8).enumerate() {
        chunk.copy_from_slice(&u64::to_ne_bytes(u64::from_be_bytes([
            input[i],
            input[input.len() / 8 + i],
            input[input.len() / 8 * 2 + i],
            input[input.len() / 8 * 3 + i],
            input[input.len() / 8 * 4 + i],
            input[input.len() / 8 * 5 + i],
            input[input.len() / 8 * 6 + i],
            input[input.len() / 8 * 7 + i],
        ])));
    }
}

fn fix_endianness_and_predict(
    buf: &mut [u8],
    bit_depth: u8,
    samples: usize,
    byte_order: ByteOrder,
    predictor: Predictor,
) {
    match predictor {
        Predictor::None => {
            fix_endianness(buf, byte_order, bit_depth);
        }
        Predictor::Horizontal => {
            fix_endianness(buf, byte_order, bit_depth);
            rev_hpredict_nsamp(buf, bit_depth, samples);
        }
        Predictor::FloatingPoint => {
            let mut buffer_copy = buf.to_vec();
            match bit_depth {
                16 => predict_f16(&mut buffer_copy, buf, samples),
                32 => predict_f32(&mut buffer_copy, buf, samples),
                64 => predict_f64(&mut buffer_copy, buf, samples),
                _ => unreachable!("Caller should have validated arguments. Please file a bug."),
            }
        }
    }
}

fn invert_colors(buf: &mut [u8], color_type: ColorType, sample_format: SampleFormat) {
    match (color_type, sample_format) {
        (ColorType::Gray(8), SampleFormat::Uint) => {
            for x in buf {
                *x = 0xff - *x;
            }
        }
        (ColorType::Gray(16), SampleFormat::Uint) => {
            for x in buf.chunks_mut(2) {
                let v = u16::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(0xffff - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(32), SampleFormat::Uint) => {
            for x in buf.chunks_mut(4) {
                let v = u32::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(0xffff_ffff - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(64), SampleFormat::Uint) => {
            for x in buf.chunks_mut(8) {
                let v = u64::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(0xffff_ffff_ffff_ffff - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(32), SampleFormat::IEEEFP) => {
            for x in buf.chunks_mut(4) {
                let v = f32::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(1.0 - v).to_ne_bytes());
            }
        }
        (ColorType::Gray(64), SampleFormat::IEEEFP) => {
            for x in buf.chunks_mut(8) {
                let v = f64::from_ne_bytes(x.try_into().unwrap());
                x.copy_from_slice(&(1.0 - v).to_ne_bytes());
            }
        }
        _ => {}
    }
}

/// Fix endianness. If `byte_order` matches the host, then conversion is a no-op.
fn fix_endianness(buf: &mut [u8], byte_order: ByteOrder, bit_depth: u8) {
    match byte_order {
        ByteOrder::LittleEndian => match bit_depth {
            0..=8 => {}
            9..=16 => buf.chunks_exact_mut(2).for_each(|v| {
                v.copy_from_slice(&u16::from_le_bytes((*v).try_into().unwrap()).to_ne_bytes())
            }),
            17..=32 => buf.chunks_exact_mut(4).for_each(|v| {
                v.copy_from_slice(&u32::from_le_bytes((*v).try_into().unwrap()).to_ne_bytes())
            }),
            _ => buf.chunks_exact_mut(8).for_each(|v| {
                v.copy_from_slice(&u64::from_le_bytes((*v).try_into().unwrap()).to_ne_bytes())
            }),
        },
        ByteOrder::BigEndian => match bit_depth {
            0..=8 => {}
            9..=16 => buf.chunks_exact_mut(2).for_each(|v| {
                v.copy_from_slice(&u16::from_be_bytes((*v).try_into().unwrap()).to_ne_bytes())
            }),
            17..=32 => buf.chunks_exact_mut(4).for_each(|v| {
                v.copy_from_slice(&u32::from_be_bytes((*v).try_into().unwrap()).to_ne_bytes())
            }),
            _ => buf.chunks_exact_mut(8).for_each(|v| {
                v.copy_from_slice(&u64::from_be_bytes((*v).try_into().unwrap()).to_ne_bytes())
            }),
        },
    };
}

impl<R: Read + Seek> Decoder<R> {
    /// Create a new decoder that decodes from the stream ```r```
    pub fn new(mut r: R) -> TiffResult<Decoder<R>> {
        let mut endianess = Vec::with_capacity(2);
        (&mut r).take(2).read_to_end(&mut endianess)?;
        let byte_order = match &*endianess {
            b"II" => ByteOrder::LittleEndian,
            b"MM" => ByteOrder::BigEndian,
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureNotFound,
                ))
            }
        };
        let mut reader = EndianReader::new(r, byte_order);

        let bigtiff = match reader.read_u16()? {
            42 => false,
            43 => {
                // Read bytesize of offsets (in bigtiff it's alway 8 but provide a way to move to 16 some day)
                if reader.read_u16()? != 8 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                // This constant should always be 0
                if reader.read_u16()? != 0 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                true
            }
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureInvalid,
                ))
            }
        };

        let next_ifd = if bigtiff {
            Some(reader.read_u64()?)
        } else {
            Some(u64::from(reader.read_u32()?))
        }
        .map(IfdPointer);

        let current_ifd = *next_ifd.as_ref().unwrap();
        let ifd_offsets = vec![current_ifd];

        let mut decoder = Decoder {
            value_reader: ValueReader {
                reader,
                bigtiff,
                limits: Default::default(),
            },
            next_ifd,
            ifd_offsets,
            current_ifd: None,
            seen_ifds: cycles::IfdCycles::new(),
            image: Image {
                ifd: None,
                width: 0,
                height: 0,
                bits_per_sample: 1,
                samples: 1,
                sample_format: SampleFormat::Uint,
                photometric_interpretation: PhotometricInterpretation::BlackIsZero,
                compression_method: CompressionMethod::None,
                jpeg_tables: None,
                predictor: Predictor::None,
                chunk_type: ChunkType::Strip,
                planar_config: PlanarConfiguration::Chunky,
                strip_decoder: None,
                tile_attributes: None,
                chunk_offsets: Vec::new(),
                chunk_bytes: Vec::new(),
            },
            non_image_ifd: None,
        };
        decoder.next_image()?;
        Ok(decoder)
    }

    pub fn with_limits(mut self, limits: Limits) -> Decoder<R> {
        self.value_reader.limits = limits;
        self
    }

    pub fn dimensions(&mut self) -> TiffResult<(u32, u32)> {
        Ok((self.image().width, self.image().height))
    }

    pub fn colortype(&mut self) -> TiffResult<ColorType> {
        self.image().colortype()
    }

    /// The offset of the directory representing the current image.
    pub fn ifd_pointer(&mut self) -> Option<IfdPointer> {
        self.current_ifd
    }

    fn image(&self) -> &Image {
        &self.image
    }

    /// Loads the IFD at the specified index in the list, if one exists
    pub fn seek_to_image(&mut self, ifd_index: usize) -> TiffResult<()> {
        let ifd = self.seek_to_directory(ifd_index)?;
        self.non_image_ifd = None;
        self.image = Image::from_reader(&mut self.value_reader, ifd)?;
        Ok(())
    }

    fn seek_to_directory(&mut self, ifd_index: usize) -> TiffResult<Directory> {
        if ifd_index < self.ifd_offsets.len() {
            // If the index is within the list of ifds then we can load the selected image/IFD
            let ifd_offset = self.ifd_offsets[ifd_index];
            let ifd = self.value_reader.read_directory(ifd_offset)?;

            self.next_ifd = ifd.next();
            self.current_ifd = Some(ifd_offset);

            return Ok(ifd);
        }

        // Follow the list until we find the one we want, or we reach the end, whichever happens
        // first. How many IFDs to read only for their `next` field?
        let step_over = self.ifd_offsets.len() - ifd_index;

        for _ in 0..step_over {
            // FIXME: for optimization we only need to read the offset of this one, not its whole
            // data. However on buffered files this should be a rather small difference unless
            // you're traversing a lot of directories.

            // Detecting an end-of-file is done by `next_ifd`. We ignore the IFD itself (but not
            // via an ignore pattern to avoid silencing must_use accidentally).
            self.next_ifd()?;
        }

        // self.next_ifd, self.current_ifd will be setup by `next_ifd`.
        self.next_ifd()
    }

    /// Start the chain of image directories from a new location.
    ///
    /// This enters a new chain of image file directories with the indicated offset as the new root
    /// (similar to the file having its initial offset at the given IFD position). After this call,
    /// [`Self::seek_to_image`] has the new root at index `0` and works relative to the new root.
    ///
    /// This is not atomic with regards to errors. If the function returns an error, the decoder is
    /// left in an intermediate state where it does not point to any image. A valid state can be
    /// recovered by calling [`Self::restart_at_image`] with a valid offset and a successful seek.
    pub fn restart_at_image(&mut self, offset: IfdPointer) -> TiffResult<()> {
        let ifd = self.restart_at_offset(offset)?;
        self.non_image_ifd = None;
        self.image = Image::from_reader(&mut self.value_reader, ifd)?;
        Ok(())
    }

    /// Start the chain of non-image directories from a new location.
    ///
    /// See [`Self::restart_at_image`] for details, except this method does not attempt to
    /// interpret the directory in the sequence as an image. Instead, it may be used to read a
    /// sequence of auxiliary IFDs that are not necessarily images. For instance, a directory
    /// referred to in the SubIfd tag may be a thumbnail
    pub fn restart_at_directory(&mut self, offset: IfdPointer) -> TiffResult<()> {
        let ifd = self.restart_at_offset(offset)?;
        self.non_image_ifd = Some(ifd);
        Ok(())
    }

    fn restart_at_offset(&mut self, offset: IfdPointer) -> TiffResult<Directory> {
        self.ifd_offsets.clear();
        self.ifd_offsets.push(offset);

        self.next_ifd = Some(offset);
        self.current_ifd = None;

        self.next_ifd()
    }

    fn next_ifd(&mut self) -> TiffResult<Directory> {
        let Some(next_ifd) = self.next_ifd.take() else {
            self.current_ifd = None;
            self.non_image_ifd = None;

            return Err(TiffError::FormatError(
                TiffFormatError::ImageFileDirectoryNotFound,
            ));
        };

        let ifd = self.value_reader.read_directory(next_ifd)?;

        // Ensure this walk does not get us into a cycle.
        self.seen_ifds.insert_next(next_ifd, ifd.next())?;

        // Extend the list of known IFD offsets in this chain, if needed.
        if self.ifd_offsets.last().copied() == self.current_ifd {
            self.ifd_offsets.push(next_ifd);
        }

        self.current_ifd = Some(next_ifd);
        self.next_ifd = ifd.next();

        Ok(ifd)
    }

    /// Reads in the next image.
    /// If there is no further image in the TIFF file a format error is returned.
    /// To determine whether there are more images call `TIFFDecoder::more_images` instead.
    pub fn next_image(&mut self) -> TiffResult<()> {
        let ifd = self.next_ifd()?;
        self.non_image_ifd = None;
        self.image = Image::from_reader(&mut self.value_reader, ifd)?;
        Ok(())
    }

    /// Read the next directory without interpreting it as an image.
    ///
    /// If there is no further image in the TIFF file a format error is returned. To determine
    /// whether there are more images call `TIFFDecoder::more_directories` instead.
    pub fn next_directory(&mut self) -> TiffResult<()> {
        let ifd = self.next_ifd()?;
        self.non_image_ifd = Some(ifd);
        Ok(())
    }

    /// Interpret the current directory as an image.
    ///
    /// This method is used after having called [`Self::restart_at`] or [`Self::next_directory`] to
    /// iterate the sequence of image file directories, having read a directory without having read
    /// its tags as image data.
    pub fn current_directory_as_image(&mut self) -> TiffResult<()> {
        let current_ifd = self.current_ifd.ok_or(TiffError::FormatError(
            TiffFormatError::ImageFileDirectoryNotFound,
        ))?;

        if let Some(ifd) = &self.non_image_ifd {
            self.image = Image::from_ref(&mut self.value_reader, ifd)?;
            // Definitely sets an IFD but this works without unwraps.
            self.image.ifd = self.non_image_ifd.take();
        } else {
            // Probably re-reading an image but that's fine.
            let ifd = self.read_directory(current_ifd)?;
            self.image = Image::from_reader(&mut self.value_reader, ifd)?;
        }

        Ok(())
    }

    /// Returns `true` if there is at least one more image available.
    pub fn more_images(&self) -> bool {
        self.next_ifd.is_some()
    }

    /// Returns the byte_order of the file.
    pub fn byte_order(&self) -> ByteOrder {
        self.value_reader.reader.byte_order
    }

    #[inline]
    pub fn read_ifd_offset(&mut self) -> Result<u64, io::Error> {
        if self.value_reader.bigtiff {
            self.read_long8()
        } else {
            self.read_long().map(u64::from)
        }
    }

    /// Reads a TIFF byte value
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, io::Error> {
        let mut buf = [0; 1];
        self.value_reader.reader.inner().read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Reads a TIFF short value
    #[inline]
    pub fn read_short(&mut self) -> Result<u16, io::Error> {
        self.value_reader.reader.read_u16()
    }

    /// Reads a TIFF sshort value
    #[inline]
    pub fn read_sshort(&mut self) -> Result<i16, io::Error> {
        self.value_reader.reader.read_i16()
    }

    /// Reads a TIFF long value
    #[inline]
    pub fn read_long(&mut self) -> Result<u32, io::Error> {
        self.value_reader.reader.read_u32()
    }

    /// Reads a TIFF slong value
    #[inline]
    pub fn read_slong(&mut self) -> Result<i32, io::Error> {
        self.value_reader.reader.read_i32()
    }

    /// Reads a TIFF float value
    #[inline]
    pub fn read_float(&mut self) -> Result<f32, io::Error> {
        self.value_reader.reader.read_f32()
    }

    /// Reads a TIFF double value
    #[inline]
    pub fn read_double(&mut self) -> Result<f64, io::Error> {
        self.value_reader.reader.read_f64()
    }

    #[inline]
    pub fn read_long8(&mut self) -> Result<u64, io::Error> {
        self.value_reader.reader.read_u64()
    }

    #[inline]
    pub fn read_slong8(&mut self) -> Result<i64, io::Error> {
        self.value_reader.reader.read_i64()
    }

    /// Reads a string
    #[inline]
    pub fn read_string(&mut self, length: usize) -> TiffResult<String> {
        let mut out = vec![0; length];
        self.value_reader.reader.inner().read_exact(&mut out)?;
        // Strings may be null-terminated, so we trim anything downstream of the null byte
        if let Some(first) = out.iter().position(|&b| b == 0) {
            out.truncate(first);
        }
        Ok(String::from_utf8(out)?)
    }

    /// Reads a TIFF IFA offset/value field
    #[inline]
    pub fn read_offset(&mut self) -> TiffResult<[u8; 4]> {
        if self.value_reader.bigtiff {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }
        let mut val = [0; 4];
        self.value_reader.reader.inner().read_exact(&mut val)?;
        Ok(val)
    }

    /// Reads a TIFF IFA offset/value field
    #[inline]
    pub fn read_offset_u64(&mut self) -> Result<[u8; 8], io::Error> {
        let mut val = [0; 8];
        self.value_reader.reader.inner().read_exact(&mut val)?;
        Ok(val)
    }

    /// Moves the cursor to the specified offset
    #[inline]
    pub fn goto_offset(&mut self, offset: u32) -> io::Result<()> {
        self.goto_offset_u64(offset.into())
    }

    #[inline]
    pub fn goto_offset_u64(&mut self, offset: u64) -> io::Result<()> {
        self.value_reader.reader.goto_offset(offset)
    }

    /// Read a tag-entry map from a known offset.
    ///
    /// A TIFF [`Directory`], aka. image file directory aka. IFD, refers to a map from
    /// tags–identified by a `u16`–to a typed vector of elements. It is encoded as a list
    /// of ascending tag values with the offset and type of their corresponding values. The
    /// semantic interpretations of a tag and its type requirements depend on the context of the
    /// directory. The main image directories, those iterated over by the `Decoder` after
    /// construction, are represented by [`Tag`] and [`ifd::Value`]. Other forms are EXIF and GPS
    /// data as well as thumbnail Sub-IFD representations associated with each image file.
    ///
    /// This method allows the decoding of a directory from an arbitrary offset in the image file
    /// with no specific semantic interpretation. Such an offset is usually found as the value of
    /// a tag, e.g. [`Tag::SubIfd`], [`Tag::ExifDirectory`], [`Tag::GpsDirectory`] and recovered
    /// from the associated value by [`ifd::Value::into_ifd_pointer`].
    ///
    /// The library will not verify whether the offset overlaps any other directory or would form a
    /// cycle with any other directory when calling this method. This will modify the position of
    /// the reader, i.e. continuing with direct reads at a later point will require going back with
    /// [`Self::goto_offset`].
    pub fn read_directory(&mut self, ptr: IfdPointer) -> TiffResult<Directory> {
        self.value_reader.read_directory(ptr)
    }

    fn check_chunk_type(&self, expected: ChunkType) -> TiffResult<()> {
        if expected != self.image().chunk_type {
            return Err(TiffError::UsageError(UsageError::InvalidChunkType(
                expected,
                self.image().chunk_type,
            )));
        }

        Ok(())
    }

    /// The chunk type (Strips / Tiles) of the image
    pub fn get_chunk_type(&self) -> ChunkType {
        self.image().chunk_type
    }

    /// Number of strips in image
    pub fn strip_count(&mut self) -> TiffResult<u32> {
        self.check_chunk_type(ChunkType::Strip)?;
        let rows_per_strip = self.image().strip_decoder.as_ref().unwrap().rows_per_strip;

        if rows_per_strip == 0 {
            return Ok(0);
        }

        // rows_per_strip - 1 can never fail since we know it's at least 1
        let height = match self.image().height.checked_add(rows_per_strip - 1) {
            Some(h) => h,
            None => return Err(TiffError::IntSizeError),
        };

        let strips = match self.image().planar_config {
            PlanarConfiguration::Chunky => height / rows_per_strip,
            PlanarConfiguration::Planar => height / rows_per_strip * self.image().samples as u32,
        };

        Ok(strips)
    }

    /// Number of tiles in image
    pub fn tile_count(&mut self) -> TiffResult<u32> {
        self.check_chunk_type(ChunkType::Tile)?;
        Ok(u32::try_from(self.image().chunk_offsets.len())?)
    }

    pub fn read_chunk_to_buffer(
        &mut self,
        mut buffer: DecodingBuffer,
        chunk_index: u32,
        output_width: usize,
    ) -> TiffResult<()> {
        let offset = self.image.chunk_file_range(chunk_index)?.0;
        self.goto_offset_u64(offset)?;

        let output_row_stride = (output_width as u64)
            .saturating_mul(self.image.samples_per_pixel() as u64)
            .saturating_mul(self.image.bits_per_sample as u64)
            / 8;

        self.image.expand_chunk(
            &mut self.value_reader,
            buffer.as_bytes_mut(),
            output_row_stride.try_into()?,
            chunk_index,
        )?;

        Ok(())
    }

    fn result_buffer(&self, width: usize, height: usize) -> TiffResult<DecodingResult> {
        let bits_per_sample = self.image().bits_per_sample;

        let row_samples = if bits_per_sample >= 8 {
            width
        } else {
            ((((width as u64) * bits_per_sample as u64) + 7) / 8)
                .try_into()
                .map_err(|_| TiffError::LimitsExceeded)?
        };

        let buffer_size = row_samples
            .checked_mul(height)
            .and_then(|x| x.checked_mul(self.image().samples_per_pixel()))
            .ok_or(TiffError::LimitsExceeded)?;

        let max_sample_bits = self.image().bits_per_sample;
        let limits = &self.value_reader.limits;

        match self.image().sample_format {
            SampleFormat::Uint => match max_sample_bits {
                n if n <= 8 => DecodingResult::new_u8(buffer_size, limits),
                n if n <= 16 => DecodingResult::new_u16(buffer_size, limits),
                n if n <= 32 => DecodingResult::new_u32(buffer_size, limits),
                n if n <= 64 => DecodingResult::new_u64(buffer_size, limits),
                n => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedBitsPerChannel(n),
                )),
            },
            SampleFormat::IEEEFP => match max_sample_bits {
                16 => DecodingResult::new_f16(buffer_size, limits),
                32 => DecodingResult::new_f32(buffer_size, limits),
                64 => DecodingResult::new_f64(buffer_size, limits),
                n => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedBitsPerChannel(n),
                )),
            },
            SampleFormat::Int => match max_sample_bits {
                n if n <= 8 => DecodingResult::new_i8(buffer_size, limits),
                n if n <= 16 => DecodingResult::new_i16(buffer_size, limits),
                n if n <= 32 => DecodingResult::new_i32(buffer_size, limits),
                n if n <= 64 => DecodingResult::new_i64(buffer_size, limits),
                n => Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedBitsPerChannel(n),
                )),
            },
            format => Err(TiffUnsupportedError::UnsupportedSampleFormat(vec![format]).into()),
        }
    }

    /// Read the specified chunk (at index `chunk_index`) and return the binary data as a Vector.
    pub fn read_chunk(&mut self, chunk_index: u32) -> TiffResult<DecodingResult> {
        let data_dims = self.image().chunk_data_dimensions(chunk_index)?;

        let mut result = self.result_buffer(data_dims.0 as usize, data_dims.1 as usize)?;

        self.read_chunk_to_buffer(result.as_buffer(0), chunk_index, data_dims.0 as usize)?;

        Ok(result)
    }

    /// Returns the default chunk size for the current image. Any given chunk in the image is at most as large as
    /// the value returned here. For the size of the data (chunk minus padding), use `chunk_data_dimensions`.
    pub fn chunk_dimensions(&self) -> (u32, u32) {
        self.image().chunk_dimensions().unwrap()
    }

    /// Returns the size of the data in the chunk with the specified index. This is the default size of the chunk,
    /// minus any padding.
    pub fn chunk_data_dimensions(&self, chunk_index: u32) -> (u32, u32) {
        self.image()
            .chunk_data_dimensions(chunk_index)
            .expect("invalid chunk_index")
    }

    /// Decodes the entire image and return it as a Vector
    pub fn read_image(&mut self) -> TiffResult<DecodingResult> {
        let width = self.image().width;
        let height = self.image().height;
        let mut result = self.result_buffer(width as usize, height as usize)?;
        if width == 0 || height == 0 {
            return Ok(result);
        }

        let chunk_dimensions = self.image().chunk_dimensions()?;
        let chunk_dimensions = (
            chunk_dimensions.0.min(width),
            chunk_dimensions.1.min(height),
        );
        if chunk_dimensions.0 == 0 || chunk_dimensions.1 == 0 {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }

        let samples = self.image().samples_per_pixel();
        if samples == 0 {
            return Err(TiffError::FormatError(
                TiffFormatError::InconsistentSizesEncountered,
            ));
        }

        let output_row_bits = (width as u64 * self.image.bits_per_sample as u64)
            .checked_mul(samples as u64)
            .ok_or(TiffError::LimitsExceeded)?;
        let output_row_stride: usize = ((output_row_bits + 7) / 8).try_into()?;

        let chunk_row_bits = (chunk_dimensions.0 as u64 * self.image.bits_per_sample as u64)
            .checked_mul(samples as u64)
            .ok_or(TiffError::LimitsExceeded)?;
        let chunk_row_bytes: usize = ((chunk_row_bits + 7) / 8).try_into()?;

        let chunks_across = ((width - 1) / chunk_dimensions.0 + 1) as usize;

        if chunks_across > 1 && chunk_row_bits % 8 != 0 {
            return Err(TiffError::UnsupportedError(
                TiffUnsupportedError::MisalignedTileBoundaries,
            ));
        }

        let image_chunks = self.image().chunk_offsets.len() / self.image().strips_per_pixel();
        // For multi-band images, only the first band is read.
        // Possible improvements:
        // * pass requested band as parameter
        // * collect bands to a RGB encoding result in case of RGB bands
        for chunk in 0..image_chunks {
            self.goto_offset_u64(self.image().chunk_offsets[chunk])?;

            let x = chunk % chunks_across;
            let y = chunk / chunks_across;
            let buffer_offset =
                y * output_row_stride * chunk_dimensions.1 as usize + x * chunk_row_bytes;
            self.image.expand_chunk(
                &mut self.value_reader,
                &mut result.as_buffer(0).as_bytes_mut()[buffer_offset..],
                output_row_stride,
                chunk as u32,
            )?;
        }

        Ok(result)
    }

    /// Get the IFD decoder for our current image IFD.
    fn current_directory_ifd(&mut self) -> IfdDecoder<'_> {
        // Special fallback. We do not want to error handle not having read a current directory, in
        // particular as the behavior having a directory without tags will produce these errors
        // anyways (most likely). Note that an empty directory is invalid in a TIFF.
        static NO_IFD: Directory = Directory::empty();

        let ifd = self
            .non_image_ifd
            .as_ref()
            .or_else(|| self.image.ifd.as_ref())
            .unwrap_or(&NO_IFD);

        IfdDecoder {
            inner: tag_reader::TagReader {
                decoder: &mut self.value_reader,
                ifd,
            },
        }
    }

    /// Prepare reading values for tags of a given directory.
    ///
    /// # Examples
    ///
    /// This method may be used to read the values of tags in directories that have been previously
    /// read with [`Decoder::read_directory`].
    ///
    /// ```no_run
    /// use tiff::decoder::Decoder;
    /// use tiff::tags::Tag;
    ///
    /// # use std::io::Cursor;
    /// # let mut data = Cursor::new(vec![0]);
    /// let mut decoder = Decoder::new(&mut data).unwrap();
    /// let sub_ifds = decoder.get_tag(Tag::SubIfd)?.into_ifd_vec()?;
    ///
    /// for ifd in sub_ifds {
    ///     let subdir = decoder.read_directory(ifd)?;
    ///     let subfile = decoder.read_directory_tags(&subdir).find_tag(Tag::SubfileType)?;
    ///     // omitted: handle the subfiles, e.g. thumbnails
    /// }
    ///
    /// # Ok::<_, tiff::TiffError>(())
    /// ```
    pub fn read_directory_tags<'ifd>(&'ifd mut self, ifd: &'ifd Directory) -> IfdDecoder<'ifd> {
        IfdDecoder {
            inner: tag_reader::TagReader {
                decoder: &mut self.value_reader,
                ifd,
            },
        }
    }

    /// Tries to retrieve a tag from the current image directory.
    /// Return `Ok(None)` if the tag is not present.
    pub fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<ifd::Value>> {
        self.current_directory_ifd().find_tag(tag)
    }

    /// Tries to retrieve a tag in the current image directory and convert it to the desired
    /// unsigned type.
    pub fn find_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<T>> {
        self.current_directory_ifd().find_tag_unsigned(tag)
    }

    /// Tries to retrieve a tag from the current image directory and convert it to the desired
    /// unsigned type. Returns an error if the tag is not present.
    pub fn get_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<T> {
        self.current_directory_ifd().get_tag_unsigned(tag)
    }

    /// Tries to retrieve a tag from the current image directory.
    /// Returns an error if the tag is not present
    pub fn get_tag(&mut self, tag: Tag) -> TiffResult<ifd::Value> {
        self.current_directory_ifd().get_tag(tag)
    }

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

    pub fn tag_iter(&mut self) -> impl Iterator<Item = TiffResult<(Tag, ifd::Value)>> + '_ {
        self.current_directory_ifd().tag_iter()
    }
}

impl<R: Seek + Read> ValueReader<R> {
    pub(crate) fn read_directory(&mut self, ptr: IfdPointer) -> Result<Directory, TiffError> {
        Self::read_ifd(&mut self.reader, self.bigtiff, ptr)
    }

    /// Reads a IFD entry.
    // An IFD entry has four fields:
    //
    // Tag   2 bytes
    // Type  2 bytes
    // Count 4 bytes
    // Value 4 bytes either a pointer the value itself
    fn read_entry(
        reader: &mut EndianReader<R>,
        bigtiff: bool,
    ) -> TiffResult<Option<(Tag, ifd::Entry)>> {
        let tag = Tag::from_u16_exhaustive(reader.read_u16()?);
        let type_ = match Type::from_u16(reader.read_u16()?) {
            Some(t) => t,
            None => {
                // Unknown type. Skip this entry according to spec.
                reader.read_u32()?;
                reader.read_u32()?;
                return Ok(None);
            }
        };
        let entry = if bigtiff {
            let mut offset = [0; 8];

            let count = reader.read_u64()?;
            reader.inner().read_exact(&mut offset)?;
            ifd::Entry::new_u64(type_, count, offset)
        } else {
            let mut offset = [0; 4];

            let count = reader.read_u32()?;
            reader.inner().read_exact(&mut offset)?;
            ifd::Entry::new(type_, count, offset)
        };
        Ok(Some((tag, entry)))
    }

    /// Reads the IFD starting at the indicated location.
    fn read_ifd(
        reader: &mut EndianReader<R>,
        bigtiff: bool,
        ifd_location: IfdPointer,
    ) -> TiffResult<Directory> {
        reader.goto_offset(ifd_location.0)?;

        let mut entries: BTreeMap<_, _> = BTreeMap::new();

        let num_tags = if bigtiff {
            reader.read_u64()?
        } else {
            reader.read_u16()?.into()
        };

        for _ in 0..num_tags {
            let (tag, entry) = match Self::read_entry(reader, bigtiff)? {
                Some(val) => val,
                None => {
                    continue;
                } // Unknown data type in tag, skip
            };

            entries.insert(tag.to_u16(), entry);
        }

        let next_ifd = if bigtiff {
            reader.read_u64()?
        } else {
            reader.read_u32()?.into()
        };

        let next_ifd = core::num::NonZeroU64::new(next_ifd);

        Ok(Directory { entries, next_ifd })
    }
}

impl IfdDecoder<'_> {
    /// Tries to retrieve a tag.
    /// Return `Ok(None)` if the tag is not present.
    pub fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<ifd::Value>> {
        self.inner.find_tag(tag)
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
}

impl<'l> IfdDecoder<'l> {
    /// Returns an iterator over all tags in the current image, along with their values.
    pub fn tag_iter(self) -> impl Iterator<Item = TiffResult<(Tag, ifd::Value)>> + 'l {
        self.inner
            .ifd
            .iter()
            .map(|(tag, entry)| match self.inner.decoder.entry_val(entry) {
                Ok(value) => Ok((tag, value)),
                Err(err) => Err(err),
            })
    }
}
