use futures::{
    // future::BoxFuture,
    io::{AsyncRead, AsyncReadExt, AsyncSeek, SeekFrom},
    AsyncSeekExt,
};
use std::collections::{HashMap, HashSet};

use crate::{TiffError, TiffFormatError, TiffUnsupportedError, UsageError, TiffResult, ColorType};

// use self::ifd::Directory;
// use self::image::Image;
use crate::tags::{
    CompressionMethod, PhotometricInterpretation, PlanarConfiguration, Predictor, SampleFormat,
    Tag, Type,
};

use crate::decoder::{
    stream::ByteOrder,
    ifd::Value,
    DecodingBuffer,
    DecodingResult,
    Limits,
    ChunkType,
};

extern crate async_trait;

pub use crate::decoder::invert_colors;
use ifd::Directory;
use image::AsyncImage;
use stream::AsyncSmartReader;

pub mod ifd;
pub mod image;
pub mod stream;
pub mod tag_reader;

#[async_trait::async_trait]
pub trait RangeReader {
    async fn read_range(
        &mut self,
        bytes_start: usize,
        bytes_end: usize,
    ) -> futures::io::Result<Vec<u8>>;
}

#[async_trait::async_trait]
impl<R: AsyncRead + AsyncSeek + Unpin + Send> RangeReader for R {
    async fn read_range(
        &mut self,
        bytes_start: usize,
        bytes_end: usize,
    ) -> futures::io::Result<Vec<u8>> {
        let length = bytes_end - bytes_start;
        let mut buffer = vec![0; length];

        // Seek to the start position
        self.seek(SeekFrom::Start(bytes_start as u64)).await?;

        // Read exactly the number of bytes we need
        self.read_exact(&mut buffer).await?;

        Ok(buffer)
    }
}

pub struct Decoder<R: AsyncRead + AsyncSeek + RangeReader + Unpin + Send> {
    reader: AsyncSmartReader<R>,
    bigtiff: bool,
    limits: Limits, // Replace with actual type
    next_ifd: Option<u64>,
    ifd_offsets: Vec<u64>,
    seen_ifds: HashSet<u64>,
    pub image: AsyncImage,
}

