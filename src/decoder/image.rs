use super::ifd::{Directory, Value};
use super::stream::{
    add_app14segment, ByteOrder, DeflateReader, JpegTagApp14Transform, LZWReader, PackBitsReader, JpegReader,
};
use super::tag_reader::TagReader;
use super::{stream::SmartReader, ChunkType};
use super::{DecodingBuffer, Limits};
use crate::tags::{CompressionMethod, PhotometricInterpretation, Predictor, SampleFormat, Tag};
use crate::{ColorType, TiffError, TiffFormatError, TiffResult, TiffUnsupportedError};
use std::convert::{TryFrom, TryInto};
use std::io::{self, Read, Seek, Cursor};
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct StripDecodeState {
    pub rows_per_strip: u32,
}

#[derive(Debug)]
/// Computed values useful for tile decoding
pub(crate) struct TileAttributes {
    pub image_width: usize,
    pub image_height: usize,
    pub samples_per_pixel: usize,

    pub tile_width: usize,
    pub tile_length: usize,
}

impl TileAttributes {
    pub fn tiles_across(&self) -> usize {
        (self.image_width + self.tile_width - 1) / self.tile_width
    }
    pub fn tiles_down(&self) -> usize {
        (self.image_height + self.tile_length - 1) / self.tile_length
    }
    fn padding_right(&self) -> usize {
        self.tile_width - self.image_width % self.tile_width
    }
    fn padding_down(&self) -> usize {
        self.tile_length - self.image_height % self.tile_length
    }
    pub fn row_samples(&self) -> usize {
        self.tile_width * self.samples_per_pixel
    }
    pub fn tile_samples(&self) -> usize {
        self.tile_length * self.tile_width * self.samples_per_pixel
    }
    fn tile_strip_samples(&self) -> usize {
        (self.tile_samples() * self.tiles_across())
            - (self.padding_right() * self.tile_length * self.samples_per_pixel)
    }

    /// Returns the tile offset in the result buffer, counted in samples
    pub fn get_offset(&self, tile: usize) -> usize {
        let row = tile / self.tiles_across();
        let column = tile % self.tiles_across();

        (row * self.tile_strip_samples()) + (column * self.row_samples())
    }

    pub fn get_padding(&self, tile: usize) -> (usize, usize) {
        let row = tile / self.tiles_across();
        let column = tile % self.tiles_across();

        let padding_right = if column == self.tiles_across() - 1 {
            self.padding_right()
        } else {
            0
        };

        let padding_down = if row == self.tiles_down() - 1 {
            self.padding_down()
        } else {
            0
        };

        (padding_right, padding_down)
    }
}

#[derive(Debug)]
pub(crate) struct Image {
    pub ifd: Option<Directory>,
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: Vec<u8>,
    #[allow(unused)]
    pub samples: u8,
    pub sample_format: Vec<SampleFormat>,
    pub photometric_interpretation: PhotometricInterpretation,
    pub compression_method: CompressionMethod,
    pub predictor: Predictor,
    pub jpeg_tables: Option<Arc<Vec<u8>>>,
    pub chunk_type: ChunkType,
    pub strip_decoder: Option<StripDecodeState>,
    pub tile_attributes: Option<TileAttributes>,
    pub chunk_offsets: Vec<u64>,
    pub chunk_bytes: Vec<u64>,
}

