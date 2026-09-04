//! All IO functionality needed for TIFF decoding
#[cfg(feature = "fax")]
use fax34::BitReader as _;
#[cfg(feature = "webp")]
use std::io::Cursor;
use std::io::{self, BufRead, BufReader, Read, Seek, Take};

pub use crate::tags::ByteOrder;

/// Reader that is aware of the byte order.
#[derive(Debug)]
pub struct EndianReader<R> {
    reader: R,
    pub(crate) byte_order: ByteOrder,
}

impl<R: Read> EndianReader<R> {
    pub fn new(reader: R, byte_order: ByteOrder) -> Self {
        Self { reader, byte_order }
    }

    pub fn inner(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn goto_offset(&mut self, offset: u64) -> io::Result<()>
    where
        R: Seek,
    {
        self.reader.seek(io::SeekFrom::Start(offset))?;
        Ok(())
    }

    /// Reads an u16
    #[inline(always)]
    pub fn read_u16(&mut self) -> Result<u16, io::Error> {
        let mut n = [0u8; 2];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => u16::from_le_bytes(n),
            ByteOrder::BigEndian => u16::from_be_bytes(n),
        })
    }

    /// Reads an i16
    #[inline(always)]
    pub fn read_i16(&mut self) -> Result<i16, io::Error> {
        let mut n = [0u8; 2];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i16::from_le_bytes(n),
            ByteOrder::BigEndian => i16::from_be_bytes(n),
        })
    }

    /// Reads an u32
    #[inline(always)]
    pub fn read_u32(&mut self) -> Result<u32, io::Error> {
        let mut n = [0u8; 4];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        })
    }

    /// Reads an i32
    #[inline(always)]
    pub fn read_i32(&mut self) -> Result<i32, io::Error> {
        let mut n = [0u8; 4];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i32::from_le_bytes(n),
            ByteOrder::BigEndian => i32::from_be_bytes(n),
        })
    }

    /// Reads an u64
    #[inline(always)]
    pub fn read_u64(&mut self) -> Result<u64, io::Error> {
        let mut n = [0u8; 8];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        })
    }

    /// Reads an i64
    #[inline(always)]
    pub fn read_i64(&mut self) -> Result<i64, io::Error> {
        let mut n = [0u8; 8];
        self.reader.read_exact(&mut n)?;
        Ok(match self.byte_order {
            ByteOrder::LittleEndian => i64::from_le_bytes(n),
            ByteOrder::BigEndian => i64::from_be_bytes(n),
        })
    }

    /// Reads an f32
    #[inline(always)]
    pub fn read_f32(&mut self) -> Result<f32, io::Error> {
        let mut n = [0u8; 4];
        self.reader.read_exact(&mut n)?;
        Ok(f32::from_bits(match self.byte_order {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        }))
    }

    /// Reads an f64
    #[inline(always)]
    pub fn read_f64(&mut self) -> Result<f64, io::Error> {
        let mut n = [0u8; 8];
        self.reader.read_exact(&mut n)?;
        Ok(f64::from_bits(match self.byte_order {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        }))
    }
}

//
// # READERS
//

/// Type alias for the deflate Reader
#[cfg(feature = "deflate")]
pub type DeflateReader<R> = flate2::read::ZlibDecoder<R>;

//
// ## LZW Reader
//

/// Reader that decompresses LZW streams
#[cfg(feature = "lzw")]
pub struct LZWReader<R: Read> {
    reader: BufReader<Take<R>>,
    decoder: weezl::decode::Decoder,
}

