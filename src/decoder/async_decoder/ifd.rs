use futures::{AsyncRead, AsyncReadExt, AsyncSeek};

use crate::decoder::{
    Limits, stream::SmartReader,
    ifd::Value
};
use crate::{TiffResult, TiffError};
pub use crate::decoder::ifd::Entry;

use super::stream::EndianAsyncReader;

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
}