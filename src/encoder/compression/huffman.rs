use super::*;
use std::io::{self, BufWriter, Write};

/// CCITT Group 3 1-dimensional modified Huffman run-length encoding for bilevel data.
///
/// Each row is encoded independently as alternating white/black run-length codes
/// (bit 0 = white, bit 1 = black) and padded to the next byte boundary; no EOL
/// codes are emitted. Like [`Packbits`], each strip must hold exactly one row so
/// that rows stay byte-aligned.
#[derive(Clone, Copy)]
pub struct Huffman {
    width: u32,
}

impl Huffman {
    /// Create a compressor for one row of `width` pixels.
    ///
    /// Input must contain exactly `width.div_ceil(8)` bytes. Bits beyond the
    /// width in the last byte are ignored. The width must be nonzero.
    pub fn new(width: u32) -> Self {
        Self { width }
    }
}

impl Compression for Huffman {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::Huffman;

    fn get_algorithm(&self) -> Compressor {
        Compressor::Huffman(*self)
    }
}

/// Write one run length as a terminal code plus any makeup codes.
fn write_run(
    write: &mut impl FnMut(fax34::Bits) -> io::Result<()>,
    white: bool,
    n: u32,
) -> io::Result<()> {
    let table = if white {
        fax34::maps::white::ENTRIES
    } else {
        fax34::maps::black::ENTRIES
    };
    let mut n = n;
    while n >= 2560 {
        write(table[63 + 2560 / 64].1)?;
        n -= 2560;
    }
    if n >= 64 {
        let d = n & !63;
        write(table[63 + d as usize / 64].1)?;
        n -= d;
    }
    write(table[n as usize].1)
}

impl CompressionAlgorithm for Huffman {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        if self.width == 0 || bytes.len() as u64 != u64::from(self.width).div_ceil(8) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Huffman input must contain exactly one packed row",
            ));
        }

        let mut writer = BufWriter::with_capacity(bytes.len().min(8 * 1024), writer);
        let (mut partial, mut bits, mut written) = (0u32, 0u8, 0u64);
        let mut write_code = |code: fax34::Bits| -> io::Result<()> {
            partial |= u32::from(code.data) << (32 - bits - code.len);
            bits += code.len;
            while bits >= 8 {
                writer.write_all(&[(partial >> 24) as u8])?;
                partial <<= 8;
                bits -= 8;
                written += 1;
            }
            Ok(())
        };
        let mut is_black = false;
        let mut run = 0u32;
        for pixel in 0..self.width {
            let byte = bytes[(pixel / 8) as usize];
            let black = (byte >> (7 - pixel % 8)) & 1 != 0;
            if black == is_black {
                run += 1;
            } else {
                write_run(&mut write_code, !is_black, run)?;
                is_black = black;
                run = 1;
            }
        }
        write_run(&mut write_code, !is_black, run)?;

        if bits != 0 {
            writer.write_all(&[(partial >> 24) as u8])?;
            written += 1;
        }
        writer.into_inner().map_err(|error| error.into_error())?;
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(bytes: &[u8]) -> Vec<u8> {
        let mut out = io::Cursor::new(Vec::new());
        Huffman::new((bytes.len() * 8) as u32)
            .write_to(&mut out, bytes)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn test_huffman_encode_row() {
        // Two runs of 16 pixels: white(4)=1011, black(4)=011, white(8)=10011,
        // padded to a byte boundary
        assert_eq!(encoded(&[0x0F, 0x00]), vec![0xB7, 0x30]);
    }

    #[test]
    fn test_huffman_encode_makeup_run() {
        // 80 white pixels via makeup code: white(64)=11011 + white(16)=101010
        assert_eq!(encoded(&[0u8; 10]), vec![0xDD, 0x40]);
    }

    #[test]
    fn huffman_streams_compressed_bytes_in_bounded_writes() {
        struct BoundedWriter {
            count: usize,
        }
        impl Write for BoundedWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                assert!(bytes.len() <= 8192, "compressed row was buffered in full");
                self.count += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let pixels = vec![0xAA; 32_768];
        let mut out = BoundedWriter { count: 0 };
        let written = Huffman::new((pixels.len() * 8) as u32)
            .write_to(&mut out, &pixels)
            .unwrap();
        assert_eq!(written, out.count as u64);
        assert!(out.count > 8192);
    }

    #[test]
    fn huffman_propagates_output_failure() {
        let mut storage = [0; 1];
        let mut writer = io::Cursor::new(&mut storage[..]);
        let error = Huffman::new(8).write_to(&mut writer, &[0xAA]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }
}