#[cfg(feature = "lzw")]
impl<R: Read> LZWReader<R> {
    /// Wraps a reader
    pub fn new(reader: R, compressed_length: usize) -> LZWReader<R> {
        let mut buffered = BufReader::with_capacity(
            (32 * 1024).min(compressed_length),
            reader.take(u64::try_from(compressed_length).unwrap()),
        );

        // `Compression = 5` covers both the current LZW variant and the pre-TIFF-6.0
        // "old-style" one, with no tag to distinguish them. New-style is MSB-first with
        // the "early change" code-size switch, and its stream begins with the Clear code
        // (256) -> first byte 0x80. Old-style is LSB-first *without* the early change
        // (GIF-style increment timing), and begins with 0x00 followed by a byte with bit
        // 0 set. This is the heuristic libtiff uses to select its compatibility decoder;
        // on a short/failed peek we default to the modern variant.
        let old_style = matches!(
            buffered.fill_buf(),
            Ok(head) if head.len() >= 2 && head[0] == 0x00 && head[1] & 0x01 != 0
        );

        let configuration = if old_style {
            weezl::decode::Configuration::new(weezl::BitOrder::Lsb, 8)
        } else {
            weezl::decode::Configuration::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
        }
        .with_yield_on_full_buffer(true);

        Self {
            reader: buffered,
            decoder: configuration.build(),
        }
    }
}

#[cfg(feature = "lzw")]
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

//
// ## PackBits Reader
//

/// Internal state machine for the PackBitsReader.
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

/// Iterator adapter that reverses bits in each byte for FillOrder=2 (LSB-to-MSB).
#[cfg(feature = "fax")]
type FaxBytes<R> =
    core::iter::Map<io::Bytes<io::BufReader<io::Take<R>>>, fn(io::Result<u8>) -> io::Result<u8>>;

#[cfg(feature = "fax")]
fn fax_byte_iter<R: Read>(reader: R, len: u64, fill_order: u16) -> FaxBytes<R> {
    let map_fn: fn(io::Result<u8>) -> io::Result<u8> = if fill_order == 2 {
        |r| r.map(u8::reverse_bits)
    } else {
        |r| r
    };
    io::BufReader::new(reader.take(len)).bytes().map(map_fn)
}

#[cfg(feature = "fax")]
pub struct Group4Reader<R: Read> {
    decoder: fax34::decoder::Group4Decoder<FaxBytes<R>>,
    line_buf: io::Cursor<Vec<u8>>,
    height: u32,
    width: u16,
    y: u32,
}

#[cfg(feature = "fax")]
impl<R: Read> Group4Reader<R> {
    pub fn new(
        dimensions: (u32, u32),
        reader: R,
        compressed_length: u64,
        fill_order: u16,
    ) -> crate::TiffResult<Self> {
        let width = u16::try_from(dimensions.0)?;
        let height = dimensions.1;

        Ok(Self {
            decoder: fax34::decoder::Group4Decoder::new(
                fax_byte_iter(reader, compressed_length, fill_order),
                width,
            )?,
            line_buf: io::Cursor::new(Vec::with_capacity(width.into())),
            width,
            height,
            y: 0,
        })
    }
}

#[cfg(feature = "fax")]
impl<R: Read> Read for Group4Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Either we have not read any line or we are at the end of a line.
        if self.line_buf.position() as usize == self.line_buf.get_ref().len()
            && self.y < self.height
        {
            let next = self.decoder.advance().map_err(std::io::Error::other)?;

            match next {
                fax34::decoder::DecodeStatus::End => (),
                fax34::decoder::DecodeStatus::Incomplete => {
                    self.y += 1;

                    // We known `transitions` yields exactly `self.width` items (per doc).
                    // FIXME: performance. We do not need an individual pixel iterator, filling
                    // memory especially for long runs can be much quicker. The `transitions` are
                    // the positions at which each run-length ends, i.e. the prefix sum of run
                    // lengths. Runs in fax4 start with white.
                    let transitions = fax34::decoder::pels(self.decoder.transition(), self.width);

                    let buffer = self.line_buf.get_mut();
                    buffer.resize(usize::from(self.width).div_ceil(8), 0u8);

                    let target = &mut buffer[..];

                    // Note: it may seem strange to treat black as 0b1 and white as 0b0 despite all
                    // our streams by default decoding as-if PhotometricInterpretation::BlackIsMin.
                    // This is however consistent with libtiff. It seems that fax4's "White"
                    // differs from what libtiff thinks of as "White". For content, a line of data
                    // is in runlength encoding of white-black-white-black always starting with
                    // white. In libtiff, the loop always does both colors in one go and the
                    // structure is:
                    //
                    // ```
                    // for (; runs < erun; runs += 2)
                    //   // white run
                    //       do { *lp++ = 0L; } while (…)
                    //   // black run
                    //       do { *lp++ = -1L; } while (…)
                    // ```
                    //
                    // So indeed the Fax4::White run is implemented by filling with zeros.
                    let mut bits = transitions.map(|c| match c {
                        fax34::Color::Black => true,
                        fax34::Color::White => false,
                    });

                    // Assemble bits in MSB as per our library representation for buffer.
                    for byte in target {
                        let mut val = 0;

                        for (idx, bit) in bits.by_ref().take(8).enumerate() {
                            val |= u8::from(bit) << (7 - idx % 8);
                        }

                        *byte = val;
                    }

                    self.line_buf.set_position(0);
                }
            }
        }

        self.line_buf.read(buf)
    }
}

