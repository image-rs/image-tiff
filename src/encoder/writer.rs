use std::io::{self, Write, Seek, SeekFrom};
use byteorder::{WriteBytesExt, BigEndian, LittleEndian};

/// Byte order of the TIFF file.
#[derive(Clone, Copy, Debug)]
pub enum ByteOrder {
    /// little endian byte order
    LittleEndian,
    /// big endian byte order
    BigEndian
}

pub struct TiffWriter<W> {
    writer: W,
    byte_order: ByteOrder,
    offset: u32,
}

impl<W: Write> TiffWriter<W> {
    pub fn new(writer: W, byte_order: ByteOrder) -> Self {
        Self {
            writer,
            byte_order,
            offset: 0,
        }
    }
    pub fn new_le(writer: W) -> Self {
        Self::new(writer, ByteOrder::LittleEndian)
    }

    pub fn new_be(writer: W) -> Self {
        Self::new(writer, ByteOrder::BigEndian)
    }

    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        self.writer.write_all(bytes)?;
        self.offset += bytes.len() as u32;
        Ok(())
    }

    pub fn write_u8(&mut self, n: u8) -> Result<(), io::Error> {
        self.writer.write_u8(n)?;
        self.offset += 1;
        Ok(())
    }

    pub fn write_u16(&mut self, n: u16) -> Result<(), io::Error> {
        match self.byte_order {
            ByteOrder::LittleEndian => self.writer.write_u16::<LittleEndian>(n),
            ByteOrder::BigEndian => self.writer.write_u16::<BigEndian>(n),
        }?;

        self.offset += 2;

        Ok(())
    }

    pub fn write_u32(&mut self, n: u32) -> Result<(), io::Error> {
        match self.byte_order {
            ByteOrder::LittleEndian => self.writer.write_u32::<LittleEndian>(n),
            ByteOrder::BigEndian => self.writer.write_u32::<BigEndian>(n),
        }?;
        
        self.offset += 4;

        Ok(())
    }

    pub fn pad_word_boundary(&mut self) -> Result<(), io::Error> {
        while self.offset % 4 != 0 {
            self.writer.write_u8(0)?;
        }
    
        Ok(())
    }
}

impl<W: Seek> TiffWriter<W> {
    pub fn too_offset(&mut self, offset: u32) -> Result<(), io::Error> {
        self.offset = offset;
        self.writer.seek(SeekFrom::Start(offset as u64))?;
        Ok(())
    }
}
