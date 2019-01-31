mod writer;
use self::writer::*;
use std::io::{self, Write, Seek};
use crate::decoder::ifd;
use std::collections::{BTreeMap, HashMap};

pub struct Rational {
    pub n: u32,
    pub d: u32,
}

pub trait IfdType {
    type Inner;
    fn byte_len() -> u32;
    fn field_type() -> u16;
    fn count(&self) -> u32;
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error>;
}

impl IfdType for [u8] {
    type Inner = u8;
    fn byte_len() -> u32 {
        1
    }
    fn field_type() -> u16 {
        1
    }
    fn count(&self) -> u32 {
        self.len() as u32
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error> {
        for x in self {
            writer.write_u8(*x)?;
        }
        Ok(())
    }
}

impl IfdType for [u16] {
    type Inner = u16;
    fn byte_len() -> u32 {
        2
    }
    fn field_type() -> u16 {
        3
    }
    fn count(&self) -> u32 {
        self.len() as u32
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error> {
        for x in self {
            writer.write_u16(*x)?;
        }
        Ok(())
    }
}

impl IfdType for [u32] {
    type Inner = u32;
    fn byte_len() -> u32 {
        4
    }
    fn field_type() -> u16 {
        4
    }
    fn count(&self) -> u32 {
        self.len() as u32
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error> {
        for x in self {
            writer.write_u32(*x)?;
        }
        Ok(())
    }
}

impl IfdType for [Rational] {
    type Inner = Rational;
    fn byte_len() -> u32 {
        8
    }
    fn field_type() -> u16 {
        5
    }
    fn count(&self) -> u32 {
        self.len() as u32
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error> {
        for x in self {
            writer.write_u32(x.n)?;
            writer.write_u32(x.d)?;
        }
        Ok(())
    }
}

impl IfdType for [i8] {
    type Inner = i8;
    fn byte_len() -> u32 {
        1
    }
    fn field_type() -> u16 {
        6
    }
    fn count(&self) -> u32 {
        self.len() as u32
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error> {
        for x in self {
            writer.write_u8(*x as u8)?;
        }
        Ok(())
    }
}

impl IfdType for str {
    type Inner = u8;
    fn byte_len() -> u32 {
        1
    }
    fn field_type() -> u16 {
        2
    }
    fn count(&self) -> u32 {
        self.len() as u32
    }
    fn write<W: Write>(&self, writer: &mut TiffWriter<W>) -> Result<(), io::Error> {
        for x in self.chars() {
            if x.is_ascii() {
                writer.write_u8(x as u8)?;
            }
        }
        writer.write_u8(0)?;
        Ok(())
    }
}

pub struct TiffEncoder<W> {
    writer: TiffWriter<W>,
    // Sort tags by ascending order
    ifd: BTreeMap<u16, (u16, u32, Vec<u8>)>,
    ifd_offsets: HashMap<u16, u32>,
    strip_number: u32,
}

impl<W: Write + Seek> TiffEncoder<W> {
    pub fn new_le(writer: W) -> TiffEncoder<W> {
        TiffEncoder {
            writer: TiffWriter::new_le(writer),
            ifd: BTreeMap::new(),
            ifd_offsets: HashMap::new(),
            strip_number: 0,
        }
    }

    pub fn new_be(writer: W) -> TiffEncoder<W> {
        TiffEncoder {
            writer: TiffWriter::new_be(writer),
            ifd: BTreeMap::new(),
            ifd_offsets: HashMap::new(),
            strip_number: 0,
        }
    }

    pub fn write_header(&mut self) -> Result<(), io::Error> {
        match self.writer.byte_order() {
            ByteOrder::LittleEndian => self.writer.write_u16(0x4949)?,
            ByteOrder::BigEndian => self.writer.write_u16(0x4d4d)?,
        };
        self.writer.write_u16(42)?;

        Ok(())
    }

    pub fn new_ifd(&mut self) -> Result<(), io::Error> {
        self.writer.pad_word_boundary()?;
        let offset = self.writer.offset();
        self.writer.write_u32(offset + 4)?;

        self.ifd = BTreeMap::new();

        Ok(())
    }

    pub fn new_ifd_entry<T: IfdType + ?Sized>(&mut self, tag: ifd::Tag, value: &T) {
        let mut bytes: Vec<u8> = Vec::new();
        {
            let mut writer = TiffWriter::new(&mut bytes, self.writer.byte_order());
            value.write(&mut writer).unwrap();
        }
        self.ifd.insert(tag.to_u16(), (<T>::field_type(), value.count(), bytes));
    }

    pub fn finish_ifd(&mut self) -> Result<(), io::Error> {
        self.ifd_offsets = HashMap::new();
        self.writer.write_u16(self.ifd.len() as u16)?;

        let mut value_offset = self.writer.offset() + self.ifd.len() as u32 * 12 + 4;

        {
            let mut ifd_values = Vec::new();

            for (tag, (type_, count, bytes)) in &self.ifd {
                self.writer.write_u16(*tag)?;
                self.writer.write_u16(*type_)?;
                self.writer.write_u32(*count)?;
                if bytes.len() <= 4 {
                    self.ifd_offsets.insert(*tag, self.writer.offset());
                    self.writer.write_bytes(bytes)?;
                    for _ in 0..4 - bytes.len() {
                        self.writer.write_u8(0)?;
                    }
                }
                else {
                    self.ifd_offsets.insert(*tag, value_offset);
                    self.writer.write_u32(value_offset)?;
                    ifd_values.push(bytes);
                    value_offset += bytes.len() as u32;
                }
            }

            self.writer.write_u32(0)?;

            for value in ifd_values {
                self.writer.write_bytes(value)?;
            }
        }

        self.ifd = BTreeMap::new();

        Ok(())
    }

    pub fn update_ifd_value<T: IfdType + ?Sized>(&mut self, tag: ifd::Tag, idx: u32, value: &T) -> Result<(), io::Error> {
        if let Some(off) = self.ifd_offsets.get(&tag.to_u16()) {
            let curr_offset = self.writer.offset();
            self.writer.too_offset(off + idx * <T>::byte_len())?;
            value.write(&mut self.writer)?;
            self.writer.too_offset(curr_offset)?;
        }

        Ok(())
    }

    pub fn write_strip(&mut self, strip_value: &[u8]) -> Result<(), io::Error> {
        let offset = self.writer.offset();
        self.writer.write_bytes(strip_value)?;

        let n = self.strip_number;

        self.update_ifd_value::<[u32]>(ifd::Tag::StripOffsets, n, &[offset])?;
        self.update_ifd_value::<[u32]>(ifd::Tag::StripByteCounts, n, &[strip_value.len() as u32])?;

        Ok(())
    }
}

pub struct IfdEncoder<'a, W> {
    tiff_encoder: &'a mut TiffEncoder<W>,
}

pub struct ImageEncoder<'a, W> {
    tiff_encoder: &'a mut TiffEncoder<W>,
}