/// Owning byte reader for Group 3 decoding: the compressed data is consumed
/// into an iterator so the reader doesn't borrow the buffer.
#[cfg(feature = "fax")]
type OwnedBitReader = fax34::ByteReader<
    core::iter::Map<std::vec::IntoIter<u8>, fn(u8) -> Result<u8, std::convert::Infallible>>,
>;

/// Decodes CCITT Group 3 (T.4) 1D compressed data line by line.
///
/// The fax crate's `Group3Decoder` has a bug where `new()` doesn't consume the
/// initial EOL marker, causing its lookup tables to destructively consume bits
/// from the EOL when trying to decode run-length codes. We work around this by
/// using the crate's lower-level `white::decode` / `black::decode` functions
/// and handling EOL markers ourselves.
#[cfg(feature = "fax")]
pub struct Group3Reader {
    reader: OwnedBitReader,
    line_buf: io::Cursor<Vec<u8>>,
    height: u32,
    width: u16,
    y: u32,
}

#[cfg(feature = "fax")]
impl Group3Reader {
    pub fn new<R: Read>(
        dimensions: (u32, u32),
        reader: R,
        compressed_length: u64,
        fill_order: u16,
    ) -> crate::TiffResult<Self> {
        let width = u16::try_from(dimensions.0)?;
        let height = dimensions.1;

        // Buffer all compressed data and apply FillOrder bit reversal
        let mut compressed = vec![0u8; compressed_length as usize];
        reader.take(compressed_length).read_exact(&mut compressed)?;
        if fill_order == 2 {
            for b in &mut compressed {
                *b = b.reverse_bits();
            }
        }

        // Create an owning bit reader (Vec consumed by into_iter, no borrow issues)
        let map_fn: fn(u8) -> Result<u8, std::convert::Infallible> = Ok;
        let mut bit_reader = fax34::ByteReader::new(compressed.into_iter().map(map_fn)).unwrap();

        // Skip fill bits + initial EOL marker
        Self::skip_eol(&mut bit_reader);

        Ok(Self {
            reader: bit_reader,
            line_buf: io::Cursor::new(Vec::with_capacity(width.into())),
            width,
            height,
            y: 0,
        })
    }

    /// Skip zero fill bits followed by the terminating '1' bit of an EOL code.
    fn skip_eol(reader: &mut impl fax34::BitReader) {
        while reader.peek(1) == Some(0) {
            let _ = reader.consume(1);
        }
        if reader.peek(1) == Some(1) {
            let _ = reader.consume(1);
        }
    }

    /// Decode a full run-length value (terminal + optional makeup codes).
    fn with_markup<R: fax34::BitReader>(
        decoder: fn(&mut R) -> Option<u16>,
        reader: &mut R,
    ) -> Option<u16> {
        let mut sum = 0u16;
        while let Some(n) = decoder(reader) {
            sum = sum.checked_add(n)?;
            if n < 64 {
                return Some(sum);
            }
        }
        None
    }
}

