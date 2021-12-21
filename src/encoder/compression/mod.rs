use std::io::prelude::*;

use crate::{
    encoder::{ColorType, DirectoryEncoder, TiffKind, TiffValue},
    error::TiffResult,
    tags::CompressionMethod,
};

mod deflate;
mod lzw;
mod uncompressed;

pub use self::deflate::{Deflate, DeflateLevel};
pub use self::lzw::Lzw;
pub use self::uncompressed::Uncompressed;

/// An algorithm used for compression with associated optional buffers and/or configurations.
pub trait Compression: Default {
    /// The corresponding tag to the algorithm.
    const COMPRESSION_METHOD: CompressionMethod;

    /// Write the data of a specific color type to the given encoder and return the offset and byte count, respectively.
    fn write_to<'a, T: ColorType, K: TiffKind, W: 'a + Write + Seek>(
        &mut self,
        encoder: &mut DirectoryEncoder<'a, W, K>,
        value: &[T::Inner],
    ) -> TiffResult<(K::OffsetType, K::OffsetType)>
    where
        [T::Inner]: TiffValue;
}

#[cfg(test)]
mod tests {
    use crate::encoder::{colortype::Gray8, compression::Compression, TiffEncoder};
    use std::io::Cursor;

    pub const TEST_DATA: &'static [u8] =
        b"This is a string for checking various compression algorithms.";

    pub fn compress<C: Compression>(data: &[u8], compressor: C) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Compress the data
        {
            let mut encoder = TiffEncoder::new(Cursor::new(&mut buffer)).unwrap();
            let encoder = encoder
                .new_image_with_compression::<Gray8, _>(data.len() as u32, 1, compressor)
                .unwrap();
            encoder.write_data(&data).unwrap();
        }

        // Remove the meta data written by the encoder
        buffer.drain(..8);
        buffer.drain(buffer.len() - 178..);

        buffer
    }
}
