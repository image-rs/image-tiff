use super::ifd::{Directory, Value};
use super::tag_reader::TagReader;
use super::Limits;
use super::{stream::SmartReader, ChunkType};
use crate::tags::{CompressionMethod, PhotometricInterpretation, Predictor, SampleFormat, Tag};
use crate::{TiffError, TiffFormatError, TiffResult, TiffUnsupportedError};
use std::convert::{TryFrom, TryInto};
use std::io::{Read, Seek};

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
    pub samples: u8,
    pub sample_format: Vec<SampleFormat>,
    pub photometric_interpretation: PhotometricInterpretation,
    pub compression_method: CompressionMethod,
    pub predictor: Predictor,
    pub jpeg_tables: Option<Vec<u8>>,
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
            let vec = tag_reader
                .find_tag(Tag::JPEGTables)?
                .unwrap()
                .into_u8_vec()?;
            if vec.len() < 2 {
                return Err(TiffError::FormatError(
                    TiffFormatError::InvalidTagValueType(Tag::JPEGTables),
                ));
            }
            Some(vec)
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
}
