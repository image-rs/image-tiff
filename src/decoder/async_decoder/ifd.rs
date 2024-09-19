use futures::{AsyncRead, AsyncReadExt, AsyncSeek};

use crate::decoder::{
    Limits, stream::SmartReader,
    ifd::Value
};
use crate::{TiffResult, TiffError, tags::Type};
pub use crate::decoder::ifd::Entry;

use super::stream::AsyncEndianReader;

impl Entry {
    pub async fn async_val<R: AsyncRead + AsyncSeek + Unpin>(
        &self,
        limits: &Limits,
        bigtiff: bool,
        reader: &mut SmartReader<R>,
    ) -> TiffResult<Value> {
        let bo = reader.byte_order();
        if let Some(res) = self.val_if_in_offset(bigtiff, bo)? {
            return Ok(res);
        }

        // check if we exceed the limits and read required bytes into a buffer if everything is ok
        // This allows us to also create a cursor in async code
        let v_bytes = usize::try_from(self.value_bytes()?)?;
        if v_bytes > limits.decoding_buffer_size {
            return Err(TiffError::LimitsExceeded);
        }
        let mut buf = vec![0; v_bytes];
        reader.goto_offset_async(self.offset(bigtiff, bo)?).await?;
        reader.read_exact(&mut buf).await?;
        let mut r = SmartReader::wrap(std::io::Cursor::new(buf), bo);
        self.val_from_cursor(&mut r)
    }

    /// Reads a single value, treating the value field as an offset field
    pub async fn val_single_into_u64_async<R: AsyncRead + AsyncSeek + Unpin>(
        &self,
        index: u64,
        bigtiff: bool,
        reader: &mut SmartReader<R>
    ) -> TiffResult<u64> {
        reader.goto_offset_async(self.offset(bigtiff, reader.byte_order())? + index * self.tag_size()).await?;
        match self.type_ {
            Type::BYTE => {
                let mut buf = [0u8;1];
                reader.read_exact(&mut buf).await?;
                Ok(u64::from(buf[0]))
            }
            Type::SHORT => Ok(u64::from(reader.read_u16_async().await?)),
            Type::LONG => Ok(u64::from(reader.read_u32_async().await?)),
            Type::LONG8 => Ok(reader.read_u64_async().await?),
            Type::SBYTE => Ok(u64::try_from(reader.read_i8_async().await?)?),
            Type::SSHORT => Ok(u64::try_from(reader.read_i16_async().await?)?),
            Type::SLONG => Ok(u64::try_from(reader.read_i32_async().await?)?),
            Type::SLONG8 => Ok(u64::try_from(reader.read_i64_async().await?)?),
            _ => Err(TiffError::IntSizeError),
        }
    }
}