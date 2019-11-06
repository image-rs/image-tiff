use error::TiffResult;
use byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian, WriteBytesExt};
use std::io::{self, Seek, SeekFrom, Write};

pub trait TiffByteOrder: ByteOrder {
    fn write_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()>;
}

impl TiffByteOrder for LittleEndian {
    fn write_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.writer.write_u16::<LittleEndian>(0x4949)?;
        writer.writer.write_u16::<LittleEndian>(42)?;

        Ok(())
    }
}

impl TiffByteOrder for BigEndian {
    fn write_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()> {
        writer.writer.write_u16::<BigEndian>(0x4d4d)?;
        writer.writer.write_u16::<BigEndian>(42)?;

        Ok(())
    }
}

pub struct TiffWriter<W> {
    writer: W,
}

impl<W: Write> TiffWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        self.writer.write_all(bytes)?;
        Ok(())
    }

    pub fn write_u8(&mut self, n: u8) -> Result<(), io::Error> {
        self.writer.write_u8(n)?;
        Ok(())
    }

    pub fn write_i8(&mut self, n: i8) -> Result<(), io::Error> {
        self.writer.write_i8(n)?;
        Ok(())
    }

    pub fn write_u16(&mut self, n: u16) -> Result<(), io::Error> {
        self.writer.write_u16::<NativeEndian>(n)?;

        Ok(())
    }

    pub fn write_i16(&mut self, n: i16) -> Result<(), io::Error> {
        self.writer.write_i16::<NativeEndian>(n)?;

        Ok(())
    }

    pub fn write_u32(&mut self, n: u32) -> Result<(), io::Error> {
        self.writer.write_u32::<NativeEndian>(n)?;

        Ok(())
    }

    pub fn write_i32(&mut self, n: i32) -> Result<(), io::Error> {
        self.writer.write_i32::<NativeEndian>(n)?;

        Ok(())
    }

    pub fn write_u64(&mut self, n: u64) -> Result<(), io::Error> {
        self.writer.write_u64::<NativeEndian>(n)?;

        Ok(())
    }

}

impl<W: Write + Seek> TiffWriter<W> {
    pub fn pad_word_boundary(&mut self) -> Result<(), io::Error> {
        let offset = self.offset();
        if offset % 4 != 0 {
            let padding = [0, 0, 0];
            let padd_len = 4 - (offset % 4);
            self.writer.write_all(&padding[..padd_len as usize])?;
        }

        Ok(())
    }
}

impl<W: Seek> TiffWriter<W> {
    pub fn goto_offset(&mut self, offset: u64) -> Result<(), io::Error> {
        self.writer.seek(SeekFrom::Start(offset as u64))?;
        Ok(())
    }

    pub fn offset(&mut self) -> u64 {
        self.writer.seek(SeekFrom::Current(0)).unwrap()
    }
}