#[cfg(feature = "fax")]
impl Read for Group3Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.line_buf.position() as usize == self.line_buf.get_ref().len()
            && self.y < self.height
        {
            // Decode one line of alternating white/black run-lengths
            let mut transitions = Vec::new();
            let mut a0 = 0u16;
            let mut is_white = true;
            loop {
                let run = if is_white {
                    Self::with_markup(fax34::maps::white::decode, &mut self.reader)
                } else {
                    Self::with_markup(fax34::maps::black::decode, &mut self.reader)
                };
                match run {
                    Some(len) => {
                        a0 = match a0.checked_add(len) {
                            Some(v) => v,
                            None => break, // overflow = corrupt data
                        };
                        transitions.push(a0);
                        is_white = !is_white;
                    }
                    None => break,
                }
            }

            self.y += 1;

            // Convert transitions to packed bilevel output
            let bytes_per_line = usize::from(self.width).div_ceil(8);
            let buffer = self.line_buf.get_mut();
            buffer.clear();
            buffer.resize(bytes_per_line, 0u8);

            let pels = fax34::decoder::pels(&transitions, self.width);
            for (i, c) in pels.enumerate() {
                if c == fax34::Color::Black {
                    buffer[i / 8] |= 1 << (7 - (i % 8));
                }
            }
            self.line_buf.set_position(0);

            // Skip fill bits + line-ending EOL
            Self::skip_eol(&mut self.reader);
        }

        self.line_buf.read(buf)
    }
}

#[cfg(feature = "webp")]
pub struct WebPReader {
    inner: Cursor<Vec<u8>>,
}

#[cfg(feature = "webp")]
impl WebPReader {
    pub fn new<R: Read + Seek>(
        reader: R,
        compressed_length: u64,
        samples: u16,
    ) -> crate::TiffResult<Self> {
        let mut decoder =
            image_webp::WebPDecoder::new(io::BufReader::new(reader.take(compressed_length)))
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if !(samples == 4 || (samples == 3 && !decoder.has_alpha())) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad sample count for WebP compressed data",
            )
            .into());
        }

        let total_bytes =
            samples as usize * decoder.dimensions().0 as usize * decoder.dimensions().1 as usize;
        let mut data = vec![0; total_bytes];

        decoder
            .read_image(&mut data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Add a fully opaque alpha channel if needed
        if samples == 4 && !decoder.has_alpha() {
            for i in (0..(total_bytes / 4)).rev() {
                data[i * 4 + 3] = 255;
                data[i * 4 + 2] = data[i * 3 + 2];
                data[i * 4 + 1] = data[i * 3 + 1];
                data[i * 4] = data[i * 3];
            }
        }

        Ok(Self {
            inner: Cursor::new(data),
        })
    }
}

#[cfg(feature = "webp")]
impl Read for WebPReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// The fax lookup tables may peek past EOF, but must never consume those bits.
#[cfg(feature = "fax")]
struct HuffmanBitReader {
    inner: OwnedBitReader,
    remaining: u64,
}

#[cfg(feature = "fax")]
impl fax34::BitReader for HuffmanBitReader {
    type Error = io::Error;

    fn peek(&self, bits: u8) -> Option<u16> {
        self.inner.peek(bits)
    }

    fn consume(&mut self, bits: u8) -> io::Result<()> {
        self.remaining = self.remaining.checked_sub(u64::from(bits)).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated Huffman code")
        })?;
        // The owning byte iterator is infallible.
        match self.inner.consume(bits) {
            Ok(()) => Ok(()),
            Err(never) => match never {},
        }
    }

    fn bits_to_byte_boundary(&self) -> u8 {
        self.inner.bits_to_byte_boundary()
    }
}

/// Decodes TIFF Compression=2 (CCITT 1D modified Huffman RLE) line by line.
///
/// Unlike Group 3, rows carry no EOL codes and each row begins on a byte
/// boundary, so decoding stops at the line width and the reader skips to the
/// next byte after every row.
#[cfg(feature = "fax")]
pub struct HuffmanReader {
    reader: HuffmanBitReader,
    line_buf: io::Cursor<Vec<u8>>,
    height: u32,
    width: u16,
    y: u32,
}

