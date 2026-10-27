use crate::encoder::compression::*;

/// The Zstandard algorithm used to compress image data in TIFF files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Zstd {
    level: i32,
}

impl Zstd {
    /// Creates a compressor with the specified level. `0` uses Zstandard's default.
    /// Negative levels favor faster compression.
    pub fn with_level(level: i32) -> Self {
        Self { level }
    }
}

impl Compression for Zstd {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::ZSTD;

    fn get_algorithm(&self) -> Compressor {
        Compressor::Zstd(*self)
    }
}

impl CompressionAlgorithm for Zstd {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        let mut encoder = ::zstd::stream::read::Encoder::with_buffer(bytes, self.level)?;
        encoder.set_pledged_src_size(Some(bytes.len() as u64))?;
        io::copy(&mut encoder, writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::compression::tests::TEST_DATA;

    #[test]
    fn zstd_frames_and_byte_counts() {
        let large: Vec<u8> = (0..131_073).map(|i| (i % 251) as u8).collect();
        for level in [-3, 0, 6] {
            let mut compressor = Zstd::with_level(level).get_algorithm();
            for bytes in [b"".as_slice(), TEST_DATA, large.as_slice()] {
                let mut output = Vec::new();
                assert_eq!(
                    compressor.write_to(&mut output, bytes).unwrap(),
                    output.len() as u64
                );
                assert_eq!(
                    ::zstd::zstd_safe::find_frame_compressed_size(&output).unwrap(),
                    output.len()
                );
                assert_eq!(
                    ::zstd::zstd_safe::get_frame_content_size(&output).unwrap(),
                    Some(bytes.len() as u64)
                );
                assert_eq!(
                    ::zstd::stream::decode_all(output.as_slice()).unwrap(),
                    bytes
                );
                #[cfg(feature = "zstd-safe-rust")]
                {
                    let mut decoded = Vec::new();
                    io::Read::read_to_end(
                        &mut zrip_decode::streaming::FrameDecoder::new(output.as_slice()),
                        &mut decoded,
                    )
                    .unwrap();
                    assert_eq!(decoded, bytes);
                }
            }
        }
        let mut limited = [0; 4];
        assert_eq!(
            Zstd::default()
                .write_to(&mut limited.as_mut_slice(), b"")
                .unwrap_err()
                .kind(),
            io::ErrorKind::WriteZero
        );
    }
}
