//! All IO functionality needed for TIFF decoding

use std::convert::TryFrom;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Take};
use std::sync::Arc;

/// Byte order of the TIFF file.
#[derive(Clone, Copy, Debug)]
pub enum ByteOrder {
    /// little endian byte order
    LittleEndian,
    /// big endian byte order
    BigEndian,
}

/// Reader that is aware of the byte order.
pub trait EndianReader: Read {
    /// Byte order that should be adhered to
    fn byte_order(&self) -> ByteOrder;

    /// Reads an u16
    #[inline(always)]
    fn read_u16(&mut self) -> Result<u16, io::Error> {
        let mut n = [0u8; 2];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => u16::from_le_bytes(n),
            ByteOrder::BigEndian => u16::from_be_bytes(n),
        })
    }

    /// Reads an i8
    #[inline(always)]
    fn read_i8(&mut self) -> Result<i8, io::Error> {
        let mut n = [0u8; 1];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i8::from_le_bytes(n),
            ByteOrder::BigEndian => i8::from_be_bytes(n),
        })
    }

    /// Reads an i16
    #[inline(always)]
    fn read_i16(&mut self) -> Result<i16, io::Error> {
        let mut n = [0u8; 2];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i16::from_le_bytes(n),
            ByteOrder::BigEndian => i16::from_be_bytes(n),
        })
    }

    /// Reads an u32
    #[inline(always)]
    fn read_u32(&mut self) -> Result<u32, io::Error> {
        let mut n = [0u8; 4];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        })
    }

    /// Reads an i32
    #[inline(always)]
    fn read_i32(&mut self) -> Result<i32, io::Error> {
        let mut n = [0u8; 4];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i32::from_le_bytes(n),
            ByteOrder::BigEndian => i32::from_be_bytes(n),
        })
    }

    /// Reads an u64
    #[inline(always)]
    fn read_u64(&mut self) -> Result<u64, io::Error> {
        let mut n = [0u8; 8];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        })
    }

    /// Reads an i64
    #[inline(always)]
    fn read_i64(&mut self) -> Result<i64, io::Error> {
        let mut n = [0u8; 8];
        self.read_exact(&mut n)?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i64::from_le_bytes(n),
            ByteOrder::BigEndian => i64::from_be_bytes(n),
        })
    }

    /// Reads an f32
    #[inline(always)]
    fn read_f32(&mut self) -> Result<f32, io::Error> {
        let mut n = [0u8; 4];
        self.read_exact(&mut n)?;
        Ok(f32::from_bits(match self.byte_order() {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        }))
    }

    /// Reads an f64
    #[inline(always)]
    fn read_f64(&mut self) -> Result<f64, io::Error> {
        let mut n = [0u8; 8];
        self.read_exact(&mut n)?;
        Ok(f64::from_bits(match self.byte_order() {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        }))
    }
}

///
/// # READERS
///

///
/// ## Deflate Reader
///

pub type DeflateReader<R> = flate2::read::ZlibDecoder<R>;

///
/// ## LZW Reader
///

/// Reader that decompresses LZW streams
pub struct LZWReader<R: Read> {
    reader: BufReader<Take<R>>,
    decoder: weezl::decode::Decoder,
}

impl<R: Read> LZWReader<R> {
    /// Wraps a reader
    pub fn new(reader: R, compressed_length: usize) -> LZWReader<R> {
        Self {
            reader: BufReader::with_capacity(
                (32 * 1024).min(compressed_length),
                reader.take(u64::try_from(compressed_length).unwrap()),
            ),
            decoder: weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8),
        }
    }
}

impl<R: Read> Read for LZWReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let result = self.decoder.decode_bytes(self.reader.fill_buf()?, buf);
            self.reader.consume(result.consumed_in);

            match result.status {
                Ok(weezl::LzwStatus::Ok) => {
                    if result.consumed_out == 0 {
                        continue;
                    } else {
                        return Ok(result.consumed_out);
                    }
                }
                Ok(weezl::LzwStatus::NoProgress) => {
                    assert_eq!(result.consumed_in, 0);
                    assert_eq!(result.consumed_out, 0);
                    assert!(self.reader.buffer().is_empty());
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "no lzw end code found",
                    ));
                }
                Ok(weezl::LzwStatus::Done) => {
                    return Ok(result.consumed_out);
                }
                Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidData, err)),
            }
        }
    }
}

///
/// ## JPEG Reader (for "new-style" JPEG format (TIFF compression tag 7))
///

pub(crate) struct JpegReader {
    jpeg_tables: Option<Arc<Vec<u8>>>,

    buffer: io::Cursor<Vec<u8>>,

    offset: usize,
}