impl<R: AsyncRead + AsyncSeek + RangeReader + Unpin + Send> Decoder<R> {
    pub async fn new(mut r: R) -> Result<Decoder<R>, TiffError> {
        let mut endianess = [0; 2];
        r.read_exact(&mut endianess).await?;
        let byte_order = match &endianess {
            b"II" => ByteOrder::LittleEndian,
            b"MM" => ByteOrder::BigEndian,
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureNotFound,
                ));
            }
        };

        let mut reader = AsyncSmartReader::wrap(r, byte_order);

        let bigtiff = match reader.read_u16().await? {
            42 => false,
            43 => {
                if reader.read_u16().await? != 8 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                if reader.read_u16().await? != 0 {
                    return Err(TiffError::FormatError(
                        TiffFormatError::TiffSignatureNotFound,
                    ));
                }
                true
            }
            _ => {
                return Err(TiffError::FormatError(
                    TiffFormatError::TiffSignatureInvalid,
                ));
            }
        };

        let next_ifd = if bigtiff {
            Some(reader.read_u64().await?)
        } else {
            Some(u64::from(reader.read_u32().await?))
        };

        let mut seen_ifds = HashSet::new();
        seen_ifds.insert(next_ifd.unwrap());

        let mut decoder = Decoder {
            reader,
            bigtiff,
            limits: Default::default(), // Replace with actual initialization
            next_ifd,
            ifd_offsets: vec![next_ifd.unwrap()],
            seen_ifds,
            image: AsyncImage {
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
                chunk_type: ChunkType::Tile,
                planar_config: PlanarConfiguration::Chunky,
                strip_decoder: None,
                tile_attributes: None,
                chunk_offsets: Vec::new(),
                chunk_bytes: Vec::new(),
            },
        };

        decoder.next_image().await?;

        Ok(decoder)
    }


    pub fn with_limits(mut self, limits: Limits) -> Decoder<R> {
        self.limits = limits;
        self
    }

    pub fn dimensions(&mut self) -> TiffResult<(u32, u32)> {
        Ok((self.image().width, self.image().height))
    }

    pub fn colortype(&mut self) -> TiffResult<ColorType> {
        self.image().colortype()
    }

    fn image(&self) -> &AsyncImage {
        &self.image
    }

    /// Loads the IFD at the specified index in the list, if one exists
    pub async fn seek_to_image(&mut self, ifd_index: usize) -> TiffResult<()> {
        // Check whether we have seen this IFD before, if so then the index will be less than the length of the list of ifd offsets
        if ifd_index >= self.ifd_offsets.len() {
            // We possibly need to load in the next IFD
            if self.next_ifd.is_none() {
                return Err(TiffError::FormatError(
                    TiffFormatError::ImageFileDirectoryNotFound,
                ));
            }

            loop {
                // Follow the list until we find the one we want, or we reach the end, whichever happens first
                let (_ifd, next_ifd) = self.next_ifd().await?;

                if next_ifd.is_none() {
                    break;
                }

                if ifd_index < self.ifd_offsets.len() {
                    break;
                }
            }
        }

        // If the index is within the list of ifds then we can load the selected image/IFD
        if let Some(ifd_offset) = self.ifd_offsets.get(ifd_index) {
            let (ifd, _next_ifd) = Self::read_ifd(&mut self.reader, self.bigtiff, *ifd_offset).await?;

            self.image = AsyncImage::from_reader(&mut self.reader, ifd, &self.limits, self.bigtiff).await?;

            Ok(())
        } else {
            Err(TiffError::FormatError(
                TiffFormatError::ImageFileDirectoryNotFound,
            ))
        }
    }

    /// reads in the first IFD tag and constructs
    // pub async fn read_first_ifd_into_image_metadata() {

    // }

    // pub async fn get_tile(overview: u64, x_index: u64, y_index: u64) -> TiffResult<DecodingResult> {

    // }

    async fn next_ifd(&mut self) -> TiffResult<(Directory, Option<u64>)> {
        if self.next_ifd.is_none() {
            return Err(TiffError::FormatError(
                TiffFormatError::ImageFileDirectoryNotFound,
            ));
        }

        let (ifd, next_ifd) = Self::read_ifd(
            &mut self.reader,
            self.bigtiff,
            self.next_ifd.take().unwrap(),
        ).await?;

        if let Some(next) = next_ifd {
            if !self.seen_ifds.insert(next) {
                return Err(TiffError::FormatError(TiffFormatError::CycleInOffsets));
            }
            self.next_ifd = Some(next);
            self.ifd_offsets.push(next);
        }

        Ok((ifd, next_ifd))
    }

    /// Returns `true` if there is at least one more image available.
    pub fn more_images(&self) -> bool {
        self.next_ifd.is_some()
    }

    /// Reads in the next image.
    /// If there is no further image in the TIFF file a format error is returned.
    /// To determine whether there are more images call `TIFFDecoder::more_images` instead.
    pub async fn next_image(&mut self) -> TiffResult<()> {
        let (ifd, _next_ifd) = self.next_ifd().await?;

        self.image = AsyncImage::from_reader(&mut self.reader, ifd, &self.limits, self.bigtiff).await?;
        Ok(())
    }

    // Reads the IFD starting at the indicated location.
    /// Reads the ifd, skipping all tags.
    async fn read_ifd(
        reader: &mut AsyncSmartReader<R>,
        bigtiff: bool,
        ifd_location: u64,
    ) -> TiffResult<(Directory, Option<u64>)> {
        reader.goto_offset(ifd_location).await?;

        let mut dir: Directory = HashMap::new();

        let num_tags = if bigtiff {
            reader.read_u64().await?
        } else {
            reader.read_u16().await?.into()
        };

        // const TAG_SIZE: i64 = 2 + 2 + 4 + 4;
        // reader
        //     .seek(std::io::SeekFrom::Current(
        //         TAG_SIZE * i64::try_from(num_tags).unwrap(),
        //     ))
        //     .await?;
        for _ in 0..num_tags {
            let (tag, entry) = match Self::read_entry(reader, bigtiff).await? {
                Some(val) => val,
                None => {
                    continue;
                } // Unknown data type in tag, skip
            };
            dir.insert(tag, entry);
        }

        let next_ifd = if bigtiff {
            reader.read_u64().await?
        } else {
            reader.read_u32().await?.into()
        };

        let next_ifd = match next_ifd {
            0 => None,
            _ => Some(next_ifd),
        };

        Ok((dir, next_ifd))
    }

    /// Reads a IFD entry.
    // An IFD entry has four fields:
    //
    // Tag   2 bytes
    // Type  2 bytes
    // Count 4 bytes
    // Value 4 bytes either a pointer the value itself
    async fn read_entry(
        reader: &mut AsyncSmartReader<R>,
        bigtiff: bool,
    ) -> TiffResult<Option<(Tag, ifd::Entry)>> {
        let tag = Tag::from_u16_exhaustive(reader.read_u16().await?);
        let type_ = match Type::from_u16(reader.read_u16().await?) {
            Some(t) => t,
            None => {
                // Unknown type. Skip this entry according to spec.
                reader.read_u32().await?;
                reader.read_u32().await?;
                return Ok(None);
            }
        };
        let entry = if bigtiff {
            let mut offset = [0; 8];

            let count = reader.read_u64().await?;
            reader.read_exact(&mut offset).await?;
            ifd::Entry::new_u64(type_, count, offset)
        } else {
            let mut offset = [0; 4];

            let count = reader.read_u32().await?;
            reader.read_exact(&mut offset).await?;
            ifd::Entry::new(type_, count, offset)
        };
        Ok(Some((tag, entry)))
    }


    /// Tries to retrieve a tag.
    /// Return `Ok(None)` if the tag is not present.
    pub async fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<Value>> {
        let entry = match self.image().ifd.as_ref().unwrap().get(&tag) {
            None => return Ok(None),
            Some(entry) => entry.clone(),
        };

        Ok(Some(entry.val(
            &self.limits,
            self.bigtiff,
            &mut self.reader,
        ).await?))
    }

    /// Tries to retrieve a tag and convert it to the desired unsigned type.
    pub async fn find_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<T>> {
        self.find_tag(tag).await?
            .map(|v| v.into_u64())
            .transpose()?
            .map(|value| {
                T::try_from(value).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
            })
            .transpose()
    }

    /// Tries to retrieve a vector of all a tag's values and convert them to
    /// the desired unsigned type.
    pub async fn find_tag_unsigned_vec<T: TryFrom<u64>>(
        &mut self,
        tag: Tag,
    ) -> TiffResult<Option<Vec<T>>> {
        self.find_tag(tag).await?
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
    pub async fn get_tag_unsigned<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<T> {
        self.find_tag_unsigned(tag).await?
            .ok_or_else(|| TiffFormatError::RequiredTagNotFound(tag).into())
    }

    /// Tries to retrieve a tag.
    /// Returns an error if the tag is not present
    pub async fn get_tag(&mut self, tag: Tag) -> TiffResult<Value> {
        match self.find_tag(tag).await? {
            Some(val) => Ok(val),
            None => Err(TiffError::FormatError(
                TiffFormatError::RequiredTagNotFound(tag),
            )),
        }
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub async fn get_tag_u32(&mut self, tag: Tag) -> TiffResult<u32> {
        self.get_tag(tag).await?.into_u32()
    }
    pub async fn get_tag_u64(&mut self, tag: Tag) -> TiffResult<u64> {
        self.get_tag(tag).await?.into_u64()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub async fn get_tag_f32(&mut self, tag: Tag) -> TiffResult<f32> {
        self.get_tag(tag).await?.into_f32()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub async fn get_tag_f64(&mut self, tag: Tag) -> TiffResult<f64> {
        self.get_tag(tag).await?.into_f64()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub async fn get_tag_u32_vec(&mut self, tag: Tag) -> TiffResult<Vec<u32>> {
        self.get_tag(tag).await?.into_u32_vec()
    }

    pub async fn get_tag_u16_vec(&mut self, tag: Tag) -> TiffResult<Vec<u16>> {
        self.get_tag(tag).await?.into_u16_vec()
    }
    pub async fn get_tag_u64_vec(&mut self, tag: Tag) -> TiffResult<Vec<u64>> {
        self.get_tag(tag).await?.into_u64_vec()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub async fn get_tag_f32_vec(&mut self, tag: Tag) -> TiffResult<Vec<f32>> {
        self.get_tag(tag).await?.into_f32_vec()
    }

    /// Tries to retrieve a tag and convert it to the desired type.
    pub async fn get_tag_f64_vec(&mut self, tag: Tag) -> TiffResult<Vec<f64>> {
        self.get_tag(tag).await?.into_f64_vec()
    }

    /// Tries to retrieve a tag and convert it to a 8bit vector.
    pub async fn get_tag_u8_vec(&mut self, tag: Tag) -> TiffResult<Vec<u8>> {
        self.get_tag(tag).await?.into_u8_vec()
    }

    /// Tries to retrieve a tag and convert it to a ascii vector.
    pub async fn get_tag_ascii_string(&mut self, tag: Tag) -> TiffResult<String> {
        self.get_tag(tag).await?.into_string()
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

    pub async fn read_chunk_to_buffer(
        &mut self,
        mut buffer: DecodingBuffer<'_>,
        chunk_index: u32,
        output_width: usize,
    ) -> TiffResult<()> {
        let (offset,  length) = self.image.chunk_file_range(chunk_index)?;
        let v = self.reader.read_range(offset.try_into()?, (offset + length).try_into()?).await?;

        let byte_order = self.reader.byte_order;

        let output_row_stride = (output_width as u64)
            .saturating_mul(self.image.samples_per_pixel() as u64)
            .saturating_mul(self.image.bits_per_sample as u64)
            / 8;

        self.image.expand_chunk(
            &mut std::io::Cursor::new(v),
            buffer.as_bytes_mut(),
            output_row_stride.try_into()?,
            byte_order,
            chunk_index,
            &self.limits,
        )?;

        Ok(())
    }

    fn result_buffer(&self, width: usize, height: usize) -> TiffResult<DecodingResult> {
        let buffer_size = match width
            .checked_mul(height)
            .and_then(|x| x.checked_mul(self.image().samples_per_pixel()))
        {
            Some(s) => s,
            None => return Err(TiffError::LimitsExceeded),
        };

        let max_sample_bits = self.image().bits_per_sample;
        match self.image().sample_format {
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
            format => Err(TiffUnsupportedError::UnsupportedSampleFormat(vec![format]).into()),
        }
    }

    /// Read the specified chunk (at index `chunk_index`) and return the binary data as a Vector.
    pub async fn read_chunk(&mut self, chunk_index: u32) -> TiffResult<DecodingResult> {
        let data_dims = self.image().chunk_data_dimensions(chunk_index)?;

        let mut result = self.result_buffer(data_dims.0 as usize, data_dims.1 as usize)?;

        self.read_chunk_to_buffer(result.as_buffer(0), chunk_index, data_dims.0 as usize).await?;

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
    pub async fn read_image(&mut self) -> TiffResult<DecodingResult> {
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

        // in planar config, an image has chunks/n_bands chunks 
        let image_chunks = self.image().chunk_offsets.len() / self.image().strips_per_pixel();
        // For multi-band images, only the first band is read.
        // Possible improvements:
        // * pass requested band as parameter
        // * collect bands to a RGB encoding result in case of RGB bands
        for chunk in 0..image_chunks {
            let (offset,  length) = self.image.chunk_file_range(chunk.try_into().unwrap())?;
            let v = self.reader.read_range(offset.try_into()?, (offset + length).try_into()?).await?;
            let mut reader = std::io::Cursor::new(v);
            // self.goto_offset_u64(self.image().chunk_offsets[chunk]).await?;

            let x = chunk % chunks_across;
            let y = chunk / chunks_across;
            let buffer_offset =
                y * output_row_stride * chunk_dimensions.1 as usize + x * chunk_row_bytes;
            let byte_order = self.reader.byte_order;
            self.image.expand_chunk(
                &mut reader,
                &mut result.as_buffer(0).as_bytes_mut()[buffer_offset..],
                output_row_stride,
                byte_order,
                chunk as u32,
                &self.limits,
            )?;
        }

        Ok(result)
    }


    #[inline]
    pub async fn goto_offset_u64(&mut self, offset: u64) -> std::io::Result<()> {
        self.reader.seek(SeekFrom::Start(offset)).await.map(|_| ())
    }
}

#[cfg(test)]
mod test {
    use super::*;
}
