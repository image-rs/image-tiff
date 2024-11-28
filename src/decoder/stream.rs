//! All IO functionality needed for TIFF decoding

use crate::TiffResult;
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use fax::decoder::Group4Decoder;
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Seek, Take};

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
            if (-127..=-1).contains(&h) {
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
        Ok(actual)
    }
}

pub struct Group4Reader<R: Read> {
    decoder: Group4Decoder<std::io::Bytes<std::io::Take<R>>>,
    bits: BitVec<u8, Msb0>,
    byte_buf: VecDeque<u8>,
    height: u16,
    width: u16,
    y: u16,
    expand_samples_to_bytes: bool,
}

impl<R: Read> Group4Reader<R> {
    pub fn new(
        dimensions: (u32, u32),
        reader: R,
        compressed_length: u64,
        expand_samples_to_bytes: bool,
    ) -> TiffResult<Self> {
        let width = u16::try_from(dimensions.0)?;
        let height = u16::try_from(dimensions.1)?;

        Ok(Self {
            decoder: Group4Decoder::new(reader.take(compressed_length).bytes(), width)?,
            bits: BitVec::new(),
            byte_buf: VecDeque::new(),
            width: width,
            height: height,
            y: 0,
            expand_samples_to_bytes: expand_samples_to_bytes,
        })
    }
}

impl<R: Read> Read for Group4Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.byte_buf.is_empty() && self.y < self.height {
            let next = self
                .decoder
                .advance()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            match next {
                fax::decoder::DecodeStatus::End => (),
                fax::decoder::DecodeStatus::Incomplete => {
                    self.y += 1;
                    let transitions = fax::decoder::pels(self.decoder.transition(), self.width);
                    if self.expand_samples_to_bytes {
                        self.byte_buf.extend(transitions.map(|c| match c {
                            fax::Color::Black => 0xFF,
                            fax::Color::White => 0x00,
                        }))
                    } else {
                        self.bits.extend(transitions.map(|c| match c {
                            fax::Color::Black => true,
                            fax::Color::White => false,
                        }));
                        self.byte_buf.extend(self.bits.as_raw_slice());
                        self.bits.clear();
                    }
                }
            }
        }
        self.byte_buf.read(buf)
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