impl Image {
    pub fn from_reader<R: Read + Seek>(
        reader: &mut SmartReader<R>,
        ifd: Directory,
        limits: &Limits,
        bigtiff: bool,
    ) -> TiffResult<Image> {
        let mut tag_reader = TagReader {
            reader,
            limits,
            ifd: &ifd,
            bigtiff,
        };

        let width = tag_reader.require_tag(Tag::ImageWidth)?.into_u32()?;
        let height = tag_reader.require_tag(Tag::ImageLength)?.into_u32()?;

        let photometric_interpretation = tag_reader
            .find_tag(Tag::PhotometricInterpretation)?
            .map(Value::into_u16)
            .transpose()?
            .and_then(PhotometricInterpretation::from_u16)
            .ok_or(TiffUnsupportedError::UnknownInterpretation)?;

        // Try to parse both the compression method and the number, format, and bits of the included samples.
        // If they are not explicitly specified, those tags are reset to their default values and not carried from previous images.
        let compression_method = match tag_reader.find_tag(Tag::Compression)? {
            Some(val) => CompressionMethod::from_u16(val.into_u16()?)
                .ok_or(TiffUnsupportedError::UnknownCompressionMethod)?,
            None => CompressionMethod::None,
        };

        let jpeg_tables = if compression_method == CompressionMethod::ModernJPEG
            && ifd.contains_key(&Tag::JPEGTables)
        {
            let mut vec = tag_reader
                .find_tag(Tag::JPEGTables)?
                .unwrap()
                .into_u8_vec()?;
            if vec.len() < 2 {
                return Err(TiffError::FormatError(
                    TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                ));
            }

            // TODO: Can we avoid this somehow?
            if photometric_interpretation == PhotometricInterpretation::RGB {
                add_app14segment(&mut vec, JpegTagApp14Transform::App14TransformUnknown)
            }

            Some(Arc::new(vec))
        } else {
            None
        };

        let samples = tag_reader
            .find_tag(Tag::SamplesPerPixel)?
            .map(Value::into_u16)
            .transpose()?
            .unwrap_or(1)
            .try_into()?;

        let sample_format = match tag_reader.find_tag_uint_vec(Tag::SampleFormat)? {
            Some(vals) => {
                let sample_format: Vec<_> = vals
                    .into_iter()
                    .map(SampleFormat::from_u16_exhaustive)
                    .collect();

                // TODO: for now, only homogenous formats across samples are supported.
                if !sample_format.windows(2).all(|s| s[0] == s[1]) {
                    return Err(TiffUnsupportedError::UnsupportedSampleFormat(sample_format).into());
                }

                sample_format
            }
            None => vec![SampleFormat::Uint],
        };

        let bits_per_sample = match samples {
            1 | 3 | 4 => tag_reader
                .find_tag_uint_vec(Tag::BitsPerSample)?
                .unwrap_or_else(|| vec![1]),
            _ => return Err(TiffUnsupportedError::UnsupportedSampleDepth(samples).into()),
        };

        let predictor = tag_reader
            .find_tag(Tag::Predictor)?
            .map(Value::into_u16)
            .transpose()?
            .map(|p| {
                Predictor::from_u16(p)
                    .ok_or(TiffError::FormatError(TiffFormatError::UnknownPredictor(p)))
            })
            .transpose()?
            .unwrap_or(Predictor::None);

        let chunk_type;
        let chunk_offsets;
        let chunk_bytes;
        let strip_decoder;
        let tile_attributes;
        match (
            ifd.contains_key(&Tag::StripByteCounts),
            ifd.contains_key(&Tag::StripOffsets),
            ifd.contains_key(&Tag::TileByteCounts),
            ifd.contains_key(&Tag::TileOffsets),
        ) {
            (true, true, false, false) => {
                chunk_type = ChunkType::Strip;

                chunk_offsets = tag_reader
                    .find_tag(Tag::StripOffsets)?
                    .unwrap()
                    .into_u64_vec()?;
                chunk_bytes = tag_reader
                    .find_tag(Tag::StripByteCounts)?
                    .unwrap()
                    .into_u64_vec()?;
                let rows_per_strip = tag_reader
                    .find_tag(Tag::RowsPerStrip)?
                    .map(Value::into_u32)
                    .transpose()?
                    .unwrap_or(height);
                strip_decoder = Some(StripDecodeState { rows_per_strip });
                tile_attributes = None;

                if chunk_offsets.len() != chunk_bytes.len() {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InconsistentSizesEncountered,
                    ));
                }
            }
            (false, false, true, true) => {
                chunk_type = ChunkType::Tile;

                let tile_width =
                    usize::try_from(tag_reader.require_tag(Tag::TileWidth)?.into_u32()?)?;
                let tile_length =
                    usize::try_from(tag_reader.require_tag(Tag::TileLength)?.into_u32()?)?;

                if tile_width == 0 {
                    return Err(TiffFormatError::InvalidTagValueType(Tag::TileWidth).into());
                } else if tile_length == 0 {
                    return Err(TiffFormatError::InvalidTagValueType(Tag::TileLength).into());
                }

                strip_decoder = None;
                tile_attributes = Some(TileAttributes {
                    image_width: usize::try_from(width)?,
                    image_height: usize::try_from(height)?,
                    samples_per_pixel: bits_per_sample.len(),
                    tile_width,
                    tile_length,
                });
                chunk_offsets = tag_reader
                    .find_tag(Tag::TileOffsets)?
                    .unwrap()
                    .into_u64_vec()?;
                chunk_bytes = tag_reader
                    .find_tag(Tag::TileByteCounts)?
                    .unwrap()
                    .into_u64_vec()?;

                let tile = tile_attributes.as_ref().unwrap();
                if chunk_offsets.len() != chunk_bytes.len()
                    || chunk_offsets.len() != tile.tiles_down() * tile.tiles_across()
                {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InconsistentSizesEncountered,
                    ));
                }
            }
            (_, _, _, _) => {
                return Err(TiffError::FormatError(
                    TiffFormatError::StripTileTagConflict,
                ))
            }
        };

        Ok(Image {
            ifd: Some(ifd),
            width,
            height,
            bits_per_sample,
            samples,
            sample_format,
            photometric_interpretation,
            compression_method,
            jpeg_tables,
            predictor,
            chunk_type,
            strip_decoder,
            tile_attributes,
            chunk_offsets,
            chunk_bytes,
        })
    }

    pub(crate) fn colortype(&self) -> TiffResult<ColorType> {
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

    fn create_reader<'r, R: 'r + Read>(
        reader: R,
        compression_method: CompressionMethod,
        compressed_length: u64,
        jpeg_tables: Option<Arc<Vec<u8>>>,
    ) -> TiffResult<Box<dyn Read + 'r>> {
        Ok(match compression_method {
            CompressionMethod::None => Box::new(reader),
            CompressionMethod::LZW => {
                Box::new(LZWReader::new(reader, usize::try_from(compressed_length)?))
            }
            CompressionMethod::PackBits => Box::new(PackBitsReader::new(reader, compressed_length)),
            CompressionMethod::Deflate | CompressionMethod::OldDeflate => {
                Box::new(DeflateReader::new(reader))
            }
            CompressionMethod::ModernJPEG => {
                if jpeg_tables.is_some() && compressed_length < 2 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                    ));
                }

                let jpeg_reader = JpegReader::new(reader, compressed_length, jpeg_tables)?;
                let mut decoder = jpeg::Decoder::new(jpeg_reader);
                let data = decoder.decode().unwrap();

                Box::new(Cursor::new(data))
            }
            method => {
                return Err(TiffError::UnsupportedError(
                    TiffUnsupportedError::UnsupportedCompressionMethod(method),
                ))
            }
        })
    }

    pub(crate) fn chunk_byte_range(&self, chunk: usize) -> TiffResult<(u64, u64)> {
        let file_offset = self.chunk_offsets.get(chunk).ok_or(TiffError::FormatError(
            TiffFormatError::InconsistentSizesEncountered,
        ))?;

        let compressed_bytes = self.chunk_bytes.get(chunk).ok_or(TiffError::FormatError(
            TiffFormatError::InconsistentSizesEncountered,
        ))?;

        Ok((*file_offset, *compressed_bytes))
    }

    pub(crate) fn expand_chunk(
        &self,
        reader: impl Read,
        mut buffer: DecodingBuffer,
        output_width: usize,
        byte_order: ByteOrder,
        chunk: usize,
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

        let compressed_bytes = self.chunk_bytes.get(chunk).ok_or(TiffError::FormatError(
            TiffFormatError::InconsistentSizesEncountered,
        ))?;

        let byte_len = buffer.byte_len();
        let compression_method = self.compression_method;
        let photometric_interpretation = self.photometric_interpretation;
        let predictor = self.predictor;
        let samples = self.bits_per_sample.len();

        let padding;
        let chunk_dimensions;
        match self.chunk_type {
            ChunkType::Strip => {
                let width = usize::try_from(self.width)?;
                let height = usize::try_from(self.height)?;
                let strip_attrs = self.strip_decoder.as_ref().unwrap();
                chunk_dimensions = (width, usize::try_from(strip_attrs.rows_per_strip)?);
                padding = (0, (chunk_dimensions.1 * (chunk + 1)).max(height) - height);
            }
            ChunkType::Tile => {
                let tile_attrs = self.tile_attributes.as_ref().unwrap();
                padding = tile_attrs.get_padding(chunk);
                chunk_dimensions = (tile_attrs.tile_width, tile_attrs.tile_length);
            }
        }

        let jpeg_tables = self.jpeg_tables.clone();
        let mut reader =
            Self::create_reader(reader, compression_method, *compressed_bytes, jpeg_tables)?;

        if output_width == chunk_dimensions.0 && padding.0 == 0 {
            let total_samples = chunk_dimensions.0 * (chunk_dimensions.1 - padding.1) * samples;
            let tile = &mut buffer.as_bytes_mut()[..total_samples * byte_len];
            reader.read_exact(tile)?;

            super::fix_endianness(&mut buffer.subrange(0..total_samples), byte_order);
            if photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
                super::invert_colors(&mut buffer.subrange(0..total_samples), color_type);
            }
        } else {
            for row in 0..(dbg!(chunk_dimensions.1) - dbg!(padding.1)) {
                let row_start = dbg!(row) * dbg!(output_width) * dbg!(samples);
                let row_end =
                    dbg!(row_start) + (dbg!(chunk_dimensions.0) - dbg!(padding.0)) * samples;

                let row = &mut buffer.as_bytes_mut()
                    [(dbg!(row_start) * dbg!(byte_len))..(dbg!(row_end) * byte_len)];
                reader.read_exact(row)?;

                // Skip horizontal padding
                if padding.0 > 0 {
                    let len = u64::try_from(padding.0 * samples * byte_len)?;
                    io::copy(&mut reader.by_ref().take(len), &mut io::sink())?;
                }

                super::fix_endianness(&mut buffer.subrange(row_start..row_end), byte_order);
                if photometric_interpretation == PhotometricInterpretation::WhiteIsZero {
                    super::invert_colors(&mut buffer.subrange(row_start..row_end), color_type);
                }
            }
        }

        match predictor {
            Predictor::None => {}
            Predictor::Horizontal => {
                super::rev_hpredict(
                    buffer,
                    (
                        u32::try_from(chunk_dimensions.0 - padding.0)?,
                        u32::try_from(chunk_dimensions.1 - padding.1)?,
                    ),
                    output_width,
                    color_type,
                )?;
            }
            Predictor::__NonExhaustive => unreachable!(),
        }

        Ok(())
    }
}