#[cfg(feature = "fax")]
impl HuffmanReader {
    pub fn new<R: Read>(
        dimensions: (u32, u32),
        reader: R,
        compressed_length: u64,
        fill_order: u16,
    ) -> crate::TiffResult<Self> {
        let width = u16::try_from(dimensions.0)?;
        let height = dimensions.1;

        // Buffer all compressed data and apply FillOrder bit reversal
        let compressed_len = usize::try_from(compressed_length)?;
        let padded_len = compressed_len
            .checked_add(2)
            .ok_or(crate::TiffError::LimitsExceeded)?;
        let mut compressed = vec![0u8; padded_len];
        reader
            .take(compressed_length)
            .read_exact(&mut compressed[..compressed_len])?;
        if fill_order == 2 {
            for b in &mut compressed[..compressed_len] {
                *b = b.reverse_bits();
            }
        }

        // The table lookups peek a full LUT width (up to 13 bits), which fails
        // on fewer remaining bits even when the next code itself is shorter.
        // Permit lookahead into zero padding, but track the actual length so
        // HuffmanBitReader rejects codes that consume synthetic bits.
        let remaining = compressed_length
            .checked_mul(8)
            .ok_or(crate::TiffError::LimitsExceeded)?;

        // Create an owning bit reader (Vec consumed by into_iter, no borrow issues)
        let map_fn: fn(u8) -> Result<u8, std::convert::Infallible> = Ok;
        let bit_reader = fax34::ByteReader::new(compressed.into_iter().map(map_fn)).unwrap();

        Ok(Self {
            reader: HuffmanBitReader {
                inner: bit_reader,
                remaining,
            },
            line_buf: io::Cursor::new(Vec::with_capacity(usize::from(width).div_ceil(8))),
            width,
            height,
            y: 0,
        })
    }
}

