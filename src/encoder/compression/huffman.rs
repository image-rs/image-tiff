use super::*;
use fax34::BitWriter as _;
use std::io::{self, Write};

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
fn write_run(writer: &mut fax34::VecWriter, white: bool, n: u32) {
    let table = if white {
        fax34::maps::white::ENTRIES
    } else {
        fax34::maps::black::ENTRIES
    };
    let mut n = n;
    while n >= 2560 {
        let _ = writer.write(table[63 + 2560 / 64].1);
        n -= 2560;
    }
    if n >= 64 {
        let d = n & !63;
        let _ = writer.write(table[63 + d as usize / 64].1);
        n -= d;
    }
    let _ = writer.write(table[n as usize].1);
}

impl CompressionAlgorithm for Huffman {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        if self.width == 0 || bytes.len() as u64 != u64::from(self.width).div_ceil(8) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Huffman input must contain exactly one packed row",
            ));
        }

        let mut out = fax34::VecWriter::with_capacity(bytes.len() * 8);
        let mut is_black = false;
        let mut run = 0u32;
        for pixel in 0..self.width {
            let byte = bytes[(pixel / 8) as usize];
            let black = (byte >> (7 - pixel % 8)) & 1 != 0;
            if black == is_black {
                run += 1;
            } else {
                write_run(&mut out, !is_black, run);
                is_black = black;
                run = 1;
            }
        }
        write_run(&mut out, !is_black, run);

        let data = out.finish();
        writer.write_all(&data)?;
        Ok(data.len() as u64)
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
}
