use std::{
    convert::TryInto,
    io::{Seek, Write},
};

use crate::{
    encoder::{
        colortype::ColorType, compression::Compression, DirectoryEncoder, TiffKind, TiffValue,
    },
    tags::CompressionMethod,
    TiffResult,
};

/// The default algorithm which does not compress at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Uncompressed;

impl Compression for Uncompressed {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::None;

    fn write_to<'a, T: ColorType, K: TiffKind, W: 'a + Write + Seek>(
        &mut self,
        encoder: &mut DirectoryEncoder<'a, W, K>,
        value: &[T::Inner],
    ) -> TiffResult<(K::OffsetType, K::OffsetType)>
    where
        [T::Inner]: TiffValue,
    {
        let byte_count = value.len().try_into()?;
        let offset = encoder.write_data(value).and_then(K::convert_offset)?;
        Ok((offset, byte_count))
    }
}

#[cfg(test)]
mod tests {
    use crate::encoder::compression::tests::{compress, TEST_DATA};

    #[test]
    fn test_no_compression() {
        let compressed_data = compress(TEST_DATA, super::Uncompressed);
        assert_eq!(TEST_DATA, compressed_data);
    }
}
