use std::{
    convert::TryInto,
    io::{Seek, Write},
};

use flate2::{write::ZlibEncoder, Compression as FlateCompression};

use crate::{
    encoder::{
        colortype::ColorType, compression::Compression, DirectoryEncoder, TiffKind, TiffValue,
    },
    tags::CompressionMethod,
    TiffResult,
};

/// The Deflate algorithm used to compress image data in TIFF files.
#[derive(Debug, Clone)]
pub struct Deflate {
    level: FlateCompression,
    buffer: Vec<u8>,
}

/// The level of compression used by the Deflate algorithm.
/// It allows trading compression ratio for compression speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DeflateLevel {
    /// The fastest possible compression mode.
    Fast = 1,
    /// The conserative choice between speed and ratio.
    Balanced = 6,
    /// The best compression available with Deflate.
    Best = 9,
}

impl Default for DeflateLevel {
    fn default() -> Self {
        DeflateLevel::Balanced
    }
}

impl Deflate {
    /// Lets be greedy and allocate more bytes in advance. We will likely encode longer image strips.
    const DEFAULT_BUFFER_SIZE: usize = 256;

    /// Create a new deflate compr+essor with a specific level of compression.
    pub fn with_level(level: DeflateLevel) -> Self {
        Self {
            buffer: Vec::with_capacity(Self::DEFAULT_BUFFER_SIZE),
            level: FlateCompression::new(level as u32),
        }
    }
}

impl Default for Deflate {
    fn default() -> Self {
        Self::with_level(DeflateLevel::default())
    }
}

impl Compression for Deflate {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::Deflate;

    fn write_to<'a, T: ColorType, K: TiffKind, W: 'a + Write + Seek>(
        &mut self,
        encoder: &mut DirectoryEncoder<'a, W, K>,
        value: &[T::Inner],
    ) -> TiffResult<(K::OffsetType, K::OffsetType)>
    where
        [T::Inner]: TiffValue,
    {
        let data = value.data();
        {
            let mut encoder = ZlibEncoder::new(&mut self.buffer, self.level);
            encoder.write_all(&data)?;
            encoder.finish()?;
        }

        let compressed_byte_count = self.buffer.len().try_into()?;
        let offset = encoder
            .write_data(self.buffer.as_slice())
            .and_then(K::convert_offset)?;

        // Clear the buffer for the next compression.
        self.buffer.clear();

        Ok((offset, compressed_byte_count))
    }
}

#[cfg(test)]
mod tests {
    use crate::encoder::compression::tests::{compress, TEST_DATA};

    #[test]
    fn test_deflate() {
        const EXPECTED_COMPRESSED_DATA: [u8; 64] = [
            0x78, 0x9C, 0x15, 0xC7, 0xD1, 0x0D, 0x80, 0x20, 0x0C, 0x04, 0xD0, 0x55, 0x6E, 0x02,
            0xA7, 0x71, 0x81, 0xA6, 0x41, 0xDA, 0x28, 0xD4, 0xF4, 0xD0, 0xF9, 0x81, 0xE4, 0xFD,
            0xBC, 0xD3, 0x9C, 0x58, 0x04, 0x1C, 0xE9, 0xBD, 0xE2, 0x8A, 0x84, 0x5A, 0xD1, 0x7B,
            0xE7, 0x97, 0xF4, 0xF8, 0x08, 0x8D, 0xF6, 0x66, 0x21, 0x3D, 0x3A, 0xE4, 0xA9, 0x91,
            0x3E, 0xAC, 0xF1, 0x98, 0xB9, 0x70, 0x17, 0x13,
        ];

        let compressed_data = compress(TEST_DATA, super::Deflate::default());
        assert_eq!(compressed_data, EXPECTED_COMPRESSED_DATA);
    }
}
