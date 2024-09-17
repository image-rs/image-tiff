use futures::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};
use std::io;
use crate::decoder::stream::{SmartReader, ByteOrder};

macro_rules! read_async_fn {
    ($name:ident, $type:ty) => {
        /// reads an $type
        #[inline(always)]
        async fn $name(&mut self) -> Result<$type, io::Error> {
            let mut n = [0u8; std::mem::size_of::<$type>()];
            self.read_exact(&mut n).await?;
            Ok(match self.byte_order() {
                ByteOrder::LittleEndian => <$type>::from_le_bytes(n),
                ByteOrder::BigEndian => <$type>::from_be_bytes(n),
            })
        }
    };
}

#[async_trait::async_trait]
/// Reader that is aware of the byte order.
pub trait EndianAsyncReader: AsyncRead + Unpin {
    /// Byte order that should be adhered to
    fn byte_order(&self) -> ByteOrder;
    
    read_async_fn!(read_u16, u16);
    read_async_fn!(read_i8, i8);
    read_async_fn!(read_i16, i16);
    read_async_fn!(read_u32, u32);
    read_async_fn!(read_i32, i32);
    read_async_fn!(read_u64, u64);
    read_async_fn!(read_i64, i64);
    read_async_fn!(read_f32, f32);
    read_async_fn!(read_f64, f64);
}



impl<R: AsyncRead + Unpin> EndianAsyncReader for SmartReader<R> {
    #[inline(always)]
    fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for SmartReader<R> {
    fn poll_read(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut [u8],
            ) -> std::task::Poll<io::Result<usize>> {
        let pinned = std::pin::Pin::new(&mut self.get_mut().reader);
        pinned.poll_read(cx, buf)
    }
}

impl<R: AsyncSeek + Unpin> AsyncSeek for SmartReader<R> {
    fn poll_seek(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                pos: io::SeekFrom,
            ) -> std::task::Poll<io::Result<u64>> {
        let pinned = std::pin::Pin::new(&mut self.get_mut().reader);
        pinned.poll_seek(cx, pos)
    }
}

impl<R: AsyncSeek + Unpin> SmartReader<R> {
    pub async fn goto_offset_async(&mut self, offset: u64) -> io::Result<()> {
        self.seek(io::SeekFrom::Start(offset)).await.map(|_| ())
    }
}