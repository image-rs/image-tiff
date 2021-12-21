use std::{
    convert::TryInto,
    io::{Seek, Write},
};

use weezl::encode::Encoder as LZWEncoder;

use crate::{
    encoder::{
        colortype::ColorType, compression::Compression, DirectoryEncoder, TiffKind, TiffValue,
    },
    tags::CompressionMethod,
    TiffResult,
};

/// The LZW algorithm used to compress image data in TIFF files.
#[derive(Debug, Clone)]
pub struct Lzw {
    buffer: Vec<u8>,
}

impl Default for Lzw {
    fn default() -> Self {
        // Lets be greedy and allocate more bytes in advance. We will likely encode longer image strips.
        const DEFAULT_BUFFER_SIZE: usize = 256;
        Self {
            buffer: Vec::with_capacity(DEFAULT_BUFFER_SIZE),
        }
    }
}

impl Compression for Lzw {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::LZW;

    fn write_to<'a, T: ColorType, K: TiffKind, W: 'a + Write + Seek>(
        &mut self,
        encoder: &mut DirectoryEncoder<'a, W, K>,
        value: &[T::Inner],
    ) -> TiffResult<(K::OffsetType, K::OffsetType)>
    where
        [T::Inner]: TiffValue,
    {
        let bytes = value.data();
        let compressed_byte_count = {
            let mut encoder = LZWEncoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
            let result = encoder.into_vec(&mut self.buffer).encode_all(&bytes);
            result.status.map(|_| result.consumed_out)
        }?
        .try_into()?;

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
    fn test_lzw() {
        const EXPECTED_COMPRESSED_DATA: [u8; 63] = [
            0x80, 0x15, 0x0D, 0x06, 0x93, 0x98, 0x82, 0x08, 0x20, 0x30, 0x88, 0x0E, 0x67, 0x43,
            0x91, 0xA4, 0xDC, 0x67, 0x10, 0x19, 0x8D, 0xE7, 0x21, 0x01, 0x8C, 0xD0, 0x65, 0x31,
            0x9A, 0xE1, 0xD1, 0x03, 0xB1, 0x86, 0x1A, 0x6F, 0x3A, 0xC1, 0x4C, 0x66, 0xF3, 0x69,
            0xC0, 0xE4, 0x65, 0x39, 0x9C, 0xCD, 0x26, 0xF3, 0x74, 0x20, 0xD8, 0x67, 0x89, 0x9A,
            0x4E, 0x86, 0x83, 0x69, 0xCC, 0x5D, 0x01,
        ];

        let compressed_data = compress(TEST_DATA, super::Lzw::default());
        assert_eq!(compressed_data, EXPECTED_COMPRESSED_DATA);
    }
}
