//! Decoder that can be Cloned, sharing the [`Image`] data between threads
//! Therefore, it holds an `Arc<Image>`
//! Loading in the image meatadata should be done using another decoder
//! Also shows how terrificly easy and ergonomic the api for the folks over at geotiff would be :P
use std::sync::Arc;

use futures::{AsyncRead, AsyncSeek};

use crate::decoder::{
    image::Image,
    Decoder, Limits, DecodingResult,
    stream::SmartReader,
    async_decoder::RangeReader,
};
use crate::TiffResult;
/// Decoder that can be Cloned, sharing the [`Image`] data between threads
#[derive(Clone, Debug)]
pub struct ChunkDecoder<R> {
    reader: SmartReader<R>,
    // bigtiff: bool,
    limits: Limits,
    image: Arc<Image>,
}

impl<R: Clone> Clone for SmartReader<R> {
    fn clone(&self) -> Self {
        Self {
            reader: self.reader.clone(),
            byte_order: self.byte_order,
        }
    }
}

impl<R: RangeReader + AsyncRead + AsyncSeek + Clone + Send + Unpin> ChunkDecoder<R>{
    pub fn from_decoder(decoder: Decoder<R>) -> Self {
        ChunkDecoder {
            reader: decoder.reader.clone(),
            // bigtiff: decoder.bigtiff,
            limits: decoder.limits.clone(),
            image: Arc::new(decoder.image().clone()),
        }
    }

    /// Get a reference to the image (in read mode)
    pub fn image(&self) -> &Image {
        // this is really bad
        &self.image//.read().expect("Could not obtain lock")
    }

    pub async fn read_chunk_async(&mut self, chunk_index: u32) -> TiffResult<DecodingResult>{
        // read_chunk code
        let (width, height) = self.image().chunk_data_dimensions(chunk_index)?;
        let mut result = Decoder::<R>::result_buffer(usize::try_from(width)?, usize::try_from(height)?, self.image(), &self.limits)?;
        // read_chunk_to_buffer code
        let (offset, length) = self.image().chunk_file_range(chunk_index)?;
        let v = self.reader.read_range(offset, offset + length).await?;
        let output_row_stride = (width as u64).saturating_mul(self.image().samples_per_pixel() as u64).saturating_mul(self.image.bits_per_sample as u64) / 8;
        self.image().expand_chunk(
            &mut std::io::Cursor::new(v),
            result.as_buffer(0).as_bytes_mut(),
            output_row_stride.try_into()?,
            self.reader.byte_order,
            chunk_index,
            &self.limits,
        )?;
        Ok(result)
    }
}