impl JpegReader {
    /// Constructs new JpegReader wrapping a SmartReader.
    /// Because JPEG compression in TIFF allows to save quantization and/or huffman tables in one
    /// central location, the constructor accepts this data as `jpeg_tables` here containing either
    /// or both.
    /// These `jpeg_tables` are simply prepended to the remaining jpeg image data.
    /// Because these `jpeg_tables` start with a `SOI` (HEX: `0xFFD8`) or __start of image__ marker
    /// which is also at the beginning of the remaining JPEG image data and would
    /// confuse the JPEG renderer, one of these has to be taken off. In this case the first two
    /// bytes of the remaining JPEG data is removed because it follows `jpeg_tables`.
    /// Similary, `jpeg_tables` ends with a `EOI` (HEX: `0xFFD9`) or __end of image__ marker,
    /// this has to be removed as well (last two bytes of `jpeg_tables`).
    pub fn new<R: Read>(
        mut reader: R,
        length: u64,
        jpeg_tables: Option<Arc<Vec<u8>>>,
    ) -> io::Result<JpegReader> {
        // Read jpeg image data
        let mut segment = vec![0; length as usize];

        reader.read_exact(&mut segment[..])?;

        match jpeg_tables {
            Some(jpeg_tables) => {
                assert!(
                    jpeg_tables.len() >= 2,
                    "jpeg_tables, if given, must be at least 2 bytes long. Got {:?}",
                    jpeg_tables
                );

                assert!(
                    length >= 2,
                    "if jpeg_tables is given, length must be at least 2 bytes long, got {}",
                    length
                );

                let mut buffer = io::Cursor::new(segment);
                // Skip the first two bytes (marker bytes)
                buffer.seek(SeekFrom::Start(2))?;

                Ok(JpegReader {
                    buffer,
                    jpeg_tables: Some(jpeg_tables),
                    offset: 0,
                })
            }
            None => Ok(JpegReader {
                buffer: io::Cursor::new(segment),
                jpeg_tables: None,
                offset: 0,
            }),
        }
    }
}

impl Read for JpegReader {
    // #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut start = 0;

        if let Some(jpeg_tables) = &self.jpeg_tables {
            if jpeg_tables.len() - 2 > self.offset {
                // Read (rest of) jpeg_tables to buf (without the last two bytes)
                let size_remaining = jpeg_tables.len() - self.offset - 2;
                let to_copy = size_remaining.min(buf.len());

                buf[start..start + to_copy]
                    .copy_from_slice(&jpeg_tables[self.offset..self.offset + to_copy]);

                self.offset += to_copy;

                if to_copy == buf.len() {
                    return Ok(to_copy);
                }

                start += to_copy;
            }
        }

        let read = self.buffer.read(&mut buf[start..])?;
        self.offset += read;

        Ok(read + start)
    }
}

///
/// ## PackBits Reader
///

enum PackBitsReaderState {
    Header,
    Literal,
    Repeat { value: u8 },
}

/// Reader that unpacks Apple's `PackBits` format
pub struct PackBitsReader<R: Read> {
    reader: Take<R>,
    state: PackBitsReaderState,
    count: usize,
}

impl<R: Read> PackBitsReader<R> {
    /// Wraps a reader
    pub fn new(reader: R, length: u64) -> Self {
        Self {
            reader: reader.take(length),
            state: PackBitsReaderState::Header,
            count: 0,
        }
    }
}

impl<R: Read> Read for PackBitsReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while let PackBitsReaderState::Header = self.state {
            if self.reader.limit() == 0 {
                return Ok(0);
            }
            let mut header: [u8; 1] = [0];
            self.reader.read_exact(&mut header)?;
            let h = header[0] as i8;
            if h >= -127 && h <= -1 {
                let mut data: [u8; 1] = [0];
                self.reader.read_exact(&mut data)?;
                self.state = PackBitsReaderState::Repeat { value: data[0] };
                self.count = (1 - h as isize) as usize;
            } else if h >= 0 {
                self.state = PackBitsReaderState::Literal;
                self.count = h as usize + 1;
            } else {
                // h = -128 is a no-op.
            }
        }

        let length = buf.len().min(self.count);
        let actual = match self.state {
            PackBitsReaderState::Literal => self.reader.read(&mut buf[..length])?,
            PackBitsReaderState::Repeat { value } => {
                for b in &mut buf[..length] {
                    *b = value;
                }

                length
            }
            PackBitsReaderState::Header => unreachable!(),
        };

        self.count -= actual;
        if self.count == 0 {
            self.state = PackBitsReaderState::Header;
        }
        return Ok(actual);
    }
}

///
/// ## SmartReader Reader
///

/// Reader that is aware of the byte order.
#[derive(Debug)]
pub struct SmartReader<R>
where
    R: Read,
{
    reader: R,
    pub byte_order: ByteOrder,
}

impl<R> SmartReader<R>
where
    R: Read,
{
    /// Wraps a reader
    pub fn wrap(reader: R, byte_order: ByteOrder) -> SmartReader<R> {
        SmartReader { reader, byte_order }
    }
    pub fn into_inner(self) -> R {
        self.reader
    }
}
impl<R: Read + Seek> SmartReader<R> {
    pub fn goto_offset(&mut self, offset: u64) -> io::Result<()> {
        self.seek(io::SeekFrom::Start(offset)).map(|_| ())
    }
}

impl<R> EndianReader for SmartReader<R>
where
    R: Read,
{
    #[inline(always)]
    fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }
}

impl<R: Read> Read for SmartReader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<R: Read + Seek> Seek for SmartReader<R> {
    #[inline]
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.reader.seek(pos)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_packbits() {
        let encoded = vec![
            0xFE, 0xAA, 0x02, 0x80, 0x00, 0x2A, 0xFD, 0xAA, 0x03, 0x80, 0x00, 0x2A, 0x22, 0xF7,
            0xAA,
        ];
        let encoded_len = encoded.len();

        let buff = io::Cursor::new(encoded);
        let mut decoder = PackBitsReader::new(buff, encoded_len as u64);

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let expected = vec![
            0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0xAA, 0xAA, 0xAA, 0xAA, 0x80, 0x00, 0x2A, 0x22,
            0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        ];
        assert_eq!(decoded, expected);
    }
}