#[cfg(feature = "fax")]
impl Read for HuffmanReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.line_buf.position() as usize == self.line_buf.get_ref().len()
            && self.y < self.height
        {
            // Decode one line of alternating white/black run-lengths;
            // stop at the line width so row padding bits are never consumed
            let bytes_per_line = usize::from(self.width).div_ceil(8);
            // Keep partial output unreadable if decoding this row fails.
            self.line_buf.set_position(bytes_per_line as u64);
            let buffer = self.line_buf.get_mut();
            buffer.clear();
            buffer.resize(bytes_per_line, 0);

            let mut a0 = 0u16;
            let mut is_white = true;
            while a0 < self.width {
                let run = if is_white {
                    Group3Reader::with_markup(fax34::maps::white::decode, &mut self.reader)
                } else {
                    Group3Reader::with_markup(fax34::maps::black::decode, &mut self.reader)
                };
                let len = run.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid or truncated Huffman run",
                    )
                })?;
                let end = a0
                    .checked_add(len)
                    .filter(|&end| end <= self.width)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "Huffman run exceeds row width")
                    })?;
                if !is_white {
                    for pixel in usize::from(a0)..usize::from(end) {
                        buffer[pixel / 8] |= 1 << (7 - pixel % 8);
                    }
                }
                a0 = end;
                is_white = !is_white;
            }

            let padding = self.reader.bits_to_byte_boundary();
            self.reader.consume(padding)?;

            self.y += 1;

            self.line_buf.set_position(0);
        }

        self.line_buf.read(buf)
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

    #[cfg(feature = "fax")]
    #[test]
    fn test_huffman_reader() {
        // Two CCITT 1D rows of 16 pixels: white(4)=1011, black(4)=011,
        // white(8)=10011, each row padded to a byte boundary
        let encoded = vec![0xB7, 0x30, 0xB7, 0x30];
        let encoded_len = encoded.len();

        let buff = io::Cursor::new(encoded);
        let mut decoder = HuffmanReader::new((16, 2), buff, encoded_len as u64, 1).unwrap();

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let expected = vec![0x0F, 0x00, 0x0F, 0x00];
        assert_eq!(decoded, expected);
    }

    #[cfg(feature = "fax")]
    #[test]
    fn test_huffman_reader_makeup_run() {
        // One CCITT 1D row of 80 pixels via makeup code:
        // white(64)=11011 + white(16)=101010, padded to a byte boundary
        let encoded = vec![0xDD, 0x40];
        let encoded_len = encoded.len();

        let buff = io::Cursor::new(encoded);
        let mut decoder = HuffmanReader::new((80, 1), buff, encoded_len as u64, 1).unwrap();

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let expected = vec![0u8; 10];
        assert_eq!(decoded, expected);
    }

    #[cfg(feature = "fax")]
    #[test]
    fn test_huffman_reader_fill_order_2() {
        // Same row as test_huffman_reader with bits of each byte reversed
        let encoded = vec![0xED, 0x0C];
        let encoded_len = encoded.len();

        let buff = io::Cursor::new(encoded);
        let mut decoder = HuffmanReader::new((16, 1), buff, encoded_len as u64, 2).unwrap();

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        let expected = vec![0x0F, 0x00];
        assert_eq!(decoded, expected);
    }

    /// Compress with the encoder, one strip per row, then decode.
    #[cfg(feature = "fax")]
    fn huffman_roundtrip(dimensions: (u32, u32), data: &[u8]) -> Vec<u8> {
        use crate::encoder::compression::{CompressionAlgorithm, Huffman};

        let bytes_per_row = usize::try_from(dimensions.0).unwrap().div_ceil(8);
        let mut compressed = Vec::new();
        for row in data.chunks(bytes_per_row) {
            let mut out = io::Cursor::new(Vec::new());
            Huffman::new(dimensions.0).write_to(&mut out, row).unwrap();
            compressed.extend_from_slice(&out.into_inner());
        }
        let compressed_len = compressed.len();

        let buff = io::Cursor::new(compressed);
        let mut decoder = HuffmanReader::new(dimensions, buff, compressed_len as u64, 1).unwrap();

        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();
        decoded
    }

    #[cfg(feature = "fax")]
    #[test]
    fn test_huffman_roundtrip() {
        assert_eq!(huffman_roundtrip((16, 1), &[0xAA, 0x55]), vec![0xAA, 0x55]);
    }

    #[cfg(feature = "fax")]
    #[test]
    fn test_huffman_roundtrip_partial_byte_row() {
        // Width 5 with three rows. Only the width pixels belong to the image,
        // so the padding bits in each row's last byte decode as zeros:
        // 0xF0 -> 11110___, 0x0F -> 00001___, 0xFF -> 11111___
        assert_eq!(
            huffman_roundtrip((5, 3), &[0xF0, 0x0F, 0xFF]),
            vec![0xF0, 0x08, 0xF8]
        );
    }
    #[cfg(feature = "fax")]
    #[test]
    fn huffman_reuses_row_buffer_without_leaking_previous_bits() {
        // Alternating pixels, all zero, all one, then an unaligned black run.
        assert_eq!(
            huffman_roundtrip((9, 4), &[0xAA, 0x80, 0, 0, 0xFF, 0x80, 0x3C, 0]),
            [0xAA, 0x80, 0, 0, 0xFF, 0x80, 0x3C, 0]
        );
    }

    #[cfg(feature = "fax")]
    #[test]
    fn huffman_rejects_invalid_rows() {
        for (width, data) in [
            (8, vec![]),
            (8, vec![0]),
            (8, vec![0x35]),
            (5, vec![0x35, 0x14]),
            (80, vec![0xDD]),
            (13, vec![0xB1]),
        ] {
            let mut reader =
                HuffmanReader::new((width, 1), io::Cursor::new(&data), data.len() as u64, 1)
                    .unwrap();
            let mut output = Vec::new();
            assert!(
                reader.read_to_end(&mut output).is_err(),
                "accepted {data:?} at width {width}"
            );
        }
    }
}
