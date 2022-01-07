use super::ChunkType;
use super::ifd::Directory;
use crate::tags::{
    CompressionMethod, PhotometricInterpretation, Predictor, SampleFormat,
};

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

}